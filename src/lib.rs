//! Async Rust client for TypeSafe's Jev System One API.

pub mod client;
pub mod error;
pub mod provider;
pub mod retry;
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

pub type Result<T> = std::result::Result<T, Error>;
pub use types::{
    Answer, AnswerMap, ChoiceAnswer, ChoiceQuestion, ChoiceResponse, EvaluationResponse,
    JsonContent, ListModelsResponse, MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, ModelCard, NoulAnswer,
    NoulCriteria, NoulQuestion, NoulResponse, Question, QuestionMap, ScoreAnswer, ScoreQuestion,
    ScoreResponse, Usage, choice, noul, noul_with, score,
};

pub const API_BASE_URL: &str = "https://api.typesafe.ai";
pub const API_KEY_ENV: &str = "TYPESAFE_API_KEY";
pub const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
pub const DEFAULT_MODEL: &str = "jev-latest";
pub const DEFAULT_MODEL_ENV: &str = "TYPESAFE_DEFAULT_MODEL";
