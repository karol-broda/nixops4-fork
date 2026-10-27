use std::io::{BufRead, Read, Write};

use serde::{de::DeserializeOwned, Serialize};

use crate::schema::v1 as gen;
use crate::v1::envelope::{parse_message, MessageKind, MessageParseError};
use crate::v1::id::RequestId;
use crate::v1::method::Method;

pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum FramingError {
    Encode(serde_json::Error),
    DecodeUtf8(std::str::Utf8Error),
    LineTooLong { limit: usize },
    Io(std::io::Error),
}

impl From<serde_json::Error> for FramingError {
    fn from(e: serde_json::Error) -> Self {
        FramingError::Encode(e)
    }
}

impl From<std::io::Error> for FramingError {
    fn from(e: std::io::Error) -> Self {
        FramingError::Io(e)
    }
}

impl std::fmt::Display for FramingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FramingError::Encode(e) => write!(f, "could not encode message: {e}"),
            FramingError::DecodeUtf8(e) => write!(f, "message line is not UTF-8: {e}"),
            FramingError::LineTooLong { limit } => {
                write!(f, "message line exceeds {limit} bytes")
            }
            FramingError::Io(e) => write!(f, "message I/O failed: {e}"),
        }
    }
}

impl std::error::Error for FramingError {}

pub struct MessageWriter<W: Write> {
    inner: W,
}

impl<W: Write> MessageWriter<W> {
    pub fn new(inner: W) -> Self {
        MessageWriter { inner }
    }

    pub fn send<M: Method>(
        &mut self,
        id: RequestId,
        params: &M::Params,
    ) -> Result<(), FramingError> {
        self.send_raw(&gen::Request::new::<M>(id, params)?)
    }

    pub fn notify(&mut self, method: &str, params: &impl Serialize) -> Result<(), FramingError> {
        self.send_raw(&gen::Notification {
            jsonrpc: gen::Jsonrpc::X20,
            method: method.to_string(),
            params: Some(serde_json::to_value(params)?),
        })
    }

    pub fn send_raw<T: Serialize + ?Sized>(&mut self, message: &T) -> Result<(), FramingError> {
        serde_json::to_writer(&mut self.inner, message)?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()?;
        Ok(())
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

pub struct MessageReader<R: BufRead> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: BufRead> MessageReader<R> {
    pub fn new(inner: R) -> Self {
        MessageReader {
            inner,
            buf: Vec::new(),
        }
    }

    pub fn next(&mut self) -> Result<Option<Message<'_>>, FramingError> {
        match read_bounded_line(&mut self.inner, &mut self.buf, MAX_LINE_BYTES)? {
            LineRead::Eof => Ok(None),
            LineRead::TooLong => Err(FramingError::LineTooLong {
                limit: MAX_LINE_BYTES,
            }),
            LineRead::Line => {
                let line = std::str::from_utf8(&self.buf).map_err(FramingError::DecodeUtf8)?;
                Ok(Some(Message { line }))
            }
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

#[derive(Debug)]
pub struct Message<'a> {
    line: &'a str,
}

impl<'a> Message<'a> {
    pub fn as_str(&self) -> &'a str {
        self.line
    }

    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(self.line)
    }

    pub fn parse_message(&self) -> Result<MessageKind, MessageParseError> {
        parse_message(self.line)
    }
}

#[derive(Debug, PartialEq)]
enum LineRead {
    Line,
    Eof,
    TooLong,
}

fn read_bounded_line<R: BufRead>(
    inner: &mut R,
    buf: &mut Vec<u8>,
    limit: usize,
) -> Result<LineRead, FramingError> {
    buf.clear();
    let mut limited = inner.take(limit as u64 + 1);
    let read = limited.read_until(b'\n', buf)?;
    if read == 0 {
        return Ok(LineRead::Eof);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
    }
    if buf.len() > limit {
        return Ok(LineRead::TooLong);
    }
    Ok(LineRead::Line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::method::Create;

    fn create_params() -> crate::schema::v1::CreateParams {
        crate::schema::v1::CreateParams {
            type_: crate::schema::v1::ResourceType("file".to_string()),
            input_properties: crate::schema::v1::InputProperties(serde_json::Map::from_iter(vec![
                (
                    "contents".to_string(),
                    serde_json::Value::String("two\nlines".to_string()),
                ),
            ])),
            is_stateful: true,
            label: None,
        }
    }

    #[test]
    fn frames_round_trip_through_writer_and_reader() {
        let mut writer = MessageWriter::new(Vec::new());
        let id = RequestId::try_from_u64(7).unwrap();
        writer.send::<Create>(id, &create_params()).unwrap();
        let bytes = writer.into_inner();

        assert!(!bytes[..bytes.len() - 1].contains(&b'\n'));

        let mut reader = MessageReader::new(bytes.as_slice());
        let message = reader.next().unwrap().unwrap();
        let MessageKind::Request(request) = message.parse_message().unwrap() else {
            panic!("expected a request");
        };
        assert_eq!(request.method, "create");
        assert!(reader.next().unwrap().is_none());
    }

    #[test]
    fn notifications_round_trip() {
        let mut writer = MessageWriter::new(Vec::new());
        writer
            .notify(
                "log",
                &crate::schema::v1::LogNotificationParams {
                    request_id: None,
                    level: None,
                    message: "hello".to_string(),
                    fields: serde_json::Map::new(),
                },
            )
            .unwrap();
        let bytes = writer.into_inner();
        let mut reader = MessageReader::new(bytes.as_slice());
        let message = reader.next().unwrap().unwrap();
        assert!(matches!(
            message.parse_message().unwrap(),
            MessageKind::Notification(_)
        ));
    }

    #[test]
    fn oversize_lines_are_rejected() {
        let mut source = std::io::Cursor::new(vec![b'x'; 12]);
        let mut buf = Vec::new();
        assert_eq!(
            read_bounded_line(&mut source, &mut buf, 8).unwrap(),
            LineRead::TooLong
        );
        let mut source = std::io::Cursor::new(b"short\n".to_vec());
        assert_eq!(
            read_bounded_line(&mut source, &mut buf, 8).unwrap(),
            LineRead::Line
        );
        assert_eq!(buf.as_slice(), b"short");
    }

    #[test]
    fn invalid_utf8_is_reported() {
        let mut reader = MessageReader::new(std::io::Cursor::new(vec![0xff, b'\n']));
        assert!(matches!(
            reader.next().unwrap_err(),
            FramingError::DecodeUtf8(_)
        ));
    }
}
