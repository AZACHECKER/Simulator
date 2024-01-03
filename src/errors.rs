use eyre::Report;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    error::Error,
};

use warp::{body::BodyDeserializeError, hyper::StatusCode, reject::Reject, Rejection, Reply};


#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

#[derive(Debug)]
pub struct NoURLForChainIdError;

impl Reject for NoURLForChainIdError {}


#[derive(Debug)]
pub struct IncorrectChainIdError();

impl Reject for IncorrectChainIdError {}


#[derive(Debug)]
pub struct MultipleChainIdsError();

impl Reject for MultipleChainIdsError {}

#[derive(Debug)]
pub struct MultipleBlockNumbersError();

impl Reject for MultipleBlockNumbersError {}


#[derive(Debug)]
pub struct InvalidBlockNumbersError();

impl Reject for InvalidBlockNumbersError {}


#[derive(Debug)]
pub struct StateNotFound();

impl Reject for StateNotFound {}


#[derive(Debug)]
pub struct OverrideError;

impl Reject for OverrideError {}


#[derive(Debug)]
pub struct EvmError(pub Report);

impl Reject for EvmError {}


#[derive(Debug)]
pub struct FailedInstantiateFork;

impl Reject for FailedInstantiateFork {}


pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = match err {
        e if e.is_not_found() => (StatusCode::NOT_FOUND, "NOT_FOUND".to_string()),
        e if e.find::<StateNotFound>().is_some() => (StatusCode::NOT_FOUND, "STATE_NOT_FOUND".to_string()),
        e if e.find::<NoURLForChainIdError>().is_some() => (StatusCode::BAD_REQUEST, "CHAIN_ID_NOT_SUPPORTED".to_string()),
        e if e.find::<IncorrectChainIdError>().is_some() => (StatusCode::BAD_REQUEST, "INCORRECT_CHAIN_ID".to_string()),
        e if e.find::<MultipleChainIdsError>().is_some() => (StatusCode::BAD_REQUEST, "MULTIPLE_CHAIN_IDS".to_string()),
        e if e.find::<MultipleBlockNumbersError>().is_some() => (StatusCode::BAD_REQUEST, "MULTIPLE_BLOCK_NUMBERS".to_string()),
        e if e.find::<InvalidBlockNumbersError>().is_some() => (StatusCode::BAD_REQUEST, "INVALID_BLOCK_NUMBERS".to_string()),
        e if e.find::<BodyDeserializeError>().is_some() => {
            let cause = e.find::<BodyDeserializeError>().unwrap().source().map(|cause| format!("{}", cause)).unwrap_or_default();
            (StatusCode::BAD_REQUEST, format!("BAD REQUEST: {}", cause))
        }
        e if e.find::<warp::reject::MethodNotAllowed>().is_some() => (StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED".to_string()),
        e if e.find::<warp::reject::MissingHeader>().is_some() => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED".to_string()),
        e if e.find::<FailedInstantiateFork>().is_some() => (StatusCode::INTERNAL_SERVER_ERROR, "FAILED_INSTANTIATE_FORK".to_string()),
        //invalid header 
        e if e.find::<warp::reject::InvalidHeader>().is_some() => (StatusCode::BAD_REQUEST, "INVALID_HEADER".to_string()),
        //evm error
        e if e.find::<EvmError>().is_some() => {
            let (code, message);
            if e.find::<EvmError>().unwrap().0.to_string().contains("CallGasCostMoreThanGasLimit") {
                code = StatusCode::BAD_REQUEST;
                message = "OUT_OF_GAS".to_string();
            } else {
                code = StatusCode::INTERNAL_SERVER_ERROR;
                message = "EVM_ERROR".to_string();
            }

            (code, message)
        }
        _ => {
            eprintln!("Unhandled rejection: {:?}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "UNHANDLED_REJECTION".to_string())
        }
    };

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}
