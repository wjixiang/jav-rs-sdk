use async_trait::async_trait;

use crate::Result;
use crate::client::SystemOneRequest;
use crate::types::{EvaluationResponse, ListModelsResponse};

pub const MODEL_JEV_LATEST: &str = "jev-latest";
pub const MODEL_JEV_PREVIEW: &str = "jev-preview";
pub const MODEL_JEV_1_13_0: &str = "jev-1.13.0";

pub struct TypeSafeProvider;

pub struct JevProvider;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelPreset {
    pub name: &'static str,
    pub aliases: Vec<String>,
    pub context_length: u64,
    pub max_state_length: u64,
    pub input_price_per_mtok: f64,
    pub output_price_per_mtok: f64,
}

impl TypeSafeProvider {
    pub const NAME: &'static str = "typesafe";
    pub const DEFAULT_BASE_URL: &'static str = crate::API_BASE_URL;
    pub const AUTH_HEADER: &'static str = "Authorization";
    pub const AUTH_SCHEME: &'static str = "Bearer";
}

impl JevProvider {
    pub fn preset_models() -> Vec<ModelPreset> {
        vec![ModelPreset {
            name: MODEL_JEV_1_13_0,
            aliases: vec![MODEL_JEV_LATEST.to_owned(), MODEL_JEV_PREVIEW.to_owned()],
            context_length: 64_000,
            max_state_length: 32_000,
            input_price_per_mtok: 0.042,
            output_price_per_mtok: 0.0,
        }]
    }
}

/// Minimal integration seam for applications that must be independent of the
/// concrete HTTP client. TypeSafe's endpoint evaluates many typed questions in
/// one call, so unlike chat-provider traits this intentionally has no generic
/// text generation method.
#[async_trait]
pub trait SystemOneProvider: Send + Sync {
    async fn evaluate(&self, request: SystemOneRequest) -> Result<EvaluationResponse>;

    async fn list_models(&self) -> Result<ListModelsResponse>;

    fn provider_name(&self) -> &'static str {
        TypeSafeProvider::NAME
    }
}

#[async_trait]
impl SystemOneProvider for crate::client::TypeSafeClient {
    async fn evaluate(&self, request: SystemOneRequest) -> Result<EvaluationResponse> {
        self.evaluate_with(request, crate::client::EvaluateOptions::new())
            .await
    }

    async fn list_models(&self) -> Result<ListModelsResponse> {
        crate::client::TypeSafeClient::list_models(self).await
    }
}
