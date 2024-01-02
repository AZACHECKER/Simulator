use std::collections::HashMap;
use ethers::abi::{ Address, Hash, Uint };
use ethers::core::types::Log;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::Bytes;
use foundry_evm::executor::Executor;
use foundry_evm::trace::identifier::EtherscanIdentifier;
use foundry_evm::trace::{ CallTraceArena, CallTraceDecoder };
use revm::interpreter::InstructionResult;

#[derive(Debug, Clone)]
pub struct CallRawRequest {
    pub from: Address,
    pub to: Address,
    pub value: Option<Uint>,
    pub data: Option<Bytes>,
    pub access_list: Option<AccessList>,
    pub format_trace: bool,
}

#[derive(Debug, Clone)]
pub struct CallRawResult {
    pub gas_used: u64,
    pub block_number: u64,
    pub success: bool,
    pub trace: Option<CallTraceArena>,
    pub logs: Vec<Log>,
    pub exit_reason: InstructionResult,
    pub return_data: Bytes,
    pub formatted_trace: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StorageOverride {
    pub slots: HashMap<Hash, Uint>,
    pub diff: bool,
}

pub struct Evm {
    pub executor: Executor,
    pub decoder: CallTraceDecoder,
    pub etherscan_identifier: Option<EtherscanIdentifier>,
}
