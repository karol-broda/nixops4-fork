use crate::schema::v1::Error as ErrorObject;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const UNSUPPORTED_PROTOCOL: i64 = -32001;
pub const NOT_INITIALIZED: i64 = -32002;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    UnsupportedProtocol,
    NotInitialized,
    Other(i64),
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        match code {
            PARSE_ERROR => ErrorCode::ParseError,
            INVALID_REQUEST => ErrorCode::InvalidRequest,
            METHOD_NOT_FOUND => ErrorCode::MethodNotFound,
            INVALID_PARAMS => ErrorCode::InvalidParams,
            UNSUPPORTED_PROTOCOL => ErrorCode::UnsupportedProtocol,
            NOT_INITIALIZED => ErrorCode::NotInitialized,
            other => ErrorCode::Other(other),
        }
    }
}

impl From<ErrorCode> for i64 {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::ParseError => PARSE_ERROR,
            ErrorCode::InvalidRequest => INVALID_REQUEST,
            ErrorCode::MethodNotFound => METHOD_NOT_FOUND,
            ErrorCode::InvalidParams => INVALID_PARAMS,
            ErrorCode::UnsupportedProtocol => UNSUPPORTED_PROTOCOL,
            ErrorCode::NotInitialized => NOT_INITIALIZED,
            ErrorCode::Other(other) => other,
        }
    }
}

impl ErrorCode {
    pub fn name(self) -> &'static str {
        match self {
            ErrorCode::ParseError => "parseError",
            ErrorCode::InvalidRequest => "invalidRequest",
            ErrorCode::MethodNotFound => "methodNotFound",
            ErrorCode::InvalidParams => "invalidParams",
            ErrorCode::UnsupportedProtocol => "unsupportedProtocol",
            ErrorCode::NotInitialized => "notInitialized",
            ErrorCode::Other(_) => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultClass {
    TreatedAsRejected,
    TreatedAsIndeterminate,
}

pub fn error(code: ErrorCode, message: impl Into<String>) -> ErrorObject {
    ErrorObject {
        code: code.into(),
        message: message.into(),
        data: None,
    }
}

impl ErrorObject {
    pub fn code(&self) -> ErrorCode {
        self.code.into()
    }

    pub fn class(&self) -> FaultClass {
        match self.code() {
            ErrorCode::ParseError
            | ErrorCode::InvalidRequest
            | ErrorCode::MethodNotFound
            | ErrorCode::InvalidParams => FaultClass::TreatedAsRejected,
            _ => FaultClass::TreatedAsIndeterminate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_codes_map_to_rejected_everything_else_to_indeterminate() {
        for code in [
            PARSE_ERROR,
            INVALID_REQUEST,
            METHOD_NOT_FOUND,
            INVALID_PARAMS,
        ] {
            assert_eq!(
                error(code.into(), "fault").class(),
                FaultClass::TreatedAsRejected
            );
        }
        for code in [UNSUPPORTED_PROTOCOL, NOT_INITIALIZED, -32099, 42] {
            assert_eq!(
                error(code.into(), "fault").class(),
                FaultClass::TreatedAsIndeterminate
            );
        }
    }

    #[test]
    fn codes_round_trip_through_i64() {
        for code in [
            ErrorCode::ParseError,
            ErrorCode::InvalidRequest,
            ErrorCode::MethodNotFound,
            ErrorCode::InvalidParams,
            ErrorCode::UnsupportedProtocol,
            ErrorCode::NotInitialized,
            ErrorCode::Other(-32100),
        ] {
            assert_eq!(ErrorCode::from(i64::from(code)), code);
        }
    }
}
