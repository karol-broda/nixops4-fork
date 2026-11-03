pub mod envelope;
pub mod fault;
pub mod framing;
pub mod hello;
pub mod id;
pub mod method;
pub mod outcome;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use envelope::{
    parse_message, Answer, AnyRequest, MessageKind, MessageParseError, ParamsError, ResponseFault,
};
pub use fault::{error, ErrorCode, FaultClass};
pub use framing::{FramingError, Message, MessageReader, MessageWriter, MAX_LINE_BYTES};
pub use hello::{check_hello_answer, Capability, HandshakeFault, Negotiated};
pub use id::{IdExhausted, IdRangeError, IdSource, RequestId, TypedId};
pub use method::{Method, MethodName};
pub use outcome::{
    Indeterminate, OperationResult, Outcome, CORE_UNSPECIFIED, PROTOCOL_MALFORMED_OUTCOME,
};
