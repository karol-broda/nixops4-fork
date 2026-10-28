use serde::de::DeserializeOwned;
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

#[derive(Debug, thiserror::Error)]
pub enum ParamsError {
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("malformed params: {0}")]
    MalformedParams(#[source] serde_json::Error),
}

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

#[derive(Debug, thiserror::Error)]
pub enum ResponseFault {
    #[error("response id {got:?} does not match the pending request")]
    IdMismatch { got: Option<i64> },
    #[error("response has neither result nor error")]
    NoResultOrError,
    #[error("response has both result and error")]
    BothResultAndError,
    #[error("malformed result: {0}")]
    MalformedResult(#[source] serde_json::Error),
}

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

#[derive(Debug, thiserror::Error)]
pub enum MessageParseError {
    #[error("not JSON: {0}")]
    NotJson(#[source] serde_json::Error),
    #[error("not a JSON-RPC message")]
    NotJsonRpc,
    #[error("batches are not part of the protocol")]
    Batch,
    #[error("malformed message: {0}")]
    MalformedMessage(#[source] serde_json::Error),
}

enum MessageTag {
    Request,
    Notification,
    Response,
}

fn decode_envelope<T: DeserializeOwned>(value: Value) -> Result<T, MessageParseError> {
    serde_json::from_value(value).map_err(MessageParseError::MalformedMessage)
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
        MessageTag::Request => MessageKind::Request(decode_envelope(value)?),
        MessageTag::Notification => MessageKind::Notification(decode_envelope(value)?),
        MessageTag::Response => MessageKind::Response(decode_envelope(value)?),
    })
}
