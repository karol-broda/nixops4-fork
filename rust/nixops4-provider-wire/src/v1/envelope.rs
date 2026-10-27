use serde_json::Value;

use crate::schema::v1 as gen;
use crate::v1::id::{RequestId, TypedId};
use crate::v1::method::{Method, MethodName};
use crate::v1::outcome::OperationResult;

#[derive(Debug)]
pub enum AnyRequest {
    Hello(gen::HelloParams),
    Create(gen::CreateParams),
    Update(gen::UpdateParams),
    StateRead(gen::StateReadParams),
    StateEvent(gen::StateEventParams),
}

impl AnyRequest {
    pub fn method_name(&self) -> MethodName {
        match self {
            AnyRequest::Hello(_) => MethodName::Hello,
            AnyRequest::Create(_) => MethodName::Create,
            AnyRequest::Update(_) => MethodName::Update,
            AnyRequest::StateRead(_) => MethodName::StateRead,
            AnyRequest::StateEvent(_) => MethodName::StateEvent,
        }
    }
}

#[derive(Debug)]
pub enum ParamsError {
    UnknownMethod(String),
    MalformedParams(serde_json::Error),
}

impl std::fmt::Display for ParamsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamsError::UnknownMethod(method) => write!(f, "unknown method: {method}"),
            ParamsError::MalformedParams(e) => write!(f, "malformed params: {e}"),
        }
    }
}

impl std::error::Error for ParamsError {}

impl MethodName {
    fn decode_params(self, value: Value) -> Result<AnyRequest, serde_json::Error> {
        Ok(match self {
            MethodName::Hello => AnyRequest::Hello(serde_json::from_value(value)?),
            MethodName::Create => AnyRequest::Create(serde_json::from_value(value)?),
            MethodName::Update => AnyRequest::Update(serde_json::from_value(value)?),
            MethodName::StateRead => AnyRequest::StateRead(serde_json::from_value(value)?),
            MethodName::StateEvent => AnyRequest::StateEvent(serde_json::from_value(value)?),
        })
    }
}

impl gen::Request {
    pub fn new<M: Method>(id: RequestId, params: &M::Params) -> Result<Self, serde_json::Error> {
        Ok(gen::Request {
            jsonrpc: gen::Jsonrpc::X20,
            id: id.to_wire(),
            method: M::NAME.to_string(),
            params: Some(serde_json::to_value(params)?),
        })
    }

    pub fn method_name(&self) -> Option<MethodName> {
        MethodName::parse(&self.method)
    }

    pub fn decode_params(self) -> Result<AnyRequest, ParamsError> {
        let name = MethodName::parse(&self.method)
            .ok_or_else(|| ParamsError::UnknownMethod(self.method.clone()))?;
        let value = self.params.unwrap_or(Value::Null);
        name.decode_params(value)
            .map_err(ParamsError::MalformedParams)
    }
}

#[derive(Debug)]
pub enum Answer<D> {
    Result(OperationResult<D>),
    Fault(gen::Error),
}

#[derive(Debug)]
pub enum ResponseFault {
    IdMismatch { got: Option<i64> },
    NoResultOrError,
    BothResultAndError,
    MalformedResult(serde_json::Error),
}

impl std::fmt::Display for ResponseFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseFault::IdMismatch { got } => {
                write!(f, "response id {got:?} does not match the pending request")
            }
            ResponseFault::NoResultOrError => {
                write!(f, "response has neither result nor error")
            }
            ResponseFault::BothResultAndError => {
                write!(f, "response has both result and error")
            }
            ResponseFault::MalformedResult(e) => write!(f, "malformed result: {e}"),
        }
    }
}

impl std::error::Error for ResponseFault {}

impl gen::Response {
    pub fn answer<M: Method>(self, pending: TypedId<M>) -> Result<Answer<M::Done>, ResponseFault> {
        if self.id.0 != Some(pending.id().as_u64() as i64) {
            return Err(ResponseFault::IdMismatch { got: self.id.0 });
        }
        if let Some(error) = self.error {
            return if self.result.is_some() {
                Err(ResponseFault::BothResultAndError)
            } else {
                Ok(Answer::Fault(error))
            };
        }
        let result = self.result.ok_or(ResponseFault::NoResultOrError)?;
        let result = serde_json::from_value::<OperationResult<M::Done>>(result)
            .map_err(ResponseFault::MalformedResult)?;
        Ok(Answer::Result(result))
    }
}

#[derive(Debug)]
pub enum MessageKind {
    Request(gen::Request),
    Response(gen::Response),
    Notification(gen::Notification),
}

#[derive(Debug)]
pub enum MessageParseError {
    NotJson(serde_json::Error),
    NotJsonRpc,
    Batch,
}

impl std::fmt::Display for MessageParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageParseError::NotJson(e) => write!(f, "not JSON: {e}"),
            MessageParseError::NotJsonRpc => write!(f, "not a JSON-RPC message"),
            MessageParseError::Batch => write!(f, "batches are not part of the protocol"),
        }
    }
}

impl std::error::Error for MessageParseError {}

enum MessageTag {
    Request,
    Notification,
    Response,
}

pub fn parse_message(line: &str) -> Result<MessageKind, MessageParseError> {
    let value: Value = serde_json::from_str(line).map_err(MessageParseError::NotJson)?;
    let Value::Object(object) = value else {
        return Err(match value {
            Value::Array(_) => MessageParseError::Batch,
            _ => MessageParseError::NotJsonRpc,
        });
    };
    let tag = if object.contains_key("method") {
        if object.contains_key("id") {
            MessageTag::Request
        } else {
            MessageTag::Notification
        }
    } else if object.contains_key("result") || object.contains_key("error") {
        MessageTag::Response
    } else {
        return Err(MessageParseError::NotJsonRpc);
    };
    let value = Value::Object(object);
    Ok(match tag {
        MessageTag::Request => {
            MessageKind::Request(serde_json::from_value(value).map_err(MessageParseError::NotJson)?)
        }
        MessageTag::Notification => MessageKind::Notification(
            serde_json::from_value(value).map_err(MessageParseError::NotJson)?,
        ),
        MessageTag::Response => MessageKind::Response(
            serde_json::from_value(value).map_err(MessageParseError::NotJson)?,
        ),
    })
}
