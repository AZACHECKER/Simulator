use serde::{Deserialize, Serialize};
use eyre::Report;

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

#[derive(Debug)]
pub struct NoURLForChainIdError;

#[derive(Debug)]
pub struct IncorrectChainIdError();

#[derive(Debug)]
pub struct MultipleChainIdsError();

#[derive(Debug)]
pub struct MultipleBlockNumbersError();

#[derive(Debug)]
pub struct InvalidBlockNumbersError();

#[derive(Debug)]
pub struct StateNotFound();

#[derive(Debug)]
pub struct OverrideError;

#[derive(Debug)]
pub struct EvmError(pub Report);

#[derive(Debug)]
pub struct FailedInstantiateFork;

#[derive(Debug)]
pub struct FailedToSetBlockTimestamp;