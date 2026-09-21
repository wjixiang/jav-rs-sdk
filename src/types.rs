use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// JSON value used for state, instructions, and human-readable criteria.
///
/// At top-level API positions TypeSafe accepts strings, objects, and arrays;
/// nested values may use the full JSON value space.
pub type JsonContent = Value;
/// Caller-chosen question IDs mapped to their questions.
pub type QuestionMap = BTreeMap<String, Question>;
/// Question IDs mapped to their typed answers.
pub type AnswerMap = BTreeMap<String, Answer>;
/// Alias matching the yes/no answer payload.
pub type NoulResponse = NoulAnswer;
/// Alias matching the choice answer payload.
pub type ChoiceResponse = ChoiceAnswer;
/// Alias matching the score answer payload.
pub type ScoreResponse = ScoreAnswer;

/// Maximum number of options accepted by a Choice question.
pub const MAX_CHOICE_OPTIONS: usize = 255;
/// Maximum number of ordered levels accepted by a Score question.
pub const MAX_SCORE_LEVELS: usize = 10;

/// Token accounting reported by the API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    /// Input tokens consumed by the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens consumed by the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

/// A typed question evaluated against the request state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// A yes/no question.
    Noul(NoulQuestion),
    /// A single-option classification question.
    Choice(ChoiceQuestion),
    /// A question rated on ordered rubric levels.
    Score(ScoreQuestion),
}

/// Yes/no question definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoulQuestion {
    /// The question and any referenced context.
    pub instructions: JsonContent,
    /// Optional descriptions of the yes and no outcomes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<NoulCriteria>,
}

/// Optional descriptions of a Noul question's true and false outcomes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NoulCriteria {
    /// Description of what a value near one means.
    #[serde(rename = "true", default, skip_serializing_if = "Option::is_none")]
    pub r#true: Option<JsonContent>,
    /// Description of what a value near zero means.
    #[serde(rename = "false", default, skip_serializing_if = "Option::is_none")]
    pub r#false: Option<JsonContent>,
}

impl NoulCriteria {
    /// Creates empty Noul criteria.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the description for the yes outcome.
    pub fn true_description(mut self, description: impl Into<JsonContent>) -> Self {
        self.r#true = Some(description.into());
        self
    }

    /// Sets the description for the no outcome.
    pub fn false_description(mut self, description: impl Into<JsonContent>) -> Self {
        self.r#false = Some(description.into());
        self
    }
}

/// Closed-set classification question definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceQuestion {
    /// The question and any referenced context.
    pub instructions: JsonContent,
    /// Option IDs mapped to optional rubric descriptions; `None` means no
    /// extra description.
    pub criteria: BTreeMap<String, Option<JsonContent>>,
}

/// Ordered rubric question definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreQuestion {
    /// The question and any referenced context.
    pub instructions: JsonContent,
    /// Ordered level descriptions, from lowest to highest.
    pub criteria: Vec<JsonContent>,
}

impl Question {
    /// Creates a yes/no question without outcome descriptions.
    pub fn noul(instructions: impl Into<JsonContent>) -> Self {
        Self::Noul(NoulQuestion {
            instructions: instructions.into(),
            criteria: None,
        })
    }

    /// Creates a yes/no question with outcome descriptions.
    pub fn noul_with(instructions: impl Into<JsonContent>, criteria: NoulCriteria) -> Self {
        Self::Noul(NoulQuestion {
            instructions: instructions.into(),
            criteria: Some(criteria),
        })
    }

    /// Creates a Choice question from `(option, description)` pairs.
    ///
    /// The description may be any top-level API content value; use an empty
    /// string when no description is needed.
    pub fn choice<I, K, D>(instructions: impl Into<JsonContent>, criteria: I) -> Self
    where
        I: IntoIterator<Item = (K, D)>,
        K: Into<String>,
        D: Into<JsonContent>,
    {
        Self::Choice(ChoiceQuestion {
            instructions: instructions.into(),
            criteria: criteria
                .into_iter()
                .map(|(k, v)| (k.into(), Some(v.into())))
                .collect(),
        })
    }

    /// Creates a Score question from ordered level descriptions.
    pub fn score<I, D>(instructions: impl Into<JsonContent>, criteria: I) -> Self
    where
        I: IntoIterator<Item = D>,
        D: Into<JsonContent>,
    {
        Self::Score(ScoreQuestion {
            instructions: instructions.into(),
            criteria: criteria.into_iter().map(Into::into).collect(),
        })
    }

    /// Checks question-specific JSON shapes and documented size limits.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message when instructions or criteria are not
    /// string/object/array content, or when Choice/Score cardinality limits
    /// are violated.
    pub fn validate(&self) -> Result<(), String> {
        validate_content(self.instructions_value(), "instructions")?;
        match self {
            Question::Noul(question) => {
                if let Some(criteria) = &question.criteria {
                    validate_optional_content(criteria.r#true.as_ref(), "noul criteria.true")?;
                    validate_optional_content(criteria.r#false.as_ref(), "noul criteria.false")?;
                }
            }
            Question::Choice(question) => {
                if question.criteria.is_empty() {
                    return Err("choice questions require at least one option".into());
                }
                if question.criteria.len() > MAX_CHOICE_OPTIONS {
                    return Err("choice questions allow at most 255 options".into());
                }
                for description in question.criteria.values().flatten() {
                    validate_content(description, "choice criteria description")?;
                }
            }
            Question::Score(question) => {
                if question.criteria.len() < 2 {
                    return Err("score questions require at least two levels".into());
                }
                if question.criteria.len() > MAX_SCORE_LEVELS {
                    return Err("score questions allow at most 10 levels".into());
                }
                for description in &question.criteria {
                    validate_content(description, "score criteria level")?;
                }
            }
        }
        Ok(())
    }

    fn instructions_value(&self) -> &JsonContent {
        match self {
            Question::Noul(question) => &question.instructions,
            Question::Choice(question) => &question.instructions,
            Question::Score(question) => &question.instructions,
        }
    }
}

/// A typed answer returned for a question.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    /// Yes/no probability answer.
    Noul(NoulAnswer),
    /// Selected option and probability distribution.
    Choice(ChoiceAnswer),
    /// Probability-weighted rubric score and distribution.
    Score(ScoreAnswer),
}

/// Yes/no answer probability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoulAnswer {
    /// Probability that the answer is yes, from zero to one.
    pub noul: f64,
}

/// Choice answer, selected option, and full distribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceAnswer {
    /// Highest-probability option.
    pub choice: String,
    /// Every option mapped to its probability.
    pub probabilities: BTreeMap<String, f64>,
    /// Certainty measure derived from the distribution, from zero to one.
    pub confidence: f64,
}

/// Score answer with weighted value, legend, and distribution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreAnswer {
    /// Probability-weighted position across rubric levels.
    pub score: f64,
    /// Level index mapped back to its description.
    pub legend: BTreeMap<String, JsonContent>,
    /// Level index, as text, mapped to its probability.
    pub probabilities: BTreeMap<String, f64>,
    /// Certainty measure derived from the distribution, from zero to one.
    pub confidence: f64,
}

impl Answer {
    /// Returns the yes probability when this is a Noul answer.
    pub fn noul_value(&self) -> Option<f64> {
        match self {
            Answer::Noul(answer) => Some(answer.noul),
            _ => None,
        }
    }

    /// Returns the selected option when this is a Choice answer.
    pub fn choice(&self) -> Option<&str> {
        match self {
            Answer::Choice(answer) => Some(&answer.choice),
            _ => None,
        }
    }

    /// Returns the weighted score when this is a Score answer.
    pub fn score(&self) -> Option<f64> {
        match self {
            Answer::Score(answer) => Some(answer.score),
            _ => None,
        }
    }

    /// Returns confidence for Choice and Score answers.
    ///
    /// Noul answers expose their probability through [`Self::noul_value`] and
    /// do not carry a separate confidence field.
    pub fn confidence(&self) -> Option<f64> {
        match self {
            Answer::Noul(_) => None,
            Answer::Choice(answer) => Some(answer.confidence),
            Answer::Score(answer) => Some(answer.confidence),
        }
    }
}

impl EvaluationResponse {
    /// Returns an answer by its original question ID.
    pub fn answer(&self, id: &str) -> Option<&Answer> {
        self.answers.get(id)
    }

    /// Convenience accessor for a Noul probability.
    pub fn noul(&self, id: &str) -> Option<f64> {
        self.answer(id).and_then(Answer::noul_value)
    }

    /// Convenience accessor for a Choice answer.
    pub fn choice(&self, id: &str) -> Option<&ChoiceAnswer> {
        match self.answer(id)? {
            Answer::Choice(answer) => Some(answer),
            _ => None,
        }
    }

    /// Convenience accessor for a Score answer.
    pub fn score(&self, id: &str) -> Option<&ScoreAnswer> {
        match self.answer(id)? {
            Answer::Score(answer) => Some(answer),
            _ => None,
        }
    }
}

impl Usage {
    /// Returns input plus output tokens, saturating on overflow.
    ///
    /// Returns `None` only when both counts are absent.
    pub fn total_tokens(&self) -> Option<u64> {
        match (self.input_tokens, self.output_tokens) {
            (Some(input), Some(output)) => Some(input.saturating_add(output)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        }
    }
}

/// Evaluation response containing answers and API metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationResponse {
    /// Versioned model that handled the request.
    pub model: String,
    /// One answer for each requested question ID.
    pub answers: AnswerMap,
    /// Token usage reported by the API.
    pub usage: Usage,
    /// `x-typesafe-request-id` response header, populated by the HTTP client.
    #[serde(default, skip)]
    pub request_id: Option<String>,
}

/// Model entry returned by the catalogue endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCard {
    /// Model identifier or alias accepted in evaluation requests.
    pub name: String,
    /// Model description.
    pub description: String,
    /// Release date.
    pub release_date: String,
}

/// Response from the model catalogue endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListModelsResponse {
    /// Models and aliases available to the account.
    pub models: Vec<ModelCard>,
    /// `x-typesafe-request-id` response header, populated by the HTTP client.
    #[serde(default, skip)]
    pub request_id: Option<String>,
}

pub(crate) fn validate_state(state: &JsonContent) -> Result<(), String> {
    validate_content(state, "state")
}

pub(crate) fn validate_content(value: &JsonContent, field: &'static str) -> Result<(), String> {
    match value {
        Value::String(_) | Value::Object(_) | Value::Array(_) => Ok(()),
        _ => Err(format!(
            "{field} must be text, a JSON object, or a JSON array"
        )),
    }
}

fn validate_optional_content(
    value: Option<&JsonContent>,
    field: &'static str,
) -> Result<(), String> {
    match value {
        Some(value) => validate_content(value, field),
        None => Ok(()),
    }
}

/// Creates a yes/no question.
pub fn noul(instructions: impl Into<JsonContent>) -> Question {
    Question::noul(instructions)
}

/// Creates a yes/no question with outcome descriptions.
pub fn noul_with(instructions: impl Into<JsonContent>, criteria: NoulCriteria) -> Question {
    Question::noul_with(instructions, criteria)
}

/// Creates a Choice question from `(option, description)` pairs.
pub fn choice<I, K, D>(instructions: impl Into<JsonContent>, criteria: I) -> Question
where
    I: IntoIterator<Item = (K, D)>,
    K: Into<String>,
    D: Into<JsonContent>,
{
    Question::choice(instructions, criteria)
}

/// Creates a Score question from ordered level descriptions.
pub fn score<I, D>(instructions: impl Into<JsonContent>, criteria: I) -> Question
where
    I: IntoIterator<Item = D>,
    D: Into<JsonContent>,
{
    Question::score(instructions, criteria)
}
