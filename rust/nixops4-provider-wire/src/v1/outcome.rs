use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::schema::v1::{Rejected, Warning};

pub const CORE_UNSPECIFIED: &str = "core.unspecified";
pub const PROTOCOL_MALFORMED_OUTCOME: &str = "protocol.malformedOutcome";

#[derive(Debug, Clone, PartialEq)]
pub struct OperationResult<D> {
    pub warnings: Vec<Warning>,
    pub kind: Outcome<D>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<D> {
    Done(D),
    Rejected(Rejected),
    Indeterminate(Indeterminate<D>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Indeterminate<D> {
    pub code: String,
    pub message: Option<String>,
    pub partial: Option<D>,
    pub raw_outcome: Option<String>,
}

impl<D> Outcome<D> {
    pub fn reject(code: impl Into<String>, message: impl Into<String>) -> Self {
        Outcome::Rejected(Rejected {
            code: code.into(),
            retryable: None,
            message: Some(message.into()),
        })
    }
}

impl Rejected {
    pub fn retryable(&self) -> bool {
        self.retryable.unwrap_or(false)
    }
}

impl<D: Serialize> Serialize for OperationResult<D> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = match &self.kind {
            Outcome::Done(payload) => {
                let mut object = payload_object(payload).map_err(S::Error::custom)?;
                object.insert("outcome".into(), Value::String("done".into()));
                object
            }
            Outcome::Rejected(rejected) => {
                let mut object = payload_object(rejected).map_err(S::Error::custom)?;
                object.insert("outcome".into(), Value::String("rejected".into()));
                object
            }
            Outcome::Indeterminate(indeterminate) => {
                let mut object = Map::new();
                object.insert("outcome".into(), Value::String("indeterminate".into()));
                object.insert("code".into(), Value::String(indeterminate.code.clone()));
                if let Some(message) = &indeterminate.message {
                    object.insert("message".into(), Value::String(message.clone()));
                }
                if let Some(partial) = &indeterminate.partial {
                    object.extend(payload_object(partial).map_err(S::Error::custom)?);
                }
                object
            }
        };
        if !self.warnings.is_empty() {
            let warnings = serde_json::to_value(&self.warnings).map_err(S::Error::custom)?;
            object.insert("warnings".into(), warnings);
        }
        Value::Object(object).serialize(serializer)
    }
}

impl<'de, D: DeserializeOwned> Deserialize<'de> for OperationResult<D> {
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        let Value::Object(mut object) = Value::deserialize(deserializer)? else {
            return Err(De::Error::custom("operation result must be a JSON object"));
        };
        let warnings = take_warnings(&mut object);
        let raw_outcome = object.remove("outcome");
        let kind = match raw_outcome.as_ref().and_then(Value::as_str) {
            Some("done") => {
                let payload = serde_json::from_value::<D>(Value::Object(object))
                    .map_err(De::Error::custom)?;
                Outcome::Done(payload)
            }
            Some("rejected") => {
                let code =
                    take_string(&mut object, "code").unwrap_or_else(|| CORE_UNSPECIFIED.into());
                object.insert("code".into(), Value::String(code));
                let rejected = serde_json::from_value::<Rejected>(Value::Object(object))
                    .map_err(De::Error::custom)?;
                Outcome::Rejected(rejected)
            }
            Some("indeterminate") => {
                Outcome::Indeterminate(indeterminate_from(object, None, CORE_UNSPECIFIED))
            }
            Some(unknown) => Outcome::Indeterminate(indeterminate_from(
                object,
                Some(unknown.to_string()),
                PROTOCOL_MALFORMED_OUTCOME,
            )),
            None => {
                Outcome::Indeterminate(indeterminate_from(object, None, PROTOCOL_MALFORMED_OUTCOME))
            }
        };
        Ok(OperationResult { warnings, kind })
    }
}

fn indeterminate_from<D: DeserializeOwned>(
    mut object: Map<String, Value>,
    raw_outcome: Option<String>,
    default_code: &str,
) -> Indeterminate<D> {
    let code = take_string(&mut object, "code").unwrap_or_else(|| default_code.to_string());
    let message = take_string(&mut object, "message");
    let partial = decode_partial(&object);
    Indeterminate {
        code,
        message,
        partial,
        raw_outcome,
    }
}

fn decode_partial<D: DeserializeOwned>(object: &Map<String, Value>) -> Option<D> {
    if object.is_empty() {
        return None;
    }
    serde_json::from_value::<D>(Value::Object(object.clone())).ok()
}

fn payload_object<T: Serialize>(value: &T) -> Result<Map<String, Value>, serde_json::Error> {
    match serde_json::to_value(value)? {
        Value::Object(object) => Ok(object),
        _ => Err(<serde_json::Error as serde::de::Error>::custom(
            "operation result payload must be a JSON object",
        )),
    }
}

fn take_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    match object.remove(key) {
        Some(Value::String(value)) => Some(value),
        Some(other) => {
            object.insert(key.to_string(), other);
            None
        }
        None => None,
    }
}

fn take_warnings(object: &mut Map<String, Value>) -> Vec<Warning> {
    object
        .remove("warnings")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::v1::CreateDone;

    fn done_with_outputs() -> CreateDone {
        CreateDone {
            output_properties: crate::schema::v1::OutputProperties(Map::new()),
        }
    }

    #[test]
    fn done_round_trips() {
        let result = OperationResult {
            warnings: Vec::new(),
            kind: Outcome::Done(done_with_outputs()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["outcome"], "done");
        let back: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        assert_eq!(back, result);
    }

    #[test]
    fn rejected_defaults_missing_code_and_retryable() {
        let json = serde_json::json!({"outcome": "rejected"});
        let result: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        let Outcome::Rejected(rejected) = result.kind else {
            panic!("expected rejected");
        };
        assert_eq!(rejected.code, CORE_UNSPECIFIED);
        assert!(!rejected.retryable());
    }

    #[test]
    fn indeterminate_keeps_partial_payload() {
        let json = serde_json::json!({
            "outcome": "indeterminate",
            "code": "aws.waitTimeout",
            "message": "Instance created, did not reach running",
            "outputProperties": {"id": "i-0abc"},
        });
        let result: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        let Outcome::Indeterminate(indeterminate) = result.kind else {
            panic!("expected indeterminate");
        };
        assert_eq!(indeterminate.code, "aws.waitTimeout");
        assert!(indeterminate.partial.is_some());
        assert_eq!(indeterminate.raw_outcome, None);
    }

    #[test]
    fn unknown_outcome_folds_to_indeterminate() {
        let json = serde_json::json!({"outcome": "partial", "code": "core.partial"});
        let result: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        let Outcome::Indeterminate(indeterminate) = result.kind else {
            panic!("expected indeterminate");
        };
        assert_eq!(indeterminate.raw_outcome.as_deref(), Some("partial"));
        assert_eq!(indeterminate.code, "core.partial");
    }

    #[test]
    fn missing_outcome_folds_to_indeterminate() {
        let json = serde_json::json!({"outputProperties": {}});
        let result: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        let Outcome::Indeterminate(indeterminate) = result.kind else {
            panic!("expected indeterminate");
        };
        assert_eq!(indeterminate.code, PROTOCOL_MALFORMED_OUTCOME);
        assert_eq!(indeterminate.raw_outcome, None);
        assert!(indeterminate.partial.is_some());
    }

    #[test]
    fn non_string_outcome_folds_to_indeterminate() {
        let json = serde_json::json!({"outcome": 3});
        let result: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        let Outcome::Indeterminate(indeterminate) = result.kind else {
            panic!("expected indeterminate");
        };
        assert_eq!(indeterminate.code, PROTOCOL_MALFORMED_OUTCOME);
        assert_eq!(indeterminate.raw_outcome, None);
    }

    #[test]
    fn warnings_survive_round_trips() {
        let result = OperationResult {
            warnings: vec![Warning {
                code: "core.deprecated".to_string(),
                message: "update is deprecated".to_string(),
            }],
            kind: Outcome::Done(done_with_outputs()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["warnings"][0]["code"], "core.deprecated");
        let back: OperationResult<CreateDone> = serde_json::from_value(json).unwrap();
        assert_eq!(back, result);
    }
}
