use std::time::Duration;

use jav_rs_sdk::{
    EvaluateOptions, JevProvider, NoulCriteria, QuestionMap, RetryPolicy, SystemOneRequestBuilder,
    TypeSafeClient, choice, noul, noul_with, score,
};
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fast_retry() -> RetryPolicy {
    RetryPolicy {
        initial_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
        jitter: 0.0,
        ..RetryPolicy::default()
    }
}

#[tokio::test]
async fn evaluates_typed_questions_and_exposes_response_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-typesafe-request-id", "req-1")
                .set_body_json(json!({
                    "model": "jev-1.13.0",
                    "answers": {
                        "urgent": {"type": "noul", "noul": 0.95},
                        "department": {
                            "type": "choice",
                            "choice": "technical",
                            "probabilities": {"technical": 0.8, "billing": 0.2},
                            "confidence": 0.81
                        },
                        "frustration": {
                            "type": "score",
                            "score": 1.05,
                            "legend": {"0": "Calm", "1": "Frustrated", "2": "Very angry"},
                            "probabilities": {"0": 0.0, "1": 0.95, "2": 0.05},
                            "confidence": 0.92
                        }
                    },
                    "usage": {"input_tokens": 296, "output_tokens": 20}
                })),
        )
        .mount(&server)
        .await;

    let client = TypeSafeClient::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(RetryPolicy::disabled())
        .build()
        .unwrap();
    let response = client
        .evaluate_with(
            SystemOneRequestBuilder::new()
                .state(json!({"message": "Help! Payouts failed for three days."}))
                .question(
                    "urgent",
                    noul_with(
                        "Does this convey urgency?",
                        NoulCriteria::new().true_description("Explicitly time-sensitive"),
                    ),
                )
                .question(
                    "department",
                    choice(
                        "Which team?",
                        [("technical", "Outages"), ("billing", "Payments")],
                    ),
                )
                .question(
                    "frustration",
                    score("How angry?", ["Calm", "Frustrated", "Very angry"]),
                )
                .build()
                .unwrap(),
            EvaluateOptions::new().model("jev-preview"),
        )
        .await
        .unwrap();

    assert_eq!(response.request_id.as_deref(), Some("req-1"));
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.noul("urgent"), Some(0.95));
    assert_eq!(response.choice("department").unwrap().choice, "technical");
    assert_eq!(response.score("frustration").unwrap().confidence, 0.92);
    assert_eq!(response.usage.total_tokens(), Some(316));

    let request = &server.received_requests().await.unwrap()[0];
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "jev-preview");
    assert_eq!(body["questions"]["urgent"]["type"], "noul");
    assert_eq!(
        body["questions"]["urgent"]["criteria"]["true"],
        "Explicitly time-sensitive"
    );
    assert_eq!(
        body["questions"]["department"]["criteria"]["billing"],
        "Payments"
    );
}

#[tokio::test]
async fn retries_rate_limit_with_retry_after_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after-ms", "1")
                .set_body_json(json!({"message": "rate limited"})),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {"yes": {"type": "noul", "noul": 0.9}},
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .mount(&server)
        .await;

    let client = TypeSafeClient::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(fast_retry())
        .build()
        .unwrap();
    let response = client
        .evaluate(
            "state",
            QuestionMap::from([("yes".to_owned(), noul("Yes?"))]),
        )
        .await
        .unwrap();

    assert_eq!(response.noul("yes"), Some(0.9));
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn lists_models_from_catalogue_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{
                "name": "jev-latest",
                "description": "Flagship model",
                "release_date": "2026-01-01"
            }]
        })))
        .mount(&server)
        .await;

    let client = TypeSafeClient::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let models = client.list_models().await.unwrap();

    assert_eq!(models.models[0].name, "jev-latest");
}

#[test]
fn validates_documented_limits_and_content_shapes() {
    let request = SystemOneRequestBuilder::new()
        .state(json!(42))
        .question("one", score("Too few", ["only"]))
        .build();
    assert!(request.is_err());

    let request = SystemOneRequestBuilder::new()
        .state("valid")
        .question(
            "bad",
            noul_with(
                "Bad criteria",
                NoulCriteria::new().true_description(json!(7)),
            ),
        )
        .build();
    assert!(request.is_err());
}

#[test]
fn exposes_provider_and_model_presets_for_integration() {
    assert_eq!(jav_rs_sdk::TypeSafeProvider::NAME, "typesafe");
    assert_eq!(JevProvider::preset_models()[0].name, "jev-1.13.0");
    assert_eq!(
        JevProvider::preset_models()[0].aliases,
        ["jev-latest".to_owned(), "jev-preview".to_owned()]
    );
}
