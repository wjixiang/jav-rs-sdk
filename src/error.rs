use std::time::Duration;

use reqwest::header::{HeaderMap, RETRY_AFTER};
use serde_json::Value;
use thiserror::Error;

/// SDK error type covering local validation, transport, HTTP, and decoding
/// failures.
#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    /// Client construction or configuration failed.
    Configuration(String),
    #[error("invalid request: {0}")]
    /// The SDK rejected the request before sending it.
    InvalidRequest(String),
    #[error("request timed out after {timeout_secs}s")]
    /// An HTTP operation exceeded its configured timeout.
    Timeout {
        /// Underlying HTTP client error.
        #[source]
        source: reqwest::Error,
        /// Requested timeout in seconds; zero means no timeout was configured.
        timeout_secs: f64,
    },
    #[error("connection error: {0}")]
    /// A connection or HTTP request failed without a usable API response.
    Connection(#[from] reqwest::Error),
    #[error("response decoding failed: {0}")]
    /// A successful HTTP response contained invalid or unexpected JSON.
    ResponseDecode(#[from] serde_json::Error),
    #[error("API request failed: {0}")]
    /// TypeSafe returned a non-success HTTP response after retries.
    Api(Box<ApiError>),
}

impl Error {
    /// Returns the HTTP status for API errors, or `None` for local and
    /// transport failures.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(error) => Some(error.status),
            _ => None,
        }
    }

    /// Returns whether the failure was an HTTP operation timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout { .. })
    }

    /// Returns whether TypeSafe returned HTTP 429 Too Many Requests.
    pub fn is_rate_limit(&self) -> bool {
        self.status() == Some(429)
    }
}

/// Details retained from a non-success TypeSafe HTTP response.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// HTTP response status code.
    pub status: u16,
    /// Parsed JSON, plain text, or empty response body.
    pub body: Box<ErrorBody>,
    /// Response headers.
    pub headers: Box<HeaderMap>,
    /// Endpoint without query parameters or credentials.
    pub endpoint: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.endpoint, self.status)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    /// Returns TypeSafe's `x-typesafe-request-id` header when present.
    pub fn request_id(&self) -> Option<&str> {
        self.headers.get("x-typesafe-request-id")?.to_str().ok()
    }

    /// Parses retry delay headers, preferring `retry-after-ms`.
    ///
    /// Supports integer seconds and HTTP dates in `Retry-After`.
    pub fn retry_after(&self) -> Option<Duration> {
        parse_retry_after(&self.headers)
    }
}

/// Representation of an unsuccessful response body.
#[derive(Debug, Clone)]
pub enum ErrorBody {
    /// The service returned a JSON object or array.
    Json(Value),
    /// The service returned non-JSON text.
    Text(String),
    /// The response had no body.
    Empty,
}

impl ErrorBody {
    /// Returns the parsed JSON body, if any.
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            ErrorBody::Json(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the raw text body, if any.
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
