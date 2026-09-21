use std::time::Duration;

use reqwest::header::{HeaderMap, RETRY_AFTER};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("request timed out after {timeout_secs}s")]
    Timeout {
        #[source]
        source: reqwest::Error,
        timeout_secs: f64,
    },
    #[error("connection error: {0}")]
    Connection(#[from] reqwest::Error),
    #[error("response decoding failed: {0}")]
    ResponseDecode(#[from] serde_json::Error),
    #[error("API request failed: {0}")]
    Api(Box<ApiError>),
}

impl Error {
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(error) => Some(error.status),
            _ => None,
        }
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout { .. })
    }

    pub fn is_rate_limit(&self) -> bool {
        self.status() == Some(429)
    }
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub body: Box<ErrorBody>,
    pub headers: Box<HeaderMap>,
    pub endpoint: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.endpoint, self.status)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn request_id(&self) -> Option<&str> {
        self.headers.get("x-typesafe-request-id")?.to_str().ok()
    }

    pub fn retry_after(&self) -> Option<Duration> {
        parse_retry_after(&self.headers)
    }
}

#[derive(Debug, Clone)]
pub enum ErrorBody {
    Json(Value),
    Text(String),
    Empty,
}

impl ErrorBody {
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            ErrorBody::Json(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ErrorBody::Text(text) => Some(text),
            _ => None,
        }
    }
}

pub(crate) fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    if let Some(value) = headers.get("retry-after-ms")
        && let Ok(value) = value.to_str()
        && let Ok(ms) = value.trim().parse::<u64>()
    {
        return Some(Duration::from_millis(ms));
    }

    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = httpdate::parse_http_date(value).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(
        date.duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .saturating_sub(now),
    )
}
