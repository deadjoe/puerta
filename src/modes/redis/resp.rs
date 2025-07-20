/// Redis RESP (Redis Serialization Protocol) parsing and generation

use bytes::{Bytes, BytesMut, Buf, BufMut};
use std::str;

/// RESP data types
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    /// Simple String (+OK\r\n)
    SimpleString(String),
    /// Error (-ERR message\r\n)
    Error(String),
    /// Integer (:123\r\n)
    Integer(i64),
    /// Bulk String ($5\r\nhello\r\n)
    BulkString(Option<Bytes>), // None represents NULL
    /// Array (*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n)
    Array(Option<Vec<RespValue>>), // None represents NULL array
}

/// RESP parser for reading Redis protocol messages
pub struct RespParser;

/// RESP encoder for writing Redis protocol messages
pub struct RespEncoder;

/// Parse error types
#[derive(Debug, thiserror::Error)]
pub enum RespParseError {
    #[error("Incomplete data - need more bytes")]
    Incomplete,
    #[error("Invalid RESP format: {0}")]
    InvalidFormat(String),
    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(#[from] str::Utf8Error),
    #[error("Invalid integer: {0}")]
    InvalidInteger(#[from] std::num::ParseIntError),
}

impl RespParser {
    /// Parse a RESP value from bytes
    pub fn parse(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if buf.is_empty() {
            return Ok(None);
        }

        let start = buf.as_ref();
        let first_byte = start[0];

        match first_byte {
            b'+' => Self::parse_simple_string(buf),
            b'-' => Self::parse_error(buf),
            b':' => Self::parse_integer(buf),
            b'$' => Self::parse_bulk_string(buf),
            b'*' => Self::parse_array(buf),
            _ => Err(RespParseError::InvalidFormat(format!(
                "Unknown RESP type: {}",
                first_byte as char
            ))),
        }
    }

    /// Parse multiple commands from a buffer
    pub fn parse_commands(buf: &mut BytesMut) -> Result<Vec<RespValue>, RespParseError> {
        let mut commands = Vec::new();
        
        while !buf.is_empty() {
            match Self::parse(buf)? {
                Some(command) => commands.push(command),
                None => break, // Incomplete data
            }
        }
        
        Ok(commands)
    }

    fn parse_simple_string(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if let Some(line) = Self::read_line(buf)? {
            let content = str::from_utf8(&line[1..])?.to_string(); // Skip '+'
            Ok(Some(RespValue::SimpleString(content)))
        } else {
            Ok(None)
        }
    }

    fn parse_error(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if let Some(line) = Self::read_line(buf)? {
            let content = str::from_utf8(&line[1..])?.to_string(); // Skip '-'
            Ok(Some(RespValue::Error(content)))
        } else {
            Ok(None)
        }
    }

    fn parse_integer(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if let Some(line) = Self::read_line(buf)? {
            let content = str::from_utf8(&line[1..])?; // Skip ':'
            let value: i64 = content.parse()?;
            Ok(Some(RespValue::Integer(value)))
        } else {
            Ok(None)
        }
    }

    fn parse_bulk_string(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if let Some(size_line) = Self::read_line(buf)? {
            let size_str = str::from_utf8(&size_line[1..])?; // Skip '$'
            let size: i64 = size_str.parse()?;

            if size == -1 {
                // NULL bulk string
                return Ok(Some(RespValue::BulkString(None)));
            }

            if size < 0 {
                return Err(RespParseError::InvalidFormat(
                    "Invalid bulk string size".to_string(),
                ));
            }

            let size = size as usize;
            
            // Check if we have enough data for the string + \r\n
            if buf.len() < size + 2 {
                // Not enough data, put back the size line
                let mut restored = BytesMut::new();
                restored.put_u8(b'$');
                restored.extend_from_slice(&size_line[1..]);
                restored.put_slice(b"\r\n");
                restored.extend_from_slice(buf);
                *buf = restored;
                return Ok(None);
            }

            let content = buf.split_to(size);
            
            // Consume \r\n
            if buf.len() < 2 || buf[0] != b'\r' || buf[1] != b'\n' {
                return Err(RespParseError::InvalidFormat(
                    "Missing \\r\\n after bulk string".to_string(),
                ));
            }
            buf.advance(2);

            Ok(Some(RespValue::BulkString(Some(content.freeze()))))
        } else {
            Ok(None)
        }
    }

    fn parse_array(buf: &mut BytesMut) -> Result<Option<RespValue>, RespParseError> {
        if let Some(size_line) = Self::read_line(buf)? {
            let size_str = str::from_utf8(&size_line[1..])?; // Skip '*'
            let size: i64 = size_str.parse()?;

            if size == -1 {
                // NULL array
                return Ok(Some(RespValue::Array(None)));
            }

            if size < 0 {
                return Err(RespParseError::InvalidFormat(
                    "Invalid array size".to_string(),
                ));
            }

            let size = size as usize;
            let mut elements = Vec::with_capacity(size);

            for _ in 0..size {
                match Self::parse(buf)? {
                    Some(element) => elements.push(element),
                    None => {
                        // Not enough data for complete array
                        // TODO: Handle partial array parsing properly
                        return Ok(None);
                    }
                }
            }

            Ok(Some(RespValue::Array(Some(elements))))
        } else {
            Ok(None)
        }
    }

    /// Read a line ending with \r\n
    fn read_line(buf: &mut BytesMut) -> Result<Option<Vec<u8>>, RespParseError> {
        for i in 0..buf.len() - 1 {
            if buf[i] == b'\r' && buf[i + 1] == b'\n' {
                let line = buf.split_to(i + 2);
                let line_content = line[..line.len() - 2].to_vec(); // Remove \r\n
                return Ok(Some(line_content));
            }
        }
        Ok(None) // No complete line found
    }
}

impl RespEncoder {
    /// Encode a RESP value to bytes
    pub fn encode(value: &RespValue) -> Bytes {
        let mut buf = BytesMut::new();
        Self::encode_into(&mut buf, value);
        buf.freeze()
    }

    /// Encode a RESP value into an existing buffer
    pub fn encode_into(buf: &mut BytesMut, value: &RespValue) {
        match value {
            RespValue::SimpleString(s) => {
                buf.put_u8(b'+');
                buf.extend_from_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::Error(s) => {
                buf.put_u8(b'-');
                buf.extend_from_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::Integer(n) => {
                buf.put_u8(b':');
                buf.extend_from_slice(n.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::BulkString(Some(data)) => {
                buf.put_u8(b'$');
                buf.extend_from_slice(data.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.extend_from_slice(data);
                buf.put_slice(b"\r\n");
            }
            RespValue::BulkString(None) => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            RespValue::Array(Some(elements)) => {
                buf.put_u8(b'*');
                buf.extend_from_slice(elements.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for element in elements {
                    Self::encode_into(buf, element);
                }
            }
            RespValue::Array(None) => {
                buf.extend_from_slice(b"*-1\r\n");
            }
        }
    }

    /// Create a Redis command from command name and arguments
    pub fn create_command(command: &str, args: &[&str]) -> RespValue {
        let mut elements = vec![RespValue::BulkString(Some(Bytes::from(command.to_string())))];
        
        for arg in args {
            elements.push(RespValue::BulkString(Some(Bytes::from(arg.to_string()))));
        }

        RespValue::Array(Some(elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let mut buf = BytesMut::from("+OK\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        assert_eq!(result, RespValue::SimpleString("OK".to_string()));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_parse_error() {
        let mut buf = BytesMut::from("-ERR unknown command\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        assert_eq!(result, RespValue::Error("ERR unknown command".to_string()));
    }

    #[test]
    fn test_parse_integer() {
        let mut buf = BytesMut::from(":1000\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        assert_eq!(result, RespValue::Integer(1000));
    }

    #[test]
    fn test_parse_bulk_string() {
        let mut buf = BytesMut::from("$5\r\nhello\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        assert_eq!(result, RespValue::BulkString(Some(Bytes::from("hello"))));
    }

    #[test]
    fn test_parse_null_bulk_string() {
        let mut buf = BytesMut::from("$-1\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        assert_eq!(result, RespValue::BulkString(None));
    }

    #[test]
    fn test_parse_array() {
        let mut buf = BytesMut::from("*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        let result = RespParser::parse(&mut buf).unwrap().unwrap();
        
        if let RespValue::Array(Some(elements)) = result {
            assert_eq!(elements.len(), 2);
            assert_eq!(elements[0], RespValue::BulkString(Some(Bytes::from("hello"))));
            assert_eq!(elements[1], RespValue::BulkString(Some(Bytes::from("world"))));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_encode_simple_string() {
        let value = RespValue::SimpleString("OK".to_string());
        let encoded = RespEncoder::encode(&value);
        assert_eq!(encoded, Bytes::from("+OK\r\n"));
    }

    #[test]
    fn test_encode_command() {
        let command = RespEncoder::create_command("SET", &["key", "value"]);
        let encoded = RespEncoder::encode(&command);
        let expected = "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n";
        assert_eq!(encoded, Bytes::from(expected));
    }

    #[test]
    fn test_incomplete_data() {
        let mut buf = BytesMut::from("+OK\r"); // Missing \n
        let result = RespParser::parse(&mut buf).unwrap();
        assert!(result.is_none()); // Should return None for incomplete data
    }
}