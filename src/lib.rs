//! Async Rust client for TypeSafe's Jev System One API.
//!
//! System One evaluates one [`JsonContent`] state against many typed
//! [`Question`]s in a single request and returns probabilities instead of
//! free-form text. The three question types are [`noul`] for yes/no decisions,
//! [`choice`] for closed-set classification, and [`score`] for ordered rubrics.
//!
//! # Example
//!
//! ```rust,no_run
//! use jav_rs_sdk::{choice, noul, score, SystemOneRequestBuilder, TypeSafeClient};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = TypeSafeClient::new(std::env::var("TYPESAFE_API_KEY")?)?;
//!
//!     let request = SystemOneRequestBuilder::new()
//!         .state("I was charged twice. Please fix this today.")
//!         .question("billing", noul("Is this about billing?"))
//!         .question(
//!             "department",
//!             choice("Which team should handle this?", [
//!                 ("billing", "Payments, invoices, and refunds"),
//!                 ("technical", "Outages and integration failures"),
//!             ]),
//!         )
//!         .question(
//!             "urgency",
//!             score("How urgent is this?", ["can wait", "today"]),
//!         )
//!         .build()?;
//!
//!     let response = client.evaluate_with(request, Default::default()).await?;
//!     println!(
//!         "billing probability: {}",
//!         response.noul("billing").unwrap_or_default()
//!     );
//!     println!(
//!         "department: {}",
//!         response
//!             .choice("department")
//!             .map(|answer| answer.choice.as_str())
//!             .unwrap_or_default()
//!     );
//!     println!(
//!         "urgency: {}",
//!         response.score("urgency").map(|answer| answer.score).unwrap_or_default()
//!     );
//!     Ok(())
//! }
//! ```

#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::redundant_explicit_links
)]

/// HTTP clients, request builders, and endpoint methods.
pub mod client;
/// Typed SDK errors and API response error metadata.
pub mod error;
/// Provider metadata and a dependency-inversion seam for host applications.
pub mod provider;
/// Configurable exponential-backoff policy.
pub mod retry;
/// Request, answer, model, and usage types used by the TypeSafe API.
pub mod types;

pub use client::{
    EvaluateOptions, ListModelsOptions, SystemOneRequest, SystemOneRequestBuilder, TypeSafeClient,
    TypeSafeClientBuilder,
};
pub use error::{ApiError, Error, ErrorBody};
pub use provider::{
    JevProvider, MODEL_JEV_1_13_0, MODEL_JEV_LATEST, MODEL_JEV_PREVIEW, ModelPreset,
    SystemOneProvider, TypeSafeProvider,
};
pub use retry::RetryPolicy;

/// Convenient result alias used throughout the SDK.
pub type Result<T> = std::result::Result<T, Error>;
pub use types::{
    Answer, AnswerMap, ChoiceAnswer, ChoiceQuestion, ChoiceResponse, EvaluationResponse,
    JsonContent, ListModelsResponse, MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, ModelCard, NoulAnswer,
    NoulCriteria, NoulQuestion, NoulResponse, Question, QuestionMap, ScoreAnswer, ScoreQuestion,
    ScoreResponse, Usage, choice, noul, noul_with, score,
};

/// Default TypeSafe production API root.
pub const API_BASE_URL: &str = "https://api.typesafe.ai";

/// Environment variable read by [`TypeSafeClient::from_env`] for the API key.
///
/// Empty or whitespace-only values are ignored.
pub const API_KEY_ENV: &str = "TYPESAFE_API_KEY";

/// Environment variable that overrides the API base URL.
///
/// Empty or whitespace-only values are ignored.
pub const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";

/// Default model alias used when no request- or client-level model is set.
pub const DEFAULT_MODEL: &str = "jev-latest";

/// Environment variable read for the client's default model.
///
/// Empty or whitespace-only values are ignored.
pub const DEFAULT_MODEL_ENV: &str = "TYPESAFE_DEFAULT_MODEL";
