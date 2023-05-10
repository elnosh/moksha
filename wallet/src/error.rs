use std::string::FromUtf8Error;

use reqwest::header::InvalidHeaderValue;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CashuWalletError {
    #[error("SerdeJsonError - {0}")]
    Json(#[from] serde_json::Error),

    #[error("ReqwestError - {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("InvalidHeaderValueError - {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error("MintError - {0} - {0}")]
    MintError(u64, String),

    #[error("UnexpectedResponse - {0}")]
    UnexpectedResponse(String),

    #[error("CashuCoreError - {0}")]
    CashuCore(#[from] cashurs_core::error::CashuCoreError),

    #[error("DB Error {0}")]
    Db(#[from] rocksdb::Error),

    #[error("Utf8 Error {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error("Invalid Proofs")]
    InvalidProofs,
}
