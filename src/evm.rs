

use ethers::abi::{ Address, Uint };

use ethers::types::transaction::eip2930::AccessList;
use ethers::types::Bytes;
use foundry_config::Chain;
use foundry_evm::executor::fork::CreateFork;
use foundry_evm::executor::{ opts::EvmOpts, Backend, ExecutorBuilder };
use foundry_evm::trace::identifier::{ EtherscanIdentifier, SignaturesIdentifier };
use foundry_evm::trace::node::CallTraceNode;
use foundry_evm::trace::CallTraceDecoderBuilder;
use foundry_evm::utils::{ h160_to_b160, u256_to_ru256 };
use revm::db::DatabaseRef;
use revm::primitives::{ Account, Bytecode, Env, StorageSlot };
use revm::DatabaseCommit;

use crate::errors::CustomRejection;
use crate::errors::CustomRejection::EvmError;
use crate::structs::CallTrace;

use crate::structs::{
    CallRawRequest,
    CallRawResult,
    StorageOverride,
    Evm,
};

impl From<CallTraceNode> for CallTrace {
    fn from(item: CallTraceNode) -> Self {
        CallTrace {
            call_type: item.trace.kind,
            from: item.trace.caller,
            to: item.trace.address,
            value: item.trace.value,
        }
    }
}


impl Evm {
    pub fn new(
        env: Option<Env>,
        fork_url: String,
        fork_block_number: Option<u64>,
        gas_limit: u64,
        tracing: bool,
        etherscan_key: Option<String>
    ) -> Result<Self, CustomRejection> {
        let evm_opts = EvmOpts {
            fork_url: Some(fork_url.clone()),
            fork_block_number,
            env: foundry_evm::executor::opts::Env {
                chain_id: None,
                code_size_limit: None,
                gas_price: Some(0),
                gas_limit: u64::MAX,
                ..Default::default()
            },
            memory_limit: foundry_config::Config::default().memory_limit,
            ..Default::default()
        };

        let env_result = evm_opts.evm_env_blocking();
        let envi = match env_result {
            Ok(envi) => envi,
            Err(err) => {
                eprintln!("Failed to instantiate forked environment: {}", err);
                return Err(CustomRejection::FailedInstantiateFork);
            }
        };

        let fork_opts = CreateFork {
            url: fork_url.clone(),
            enable_caching: true,
            env: envi,
            evm_opts: evm_opts.clone(),
        };

        let db = Backend::spawn(Some(fork_opts.clone()));

        let builder = ExecutorBuilder::default()
            .with_gas_limit(gas_limit.into())
            .set_tracing(tracing);

        let executor = if let Some(env) = env {
            builder.with_config(env).build(db)
        } else {
            builder.with_config(fork_opts.env.clone()).build(db)
        };

        let foundry_config = foundry_config::Config {
            etherscan_api_key: etherscan_key,
            ..Default::default()
        };

        let chain: Chain = fork_opts.env.cfg.chain_id.to::<u64>().into();
        let etherscan_identifier = EtherscanIdentifier::new(&foundry_config, Some(chain)).ok();
        let mut decoder = CallTraceDecoderBuilder::new().with_verbosity(5).build();

        if
            let Ok(identifier) = SignaturesIdentifier::new(
                foundry_config::Config::foundry_cache_dir(),
                false
            )
        {
            decoder.add_signature_identifier(identifier);
        }

        Ok(Evm {
            executor,
            decoder,
            etherscan_identifier,
        })
    }

    pub async fn call_raw(
        &mut self,
        call: CallRawRequest
    ) -> Result<CallRawResult, CustomRejection> {
        self.set_access_list(call.access_list);
        let res = self.executor
            .call_raw(
                call.from,
                call.to,
                call.data.map(|d| d.0).unwrap_or_default(),
                call.value.unwrap_or_default()
            )
            .map_err(|err| {
                dbg!(&err);
                EvmError(err)
            })?;

        let formatted_trace = if call.format_trace {
            let mut output = String::new();
            for trace in &mut res.traces.clone() {
                if let Some(identifier) = &mut self.etherscan_identifier {
                    self.decoder.identify(trace, identifier);
                }
                self.decoder.decode(trace).await;
                output.push_str(&format!("{trace}"));
            }
            Some(output)
        } else {
            None
        };

        Ok(CallRawResult {
            gas_used: res.gas_used,
            block_number: res.env.block.number.to(),
            success: !res.reverted,
            trace: res.traces,
            logs: res.logs,
            exit_reason: res.exit_reason,
            return_data: Bytes(res.result),
            formatted_trace,
        })
    }

    pub fn override_account(
        &mut self,
        address: Address,
        balance: Option<Uint>,
        nonce: Option<u64>,
        code: Option<Bytes>,
        storage: Option<StorageOverride>
    ) -> Result<(), CustomRejection> {
        let address = h160_to_b160(address);
        let mut account = Account {
            info: self.executor
                .backend()
                .basic(address)
                .map_err(|_| CustomRejection::OverrideError)?
                .unwrap_or_default(),
            ..Account::new_not_existing()
        };
        if let Some(balance) = balance {
            account.info.balance = u256_to_ru256(balance);
        }
        if let Some(nonce) = nonce {
            account.info.nonce = nonce;
        }
        if let Some(code) = code {
            account.info.code = Some(Bytecode::new_raw(code.to_vec().into()));
        }
        if let Some(storage) = storage {
            self.handle_storage_override(&mut account, storage)?;
        }
        self.executor.backend_mut().commit([(address, account)].into_iter().collect());

        Ok(())
    }

    fn handle_storage_override(
        &self,
        account: &mut Account,
        storage: StorageOverride
    ) -> Result<(), CustomRejection> {
        account.storage_cleared = !storage.diff;
        account.storage.extend(
            storage.slots
                .into_iter()
                .map(|(key, value)| {
                    (
                        u256_to_ru256(Uint::from_big_endian(key.as_bytes())),
                        StorageSlot::new(u256_to_ru256(value)),
                    )
                })
        );
        Ok(())
    }

    pub async fn call_raw_committing(
        &mut self,
        call: CallRawRequest,
        gas_limit: u64
    ) -> Result<CallRawResult, CustomRejection> {
        self.executor.set_gas_limit(gas_limit.into());
        self.set_access_list(call.access_list);
        let res = self.executor
            .call_raw_committing(
                call.from,
                call.to,
                call.data.unwrap_or_default().0,
                call.value.unwrap_or_default()
            )
            .map_err(|err| {
                dbg!(&err);
                EvmError(err)
            })?;

        let formatted_trace = if call.format_trace {
            let mut output = String::new();
            for trace in &mut res.traces.clone() {
                if let Some(identifier) = &mut self.etherscan_identifier {
                    self.decoder.identify(trace, identifier);
                }
                self.decoder.decode(trace).await;
                output += &format!("{:?}", trace);
            }
            Some(output)
        } else {
            None
        };

        Ok(CallRawResult {
            gas_used: res.gas_used,
            block_number: res.env.block.number.to(),
            success: !res.reverted,
            trace: res.traces.clone(), // Clonazione potrebbe essere evitata se non è necessaria
            logs: res.logs,
            exit_reason: res.exit_reason,
            return_data: Bytes(res.result),
            formatted_trace,
        })
    }

    pub async fn set_block(&mut self, number: u64) -> Result<(), CustomRejection> {
        self.executor.env_mut().block.number = Uint::from(number).into();
        Ok(())
    }

    pub fn get_block(&self) -> Uint {
        self.executor.env().block.number.into()
    }

    pub async fn set_block_timestamp(&mut self, timestamp: u64) -> Result<(), CustomRejection> {
        self.executor.env_mut().block.timestamp = Uint::from(timestamp).into();
        Ok(())
    }

    pub fn get_block_timestamp(&self) -> Uint {
        self.executor.env().block.timestamp.into()
    }

    pub fn get_chain_id(&self) -> Uint {
        self.executor.env().cfg.chain_id.into()
    }

    fn set_access_list(&mut self, access_list: Option<AccessList>) {
        self.executor.env_mut().tx.access_list = access_list
            .unwrap_or_default()
            .0.into_iter()
            .map(|item| {
                (
                    h160_to_b160(item.address),
                    item.storage_keys
                        .into_iter()
                        .map(|key| u256_to_ru256(Uint::from_big_endian(key.as_bytes())))
                        .collect(),
                )
            })
            .collect();
    }
}
