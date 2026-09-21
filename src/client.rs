use std::collections::HashMap;
use std::env;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::{ApiError, Error, ErrorBody, parse_retry_after};
use crate::retry::RetryPolicy;
use crate::types::{
    EvaluationResponse, JsonContent, ListModelsResponse, Question, QuestionMap, validate_state,
};
use crate::{API_KEY_ENV, BASE_URL_ENV, DEFAULT_MODEL_ENV};

const SYSTEM_ONE_PATH: &str = "/v1/systemone";
const MODELS_PATH: &str = "/v1/models";

/// A complete System One request body.
///
/// The same keys used in `questions` are preserved in the response's answers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SystemOneRequest {
    /// Text or structured content to evaluate.
    pub state: JsonContent,
    /// Named questions; keys are returned unchanged in the answer map.
    pub questions: QuestionMap,
}

/// Fluent builder for validating and assembling a [`SystemOneRequest`].
#[derive(Debug, Clone, Default)]
pub struct SystemOneRequestBuilder {
    state: Option<JsonContent>,
    questions: QuestionMap,
}

impl SystemOneRequestBuilder {
    /// Creates an empty builder.
    ///
    /// A request is not valid until [`state`](Self::state) and at least one
    /// [`question`](Self::question) have been added.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the state to evaluate.
    ///
    /// The value must be a string, JSON object, or JSON array.
    pub fn state(mut self, state: impl Into<JsonContent>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Adds or replaces a question under a caller-chosen identifier.
    ///
    /// Identifiers do not affect inference; they are labels used to correlate
    /// requests with answers.
    pub fn question(mut self, id: impl Into<String>, question: Question) -> Self {
        self.questions.insert(id.into(), question);
        self
    }

    /// Validates limits and JSON shapes, then builds the request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRequest`] when state is missing or malformed,
    /// no questions are present, or a question violates TypeSafe's constraints.
    pub fn build(self) -> Result<SystemOneRequest, Error> {
        let state = self.state.ok_or_else(|| {
            Error::InvalidRequest("state is required before building a request".into())
        })?;
        let request = SystemOneRequest::new(state, self.questions);
        request.validate()?;
        Ok(request)
    }
}

impl SystemOneRequest {
    /// Creates a request without validating it.
    ///
    /// Prefer [`SystemOneRequestBuilder::build`] for a checked constructor.
    /// [`TypeSafeClient::evaluate_with`] validates again before sending.
    pub fn new(state: impl Into<JsonContent>, questions: QuestionMap) -> Self {
        Self {
            state: state.into(),
            questions,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        validate_state(&self.state)
            .map_err(|message| Error::InvalidRequest(format!("state {message}")))?;
        if self.questions.is_empty() {
            return Err(Error::InvalidRequest(
                "at least one question is required".into(),
            ));
        }
        for (id, question) in &self.questions {
            question
                .validate()
                .map_err(|message| Error::InvalidRequest(format!("question `{id}`: {message}")))?;
        }
        Ok(())
    }
}

/// Per-call overrides applied on top of the client configuration.
#[derive(Debug, Clone, Default)]
pub struct EvaluateOptions {
    /// Overrides the client's default model for this request.
    pub model: Option<String>,
    /// Replaces the client retry policy for this request.
    pub retry: Option<RetryPolicy>,
    /// Overrides the timeout for a single HTTP operation.
    pub timeout: Option<Duration>,
    /// Additional headers. `Authorization` remains SDK-managed.
    pub extra_headers: HashMap<String, String>,
    /// Extra top-level JSON fields, shallow-merged after SDK-owned fields.
    pub extra_body: Map<String, Value>,
}

impl EvaluateOptions {
    /// Creates default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the model that will handle this evaluation.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Replaces the effective retry policy.
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Overrides the timeout for this operation.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Adds a custom header; later calls replace values with the same name.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(name.into(), value.into());
        self
    }

    /// Adds a top-level request field, replacing SDK fields on collision.
    pub fn extra_body(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra_body.insert(name.into(), value.into());
        self
    }
}

/// Options accepted by [`TypeSafeClient::list_models_with`].
///
/// Retry, timeout, and headers apply. `extra_body` has no effect on `GET`
/// model catalogue requests.
pub type ListModelsOptions = EvaluateOptions;

/// Configured asynchronous client for TypeSafe's public API.
///
/// Cloning shares the underlying connection pool and configuration. Debug
/// output intentionally omits the API key.
#[derive(Clone)]
pub struct TypeSafeClient {
    api_key: String,
    base_url: String,
    model: String,
    retry: RetryPolicy,
    timeout: Option<Duration>,
    http: reqwest::Client,
}

impl std::fmt::Debug for TypeSafeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeSafeClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("retry", &self.retry)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// Builder for [`TypeSafeClient`].
#[derive(Debug, Clone)]
pub struct TypeSafeClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    retry: RetryPolicy,
    timeout: Option<Duration>,
}

impl Default for TypeSafeClientBuilder {
    /// Defaults to the production API root, `jev-latest`, standard retry
    /// behavior, and a 10-second HTTP timeout.
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            model: None,
            retry: RetryPolicy::default(),
            timeout: Some(Duration::from_secs(10)),
        }
    }
}

impl TypeSafeClientBuilder {
    /// Sets the API key used for bearer authentication.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the API root without a trailing slash.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Sets the default model used when an evaluation supplies no override.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the client-level retry policy.
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Sets the default HTTP operation timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Removes the client-level HTTP timeout.
    pub fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Builds a client with explicit settings taking precedence over
    /// environment variables.
    ///
    /// Reads [`API_KEY_ENV`], [`BASE_URL_ENV`], and [`DEFAULT_MODEL_ENV`]
    /// only when the corresponding explicit value is absent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] when required settings are absent or
    /// invalid.
    pub fn from_env(self) -> Result<TypeSafeClient, Error> {
        let api_key = self
            .api_key
            .or_else(|| non_empty_env(API_KEY_ENV))
            .ok_or_else(|| Error::Configuration(format!("{} is not set", crate::API_KEY_ENV)))?;
        let base_url = self
            .base_url
            .or_else(|| non_empty_env(BASE_URL_ENV))
            .unwrap_or_else(|| crate::API_BASE_URL.to_string());
        let model = self
            .model
            .or_else(|| non_empty_env(DEFAULT_MODEL_ENV))
            .unwrap_or_else(|| crate::DEFAULT_MODEL.to_string());
        TypeSafeClient::build_inner(api_key, base_url, model, self.retry, self.timeout)
    }

    /// Builds a client from explicit builder values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] for a missing API key, empty model, or
    /// invalid base URL.
    pub fn build(self) -> Result<TypeSafeClient, Error> {
        let api_key = self
            .api_key
            .ok_or_else(|| Error::Configuration("API key is required".into()))?;
        let base_url = self
            .base_url
            .unwrap_or_else(|| crate::API_BASE_URL.to_string());
        let model = self
            .model
            .unwrap_or_else(|| crate::DEFAULT_MODEL.to_string());
        TypeSafeClient::build_inner(api_key, base_url, model, self.retry, self.timeout)
    }
}

impl TypeSafeClient {
    /// Returns a builder using SDK defaults.
    pub fn builder() -> TypeSafeClientBuilder {
        TypeSafeClientBuilder::default()
    }

    /// Creates a client with the production base URL, `jev-latest`, standard
    /// retries, and a 10-second timeout.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] when the key is blank or the HTTP
    /// client cannot be initialized.
    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder().api_key(api_key).build()
    }

    /// Equivalent to [`TypeSafeClientBuilder::from_env`] with default settings.
    pub fn from_env() -> Result<Self, Error> {
        Self::builder().from_env()
    }

    fn build_inner(
        api_key: String,
        base_url: String,
        model: String,
        retry: RetryPolicy,
        timeout: Option<Duration>,
    ) -> Result<Self, Error> {
        if api_key.trim().is_empty() {
            return Err(Error::Configuration("API key must not be empty".into()));
        }
        if model.trim().is_empty() {
            return Err(Error::Configuration("model must not be empty".into()));
        }

        let mut builder = reqwest::Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .use_rustls_tls();
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        let http = builder.build().map_err(Error::Connection)?;

        Ok(Self {
            api_key,
            base_url: normalize_base_url(base_url)?,
            model,
            retry,
            timeout,
            http,
        })
    }

    /// Alias for [`TypeSafeClient::evaluate`], named after the API resource.
    pub async fn system_one(
        &self,
        state: impl Into<JsonContent>,
        questions: QuestionMap,
    ) -> Result<EvaluationResponse, Error> {
        self.evaluate(state, questions).await
    }

    /// Evaluates a state against all questions in a single API call.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRequest`] before sending when validation fails;
    /// transport, HTTP, and response decoding failures use the corresponding
    /// [`Error`] variants.
    pub async fn evaluate(
        &self,
        state: impl Into<JsonContent>,
        questions: QuestionMap,
    ) -> Result<EvaluationResponse, Error> {
        self.evaluate_with(
            SystemOneRequest::new(state, questions),
            EvaluateOptions::new(),
        )
        .await
    }

    /// Evaluates a prebuilt request with request-specific options.
    ///
    /// The request is validated before the HTTP call. `extra_body` values are
    /// shallow-merged last and can override `state`, `questions`, or `model`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] when TypeSafe returns a non-success status after
    /// retries.
    pub async fn evaluate_with(
        &self,
        request: SystemOneRequest,
        options: EvaluateOptions,
    ) -> Result<EvaluationResponse, Error> {
        request.validate()?;
        let model = options.model.clone().unwrap_or_else(|| self.model.clone());
        if model.trim().is_empty() {
            return Err(Error::Configuration("model must not be empty".into()));
        }

        let mut body = serde_json::to_value(&request)?;
        let object = body.as_object_mut().ok_or_else(|| {
            Error::InvalidRequest("SystemOneRequest did not serialize to an object".into())
        })?;
        object.insert("model".into(), Value::String(model));
        let extra_body = options.extra_body.clone();
        for (key, value) in extra_body {
            object.insert(key, value);
        }

        let response = self
            .send_with_retries(
                self.http
                    .post(format!("{}{SYSTEM_ONE_PATH}", self.base_url))
                    .json(&body),
                &options,
            )
            .await?;
        tracing::debug!("evaluation request completed");
        decode_response(response).await
    }

    /// Lists models and aliases available to the authenticated account.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success HTTP status after retries.
    pub async fn list_models(&self) -> Result<ListModelsResponse, Error> {
        self.list_models_with(EvaluateOptions::new()).await
    }

    /// Lists models with request-specific retry, timeout, and header options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] for a non-success HTTP status after retries.
    pub async fn list_models_with(
        &self,
        options: ListModelsOptions,
    ) -> Result<ListModelsResponse, Error> {
        let response = self
            .send_with_retries(
                self.http.get(format!("{}{MODELS_PATH}", self.base_url)),
                &options,
            )
            .await?;
        tracing::debug!("model catalogue request completed");
        decode_response(response).await
    }

    async fn send_with_retries(
        &self,
        mut request: reqwest::RequestBuilder,
        options: &EvaluateOptions,
    ) -> Result<reqwest::Response, Error> {
        let retry = options.retry.clone().unwrap_or_else(|| self.retry.clone());
        for (name, value) in &options.extra_headers {
            request = request.header(name, value);
        }
        request = request
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(timeout) = options.timeout.or(self.timeout) {
            request = request.timeout(timeout);
        }

        let mut attempt = 0;
        loop {
            let result = request
                .try_clone()
                .ok_or_else(|| {
                    Error::InvalidRequest("request could not be cloned for retry".into())
                })?
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if retry.should_retry_status(status.as_u16()) && attempt < retry.max_retries {
                        let headers = response.headers().clone();
                        response.bytes().await.map_err(Error::Connection)?;
                        let delay = retry.delay(attempt, parse_retry_after(&headers));
                        tracing::debug!(
                            status = status.as_u16(),
                            attempt,
                            ?delay,
                            "retrying TypeSafe API request"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Ok(response);
                }
                Err(error) => {
                    let transient_timeout = retry.retry_timeout_errors && error.is_timeout();
                    let transient_connection = retry.retry_connection_errors
                        && !error.is_timeout()
                        && (error.is_connect() || error.is_request());
                    if (transient_timeout || transient_connection) && attempt < retry.max_retries {
                        let delay = retry.delay(attempt, None);
                        tracing::debug!(attempt, ?delay, "retrying TypeSafe transport error");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    let timeout = options.timeout.or(self.timeout);
                    if error.is_timeout() {
                        return Err(Error::Timeout {
                            source: error,
                            timeout_secs: timeout.map_or(0.0, |duration| duration.as_secs_f64()),
                        });
                    }
                    return Err(Error::Connection(error));
                }
            }
        }
    }
}

async fn decode_response<T: DeserializeOwned + HaveRequestId>(
    response: reqwest::Response,
) -> Result<T, Error> {
    let headers = response.headers().clone();
    let endpoint = format!(
        "{} {}",
        response.url().host_str().unwrap_or_default(),
        response.url().path()
    );
    let status = response.status();
    let bytes = response.bytes().await.map_err(Error::Connection)?;

    if !status.is_success() {
        let body = decode_error_body(&bytes);
        return Err(Error::Api(Box::new(ApiError {
            status: status.as_u16(),
            body: Box::new(body),
            headers: Box::new(headers),
            endpoint,
        })));
    }

    let mut value: T = serde_json::from_slice(&bytes).map_err(Error::ResponseDecode)?;
    value.set_request_id(
        headers
            .get("x-typesafe-request-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
    );
    Ok(value)
}

trait HaveRequestId {
    fn set_request_id(&mut self, value: Option<String>);
}

impl HaveRequestId for EvaluationResponse {
    fn set_request_id(&mut self, value: Option<String>) {
        self.request_id = value;
    }
}

impl HaveRequestId for ListModelsResponse {
    fn set_request_id(&mut self, value: Option<String>) {
        self.request_id = value;
    }
}

fn decode_error_body(bytes: &[u8]) -> ErrorBody {
    if bytes.is_empty() {
        return ErrorBody::Empty;
    }
    match serde_json::from_slice(bytes) {
        Ok(value) => ErrorBody::Json(value),
        Err(_) => ErrorBody::Text(String::from_utf8_lossy(bytes).into_owned()),
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn normalize_base_url(base_url: String) -> Result<String, Error> {
    let base_url = base_url.trim().trim_end_matches('/').to_owned();
    if base_url.is_empty() {
        return Err(Error::Configuration("base URL must not be empty".into()));
    }
    reqwest::Url::parse(&base_url)
        .map_err(|_| Error::Configuration(format!("invalid base URL: {base_url}")))?;
    Ok(base_url)
}
