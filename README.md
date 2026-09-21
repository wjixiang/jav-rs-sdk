# jav-rs-sdk

An async Rust SDK for TypeSafe's Jev System One API.

## Usage

```rust
use jav_rs_sdk::{noul, choice, score, QuestionMap, TypeSafeClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TypeSafeClient::builder()
        .api_key(std::env::var("TYPESAFE_API_KEY")?)
        .from_env()?;

    let mut questions = QuestionMap::new();
    questions.insert("billing".into(), noul("Is this about billing?"));
    questions.insert(
        "department".into(),
        choice("Which team should handle this?", [
            ("billing", "Payments, invoicing, refunds"),
            ("technical", "Bugs, outages, integrations"),
        ]),
    );
    questions.insert(
        "urgency".into(),
        score("How urgent is this?", ["can wait", "today"]),
    );

    let response = client
        .evaluate("I was charged twice. Please fix this today.", questions)
        .await?;
    println!("{}", response.answers["billing"].noul_value().unwrap());
    println!("{}", response.answers["department"].choice().unwrap());
    println!("{}", response.answers["urgency"].score().unwrap());
    Ok(())
}
```

The client reads `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, and
`TYPESAFE_DEFAULT_MODEL` through `TypeSafeClient::from_env()`. Calls retry
`408`, `429`, `529`, and 5xx responses with exponential backoff by default and
honor `Retry-After` / `retry-after-ms`.

## Integration surface

The SDK follows the same boundaries as the existing Agentik provider SDKs:
provider/model metadata is separate from connection configuration, credentials
are redacted from debug output, and applications can depend on a small provider
trait rather than the concrete HTTP client.

```rust
use std::sync::Arc;
use jav_rs_sdk::{JevProvider, SystemOneProvider, TypeSafeClient};

let client = TypeSafeClient::new(std::env::var("TYPESAFE_API_KEY")?)?;
let provider: Arc<dyn SystemOneProvider> = Arc::new(client);
assert_eq!(provider.provider_name(), "typesafe");

for model in JevProvider::preset_models() {
    println!("{}: {:?}", model.name, model.aliases);
}
```

Requests can be assembled with `SystemOneRequestBuilder`; `evaluate_with` accepts
per-call model, retry, timeout, header, and extra-body overrides. `Choice`
questions accept up to 255 options and `Score` questions accept 2-10 levels.
