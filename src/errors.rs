
use std::{
    convert::Infallible,
    error::Error,
};

use warp::{body::BodyDeserializeError, hyper::StatusCode, reject::Reject, Rejection, Reply};

use crate::structs::{
    ErrorMessage,
    NoURLForChainIdError,
    IncorrectChainIdError,
    MultipleChainIdsError,
    MultipleBlockNumbersError,
    InvalidBlockNumbersError,
    StateNotFound,
    OverrideError,
    EvmError,
    FailedInstantiateFork,
    FailedToSetBlockTimestamp,
};

impl Reject for NoURLForChainIdError {}

impl Reject for IncorrectChainIdError {}

impl Reject for MultipleChainIdsError {}

impl Reject for MultipleBlockNumbersError {}

impl Reject for InvalidBlockNumbersError {}

impl Reject for StateNotFound {}

impl Reject for OverrideError {}

impl Reject for EvmError {}

impl Reject for FailedInstantiateFork {}

impl Reject for FailedToSetBlockTimestamp {}

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
        e if e.find::<warp::reject::InvalidHeader>().is_some() => (StatusCode::BAD_REQUEST, "INVALID_HEADER".to_string()),
        e if e.find::<FailedToSetBlockTimestamp>().is_some() => (StatusCode::INTERNAL_SERVER_ERROR, "FAILED_TO_SET_BLOCK_TIMESTAMP".to_string()),
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
