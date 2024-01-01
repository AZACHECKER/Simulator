use eyre::Report;
use serde::{ Deserialize, Serialize };
use std::{ convert::Infallible };

use warp::{ hyper::StatusCode, reject::{ Reject, Rejection } };
use warp::Reply;

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

#[derive(Debug)]
pub enum CustomRejection {
    NotFound,
    StateNotFound,
    NoURLForChainId,
    IncorrectChainId,
    MultipleChainIds,
    MultipleBlockNumbers,
    InvalidBlockNumbers,
    OverrideError,
    EvmError(Report),
    BodyDeserializeError,
    MethodNotAllowed,
    MissingHeader,
    FailedLock,
    FailedInstantiateFork,
    UnhandledRejection,
}

impl Reject for CustomRejection {}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = match err.find::<CustomRejection>() {
        Some(CustomRejection::NotFound) => (StatusCode::NOT_FOUND, "NOT_FOUND".to_string()),
        Some(CustomRejection::StateNotFound) =>
            (StatusCode::NOT_FOUND, "STATE_NOT_FOUND".to_string()),
        Some(CustomRejection::NoURLForChainId) =>
            (StatusCode::BAD_REQUEST, "CHAIN_ID_NOT_SUPPORTED".to_string()),
        Some(CustomRejection::IncorrectChainId) =>
            (StatusCode::BAD_REQUEST, "INCORRECT_CHAIN_ID".to_string()),
        Some(CustomRejection::MultipleChainIds) =>
            (StatusCode::BAD_REQUEST, "MULTIPLE_CHAIN_IDS".to_string()),
        Some(CustomRejection::MultipleBlockNumbers) =>
            (StatusCode::BAD_REQUEST, "MULTIPLE_BLOCK_NUMBERS".to_string()),
        Some(CustomRejection::InvalidBlockNumbers) =>
            (StatusCode::BAD_REQUEST, "INVALID_BLOCK_NUMBERS".to_string()),
        Some(CustomRejection::OverrideError) =>
            (StatusCode::INTERNAL_SERVER_ERROR, "OVERRIDE_ERROR".to_string()),
        Some(CustomRejection::FailedLock) =>
            (StatusCode::INTERNAL_SERVER_ERROR, "FAILED_LOCK".to_string()),
        Some(CustomRejection::FailedInstantiateFork) =>
            (StatusCode::INTERNAL_SERVER_ERROR, "FAILED_INSTANTIATE_FORK".to_string()),
        Some(CustomRejection::EvmError(report)) => {
            if report.to_string().contains("CallGasCostMoreThanGasLimit") {
                (StatusCode::BAD_REQUEST, "OUT_OF_GAS".to_string())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "EVM_ERROR".to_string())
            }
        }
        Some(CustomRejection::BodyDeserializeError) =>
            (StatusCode::BAD_REQUEST, "BAD_REQUEST".to_string()),
        Some(CustomRejection::MethodNotAllowed) =>
            (StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED".to_string()),
        Some(CustomRejection::MissingHeader) =>
            (StatusCode::UNAUTHORIZED, "UNAUTHORIZED".to_string()),
        _ => {
            eprintln!("unhandled rejection: {:?}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "UNHANDLED_REJECTION".to_string())
        }
    };

    let json = warp::reply::json(
        &(ErrorMessage {
            code: code.as_u16(),
            message,
        })
    );

    Ok(warp::reply::with_status(json, code))
}
