use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type JsonContent = Value;
pub type QuestionMap = BTreeMap<String, Question>;
pub type AnswerMap = BTreeMap<String, Answer>;
pub type NoulResponse = NoulAnswer;
pub type ChoiceResponse = ChoiceAnswer;
pub type ScoreResponse = ScoreAnswer;

pub const MAX_CHOICE_OPTIONS: usize = 255;
pub const MAX_SCORE_LEVELS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Noul(NoulQuestion),
    Choice(ChoiceQuestion),
    Score(ScoreQuestion),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoulQuestion {
    pub instructions: JsonContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<NoulCriteria>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NoulCriteria {
    #[serde(rename = "true", default, skip_serializing_if = "Option::is_none")]
    pub r#true: Option<JsonContent>,
    #[serde(rename = "false", default, skip_serializing_if = "Option::is_none")]
    pub r#false: Option<JsonContent>,
}

impl NoulCriteria {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn true_description(mut self, description: impl Into<JsonContent>) -> Self {
        self.r#true = Some(description.into());
        self
    }

    pub fn false_description(mut self, description: impl Into<JsonContent>) -> Self {
        self.r#false = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceQuestion {
    pub instructions: JsonContent,
    pub criteria: BTreeMap<String, Option<JsonContent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreQuestion {
    pub instructions: JsonContent,
    pub criteria: Vec<JsonContent>,
}

impl Question {
    pub fn noul(instructions: impl Into<JsonContent>) -> Self {
        Self::Noul(NoulQuestion {
            instructions: instructions.into(),
            criteria: None,
        })
    }

    pub fn noul_with(instructions: impl Into<JsonContent>, criteria: NoulCriteria) -> Self {
        Self::Noul(NoulQuestion {
            instructions: instructions.into(),
            criteria: Some(criteria),
        })
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Noul(NoulAnswer),
    Choice(ChoiceAnswer),
    Score(ScoreAnswer),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoulAnswer {
    pub noul: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreAnswer {
    pub score: f64,
    pub legend: BTreeMap<String, JsonContent>,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: f64,
}

impl Answer {
    pub fn noul_value(&self) -> Option<f64> {
        match self {
            Answer::Noul(answer) => Some(answer.noul),
            _ => None,
        }
    }

    pub fn choice(&self) -> Option<&str> {
        match self {
            Answer::Choice(answer) => Some(&answer.choice),
            _ => None,
        }
    }

    pub fn score(&self) -> Option<f64> {
        match self {
            Answer::Score(answer) => Some(answer.score),
            _ => None,
        }
    }

    pub fn confidence(&self) -> Option<f64> {
        match self {
            Answer::Noul(_) => None,
            Answer::Choice(answer) => Some(answer.confidence),
            Answer::Score(answer) => Some(answer.confidence),
        }
    }
}

impl EvaluationResponse {
    pub fn answer(&self, id: &str) -> Option<&Answer> {
        self.answers.get(id)
    }

    pub fn noul(&self, id: &str) -> Option<f64> {
        self.answer(id).and_then(Answer::noul_value)
    }

    pub fn choice(&self, id: &str) -> Option<&ChoiceAnswer> {
        match self.answer(id)? {
            Answer::Choice(answer) => Some(answer),
            _ => None,
        }
    }

    pub fn score(&self, id: &str) -> Option<&ScoreAnswer> {
        match self.answer(id)? {
            Answer::Score(answer) => Some(answer),
            _ => None,
        }
    }
}

impl Usage {
    pub fn total_tokens(&self) -> Option<u64> {
        match (self.input_tokens, self.output_tokens) {
            (Some(input), Some(output)) => Some(input.saturating_add(output)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationResponse {
    pub model: String,
    pub answers: AnswerMap,
    pub usage: Usage,
    #[serde(default, skip)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCard {
    pub name: String,
    pub description: String,
    pub release_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListModelsResponse {
    pub models: Vec<ModelCard>,
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

pub fn noul(instructions: impl Into<JsonContent>) -> Question {
    Question::noul(instructions)
}

pub fn noul_with(instructions: impl Into<JsonContent>, criteria: NoulCriteria) -> Question {
    Question::noul_with(instructions, criteria)
}

pub fn choice<I, K, D>(instructions: impl Into<JsonContent>, criteria: I) -> Question
where
    I: IntoIterator<Item = (K, D)>,
    K: Into<String>,
    D: Into<JsonContent>,
{
    Question::choice(instructions, criteria)
}

pub fn score<I, D>(instructions: impl Into<JsonContent>, criteria: I) -> Question
where
    I: IntoIterator<Item = D>,
    D: Into<JsonContent>,
{
    Question::score(instructions, criteria)
}
