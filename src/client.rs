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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SystemOneRequest {
    pub state: JsonContent,
    pub questions: QuestionMap,
}

#[derive(Debug, Clone, Default)]
pub struct SystemOneRequestBuilder {
    state: Option<JsonContent>,
    questions: QuestionMap,
}

impl SystemOneRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(mut self, state: impl Into<JsonContent>) -> Self {
        self.state = Some(state.into());
        self
    }

    pub fn question(mut self, id: impl Into<String>, question: Question) -> Self {
        self.questions.insert(id.into(), question);
        self
    }

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

#[derive(Debug, Clone, Default)]
pub struct EvaluateOptions {
    pub model: Option<String>,
    pub retry: Option<RetryPolicy>,
    pub timeout: Option<Duration>,
    pub extra_headers: HashMap<String, String>,
    pub extra_body: Map<String, Value>,
}

impl EvaluateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(name.into(), value.into());
        self
    }

    pub fn extra_body(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.extra_body.insert(name.into(), value.into());
        self
    }
}

pub type ListModelsOptions = EvaluateOptions;

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

#[derive(Debug, Clone)]
pub struct TypeSafeClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    retry: RetryPolicy,
    timeout: Option<Duration>,
}

impl Default for TypeSafeClientBuilder {
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
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

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
    pub fn builder() -> TypeSafeClientBuilder {
        TypeSafeClientBuilder::default()
    }

    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder().api_key(api_key).build()
    }

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

    pub async fn system_one(
        &self,
        state: impl Into<JsonContent>,
        questions: QuestionMap,
    ) -> Result<EvaluationResponse, Error> {
        self.evaluate(state, questions).await
    }

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

    pub async fn list_models(&self) -> Result<ListModelsResponse, Error> {
        self.list_models_with(EvaluateOptions::new()).await
    }

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
