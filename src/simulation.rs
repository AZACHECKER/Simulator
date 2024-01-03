use std::str::FromStr;
use std::sync::Arc;
use crate::structs::StorageOverride;
use crate::SharedSimulationState;
use dashmap::mapref::one::RefMut;
use ethers::abi::Uint;
use serde::Deserialize;
use tokio::sync::Mutex;
use uuid::Uuid;
use warp::reply::Json;
use warp::Rejection;

use crate::structs::{
        SimulationRequest,
        SimulationResponse,
        StatefulSimulationRequest,
        StatefulSimulationResponse,
        StatefulSimulationEndResponse,
        CallTrace,
        PermissiveUint,
        State,
        IncorrectChainIdError,
        InvalidBlockNumbersError,
        MultipleChainIdsError,
        NoURLForChainIdError,
        StateNotFound,
        FailedToSetBlockTimestamp,
    };

use super::structs::Config;
use super::structs::{ CallRawRequest, Evm };

impl From<State> for StorageOverride {
    fn from(value: State) -> Self {
        let (slots, diff) = match value {
            State::Full { state } => (state, false),
            State::Diff { state_diff } => (state_diff, true),
        };

        StorageOverride {
            slots: slots
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect(),
            diff,
        }
    }
}

impl From<PermissiveUint> for Uint {
    fn from(value: PermissiveUint) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for PermissiveUint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de> {
        // Accept value in hex or decimal formats
        let value = String::deserialize(deserializer)?;
        let parsed = if value.starts_with("0x") {
            Uint::from_str(&value).map_err(serde::de::Error::custom)?
        } else {
            Uint::from_dec_str(&value).map_err(serde::de::Error::custom)?
        };
        Ok(Self(parsed))
    }
}

fn chain_id_to_fork_url(chain_id: u64) -> Result<String, Rejection> {
    match chain_id {
        // ethereum
        1 => Ok("https://eth.llamarpc.com".to_string()),
        5 => Ok("https://eth-goerli.g.alchemy.com/v2/demo".to_string()),
        11155111 => Ok("https://eth-sepolia.g.alchemy.com/v2/demo".to_string()),
        // polygon
        137 => Ok("https://polygon-mainnet.g.alchemy.com/v2/demo".to_string()),
        80001 => Ok("https://polygon-mumbai.g.alchemy.com/v2/demo".to_string()),
        // avalanche
        43114 => Ok("https://api.avax.network/ext/bc/C/rpc".to_string()),
        43113 => Ok("https://api.avax-test.network/ext/bc/C/rpc".to_string()),
        // fantom
        250 => Ok("https://rpcapi.fantom.network/".to_string()),
        4002 => Ok("https://rpc.testnet.fantom.network/".to_string()),
        // xdai
        100 => Ok("https://rpc.xdaichain.com/".to_string()),
        // bsc
        56 => Ok("https://bsc-dataseed.binance.org/".to_string()),
        97 => Ok("https://data-seed-prebsc-1-s1.binance.org:8545/".to_string()),
        // arbitrum
        42161 => Ok("https://arb1.arbitrum.io/rpc".to_string()),
        421613 => Ok("https://goerli-rollup.arbitrum.io/rpc".to_string()),
        // optimism
        10 => Ok("https://mainnet.optimism.io/".to_string()),
        420 => Ok("https://goerli.optimism.io/".to_string()),
        _ => Err(NoURLForChainIdError.into()),
    }
}

async fn run(
    evm: &mut Evm,
    transaction: SimulationRequest,
    commit: bool
) -> Result<SimulationResponse, Rejection> {
    for (address, state_override) in transaction.state_overrides.into_iter().flatten() {
        evm.override_account(
            address,
            state_override.balance.map(Uint::from),
            state_override.nonce,
            state_override.code,
            state_override.state.map(StorageOverride::from)
        )?;
    }

    let call = CallRawRequest {
        from: transaction.from,
        to: transaction.to,
        value: transaction.value.map(Uint::from),
        data: transaction.data,
        access_list: transaction.access_list,
        format_trace: transaction.format_trace.unwrap_or_default(),
    };
    let result = if commit {
        evm.call_raw_committing(call, transaction.gas_limit).await?
    } else {
        evm.call_raw(call).await?
    };

    Ok(SimulationResponse {
        simulation_id: 1,
        gas_used: result.gas_used,
        block_number: result.block_number,
        success: result.success,
        trace: result.trace.unwrap_or_default().arena.into_iter().map(CallTrace::from).collect(),
        logs: result.logs,
        exit_reason: result.exit_reason,
        formatted_trace: result.formatted_trace,
        return_data: result.return_data,
    })
}

pub async fn simulate(transaction: SimulationRequest, config: Config) -> Result<Json, Rejection> {
    let fork_url = config
        .fork_url
        .map_or_else(|| chain_id_to_fork_url(transaction.chain_id), Ok)?;

    let mut evm = Evm::new(
        None,
        fork_url,
        transaction.block_number,
        transaction.gas_limit,
        true,
        config.etherscan_key,
    )
    .map_err(|err| warp::reject::custom(err))?;

    if evm.get_chain_id() != Uint::from(transaction.chain_id) {
        return Err(warp::reject::custom(IncorrectChainIdError()));
    }

    if let Some(timestamp) = transaction.block_timestamp {
        evm.set_block_timestamp(timestamp)
            .await
            .map_err(|_| warp::reject::custom(FailedToSetBlockTimestamp))?;
    }

    let response = run(&mut evm, transaction, false).await?;

    Ok(warp::reply::json(&response))
}

pub async fn simulate_bundle(
    transactions: Vec<SimulationRequest>,
    config: Config
) -> Result<Json, Rejection> {
    let first_chain_id = transactions[0].chain_id;
    let first_block_number = transactions[0].block_number;
    let first_block_timestamp = transactions[0].block_timestamp;

    let fork_url = config.fork_url.unwrap_or(chain_id_to_fork_url(first_chain_id)?);

    // Obtain the EVM from the Result<Evm, CustomRejection>.
    let mut evm = match
        Evm::new(
            None,
            fork_url,
            first_block_number,
            transactions[0].gas_limit,
            true,
            config.etherscan_key
        )
    {
        Ok(evm) => evm, // Successfully obtained the EVM.
        Err(err) => {
            return Err(warp::reject::custom(err));
        } // Return the rejection error.
    };

    if evm.get_chain_id() != Uint::from(first_chain_id) {
        return Err(warp::reject::custom(IncorrectChainIdError()));
    }

    if let Some(timestamp) = first_block_timestamp {
        evm.set_block_timestamp(timestamp).await.expect("failed to set block timestamp");
    }

    let response = Vec::with_capacity(transactions.len());
    let response = process_transactions(&mut evm, transactions, response).await?;

    Ok(warp::reply::json(&response))
}

pub async fn simulate_stateful_new(
    stateful_simulation_request: StatefulSimulationRequest,
    config: Config,
    state: Arc<SharedSimulationState>
) -> Result<Json, Rejection> {
    let fork_url = config.fork_url.unwrap_or(
        chain_id_to_fork_url(stateful_simulation_request.chain_id)?
    );

    // Obtain the EVM from the Result<Evm, CustomRejection>.
    let mut evm = match
        Evm::new(
            None,
            fork_url,
            stateful_simulation_request.block_number,
            stateful_simulation_request.gas_limit,
            true,
            config.etherscan_key
        )
    {
        Ok(evm) => evm, // Successfully obtained the EVM.
        Err(err) => {
            return Err(warp::reject::custom(err));
        } // Return the rejection error.
    };

    if let Some(timestamp) = stateful_simulation_request.block_timestamp {
        evm.set_block_timestamp(timestamp).await?;
    }

    let new_id = Uuid::new_v4();
    state.evms.insert(new_id, Arc::new(Mutex::new(evm)));

    let response = StatefulSimulationResponse {
        stateful_simulation_id: new_id,
    };

    Ok(warp::reply::json(&response))
}

pub async fn simulate_stateful_end(
    param: Uuid,
    state: Arc<SharedSimulationState>
) -> Result<Json, Rejection> {
    if state.evms.contains_key(&param) {
        state.evms.remove(&param);
        let response = StatefulSimulationEndResponse { success: true };
        Ok(warp::reply::json(&response))
    } else {
        Err(warp::reject::custom(StateNotFound()))
    }
}

pub async fn simulate_stateful(
    param: Uuid,
    transactions: Vec<SimulationRequest>,
    state: Arc<SharedSimulationState>
) -> Result<Json, Rejection> {
    let first_chain_id = transactions[0].chain_id;

    let response = Vec::with_capacity(transactions.len());

    let evm_ref_mut: RefMut<'_, Uuid, Arc<Mutex<Evm>>> = state.evms
        .get_mut(&param)
        .ok_or_else(warp::reject::not_found)?;

    let evm = evm_ref_mut.value();
    let mut evm = evm.lock().await;

    if evm.get_chain_id() != Uint::from(first_chain_id) {
        return Err(warp::reject::custom(IncorrectChainIdError()));
    }

    let response = process_transactions(&mut evm, transactions, response).await?;

    Ok(warp::reply::json(&response))
}

async fn process_transactions(
    evm: &mut Evm,
    transactions: Vec<SimulationRequest>,
    mut response: Vec<SimulationResponse>,
) -> Result<Vec<SimulationResponse>, Rejection> {
    let first_chain_id = transactions[0].chain_id;
    let first_block_number = transactions[0].block_number;

    for transaction in transactions {
        if transaction.chain_id != first_chain_id {
            return Err(warp::reject::custom(MultipleChainIdsError()));
        }
        if
            transaction.block_number != first_block_number ||
            transaction.block_number.unwrap() != evm.get_block().as_u64()
            {
                let tx_block = transaction.block_number.expect("Transaction has no block number");
                if transaction.block_number < first_block_number || tx_block < evm.get_block().as_u64() {
                    return Err(warp::reject::custom(InvalidBlockNumbersError()));
                }
                evm.set_block(tx_block).await.expect("Failed to set block number");
                evm.set_block_timestamp(evm.get_block_timestamp().as_u64() + 12).await.expect(
                    "Failed to set block timestamp"
                );
            }
            response.push(run(evm, transaction, true).await?);
        }

        Ok(response)
}
