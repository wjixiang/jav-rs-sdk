use async_trait::async_trait;

use crate::Result;
use crate::client::SystemOneRequest;
use crate::types::{EvaluationResponse, ListModelsResponse};

/// Stable alias for the newest stable Jev release.
pub const MODEL_JEV_LATEST: &str = "jev-latest";
/// Stable alias for the newest release, including preview builds.
pub const MODEL_JEV_PREVIEW: &str = "jev-preview";
/// Versioned Jev 1.13 model identifier.
pub const MODEL_JEV_1_13_0: &str = "jev-1.13.0";

/// Marker for TypeSafe provider-level constants.
pub struct TypeSafeProvider;

/// Marker and preset source for TypeSafe's Jev models.
pub struct JevProvider;

/// Static catalogue metadata for a model preset.
///
/// This type does not carry credentials or an HTTP client.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelPreset {
    /// Versioned model identifier accepted by the API.
    pub name: &'static str,
    /// Model aliases that currently resolve to `name`.
    pub aliases: Vec<String>,
    /// Documented total request token budget.
    pub context_length: u64,
    /// Documented state-only token budget.
    pub max_state_length: u64,
    /// Published input price per million tokens.
    pub input_price_per_mtok: f64,
    /// Published output price per million tokens.
    pub output_price_per_mtok: f64,
}

impl TypeSafeProvider {
    /// Canonical provider name.
    pub const NAME: &'static str = "typesafe";
    /// Default TypeSafe API root.
    pub const DEFAULT_BASE_URL: &'static str = crate::API_BASE_URL;
    /// Header carrying bearer authentication.
    pub const AUTH_HEADER: &'static str = "Authorization";
    /// Authentication scheme used before the API key.
    pub const AUTH_SCHEME: &'static str = "Bearer";
}

impl JevProvider {
    /// Returns the bundled Jev model catalogue.
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
/// concrete HTTP client.
///
/// TypeSafe's endpoint evaluates many typed questions in one call, so unlike
/// chat-provider traits this intentionally has no generic text-generation
/// method.
#[async_trait]
pub trait SystemOneProvider: Send + Sync {
    /// Evaluates a validated request and returns typed answers.
    ///
    /// # Errors
    ///
    /// Propagates configuration, validation, transport, HTTP, and decoding
    /// errors.
    async fn evaluate(&self, request: SystemOneRequest) -> Result<EvaluationResponse>;

    /// Lists models available to the configured account.
    ///
    /// # Errors
    ///
    /// Propagates transport, HTTP, and decoding errors.
    async fn list_models(&self) -> Result<ListModelsResponse>;

    /// Canonical provider identifier for diagnostics and registry metadata.
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
