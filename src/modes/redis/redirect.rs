/// Redis cluster redirection handling (MOVED and ASK)
///
/// Based on RCProxy's implementation for handling Redis cluster redirections.
/// Implements efficient parsing of MOVED/ASK responses using Aho-Corasick for
/// pattern matching similar to rcproxy/src/protocol/redis/resp.rs
use super::resp::RespValue;
use aho_corasick::AhoCorasick;
use bytes::Bytes;
use lazy_static::lazy_static;
use std::str;

/// Types of Redis cluster redirections
#[derive(Debug, Clone, PartialEq)]
pub enum RedirectType {
    /// MOVED redirection - slot has permanently moved
    Moved { slot: u16, address: String },
    /// ASK redirection - temporary redirection during slot migration
    Ask { slot: u16, address: String },
}

/// Constants for efficient redirection parsing based on RCProxy
const BYTE_SPACE: u8 = b' ';
const PATTERNS: &[&str] = &["ASK", "MOVED"];

lazy_static! {
    static ref FINDER: AhoCorasick =
        AhoCorasick::new(PATTERNS).expect("Failed to create AhoCorasick pattern finder");
}

/// Redis cluster redirection parser with RCProxy-style efficient parsing
pub struct RedirectParser;

/// Parse error for redirection responses
#[derive(Debug, thiserror::Error)]
pub enum RedirectError {
    #[error("Not a redirection response")]
    NotRedirection,
    #[error("Invalid redirection format: {0}")]
    InvalidFormat(String),
    #[error("Invalid slot number: {0}")]
    InvalidSlot(#[from] std::num::ParseIntError),
}

impl RedirectParser {
    /// Parse raw bytes for MOVED/ASK redirections using RCProxy's efficient approach
    /// Based on rcproxy/src/protocol/redis/resp.rs:554-576
    pub fn parse_redirect_bytes(data: &[u8]) -> Option<RedirectType> {
        if let Some(mat) = FINDER.find(data) {
            let pat = mat.pattern();
            let end = mat.end();

            // Check if there's a space after the pattern
            if end >= data.len() || data[end] != BYTE_SPACE {
                return None;
            }

            let rdata = &data[end + 1..];

            let pos = rdata.iter().position(|&x| x == BYTE_SPACE)?;

            let sdata = &rdata[..pos];
            let tdata = &rdata[pos + 1..];

            if let Ok(slot) = btoi::btoi::<u16>(sdata) {
                let to = String::from_utf8_lossy(tdata);
                let to = to.trim_end_matches('\n').trim_end_matches('\r').to_string();

                if pat.as_u32() == 0 {
                    return Some(RedirectType::Ask { slot, address: to });
                } else {
                    // moved
                    return Some(RedirectType::Moved { slot, address: to });
                }
            }
        }
        None
    }

    /// Parse a RESP error response to extract redirection information
    pub fn parse_redirect(resp_value: &RespValue) -> Result<RedirectType, RedirectError> {
        match resp_value {
            RespValue::Error(error_msg) => Self::parse_error_message(error_msg),
            _ => Err(RedirectError::NotRedirection),
        }
    }

    /// Parse raw error response bytes (like "-MOVED 3999 127.0.0.1:6381\r\n")
    pub fn parse_redirect_raw(response: &[u8]) -> Option<RedirectType> {
        // Redis error responses start with '-'
        if response.first() == Some(&b'-') {
            // Skip the '-' prefix and parse the actual error message
            return Self::parse_redirect_bytes(&response[1..]);
        }
        None
    }

    /// Parse error message for MOVED or ASK redirections
    fn parse_error_message(error_msg: &str) -> Result<RedirectType, RedirectError> {
        if error_msg.starts_with("MOVED ") {
            Self::parse_moved(error_msg)
        } else if error_msg.starts_with("ASK ") {
            Self::parse_ask(error_msg)
        } else {
            Err(RedirectError::NotRedirection)
        }
    }

    /// Parse MOVED redirection: "MOVED 3999 127.0.0.1:6381"
    fn parse_moved(error_msg: &str) -> Result<RedirectType, RedirectError> {
        let parts: Vec<&str> = error_msg.splitn(3, ' ').collect();
        if parts.len() != 3 {
            return Err(RedirectError::InvalidFormat(
                "MOVED requires format: MOVED <slot> <address>".to_string(),
            ));
        }

        let slot: u16 = parts[1].parse()?;
        let address = parts[2].to_string();

        Ok(RedirectType::Moved { slot, address })
    }

    /// Parse ASK redirection: "ASK 3999 127.0.0.1:6381"
    fn parse_ask(error_msg: &str) -> Result<RedirectType, RedirectError> {
        let parts: Vec<&str> = error_msg.splitn(3, ' ').collect();
        if parts.len() != 3 {
            return Err(RedirectError::InvalidFormat(
                "ASK requires format: ASK <slot> <address>".to_string(),
            ));
        }

        let slot: u16 = parts[1].parse()?;
        let address = parts[2].to_string();

        Ok(RedirectType::Ask { slot, address })
    }

    /// Check if a RESP value is a redirection error
    pub fn is_redirect(resp_value: &RespValue) -> bool {
        match resp_value {
            RespValue::Error(error_msg) => {
                error_msg.starts_with("MOVED ") || error_msg.starts_with("ASK ")
            }
            _ => false,
        }
    }

    /// Extract slot number from redirection if possible
    pub fn extract_slot(resp_value: &RespValue) -> Option<u16> {
        if let Ok(redirect) = Self::parse_redirect(resp_value) {
            match redirect {
                RedirectType::Moved { slot, .. } | RedirectType::Ask { slot, .. } => Some(slot),
            }
        } else {
            None
        }
    }

    /// Extract target address from redirection if possible
    pub fn extract_address(resp_value: &RespValue) -> Option<String> {
        if let Ok(redirect) = Self::parse_redirect(resp_value) {
            match redirect {
                RedirectType::Moved { address, .. } | RedirectType::Ask { address, .. } => {
                    Some(address)
                }
            }
        } else {
            None
        }
    }
}

/// Redirection context for tracking redirect chains (based on RCProxy)
#[derive(Debug, Clone)]
pub struct RedirectionContext {
    pub original_slot: u16,
    pub redirect_count: u8,
    pub max_redirects: u8,
    pub redirect_chain: Vec<String>,
}

impl RedirectionContext {
    pub fn new(slot: u16, max_redirects: u8) -> Self {
        Self {
            original_slot: slot,
            redirect_count: 0,
            max_redirects,
            redirect_chain: Vec::new(),
        }
    }

    pub fn add_redirect(&mut self, target: &str) -> Result<(), RedirectError> {
        if self.redirect_count >= self.max_redirects {
            return Err(RedirectError::InvalidFormat(
                "Max redirects exceeded".to_string(),
            ));
        }

        self.redirect_count += 1;
        self.redirect_chain.push(target.to_string());
        Ok(())
    }

    pub fn can_redirect(&self) -> bool {
        self.redirect_count < self.max_redirects
    }
}

/// Redirection data structure based on RCProxy's Redirection
#[derive(Debug, Clone)]
pub struct Redirection {
    pub target: RedirectType,
    pub requires_asking: bool,
}

impl Redirection {
    pub fn new_move(slot: u16, to: String) -> Self {
        Self {
            target: RedirectType::Moved { slot, address: to },
            requires_asking: false,
        }
    }

    pub fn new_ask(slot: u16, to: String) -> Self {
        Self {
            target: RedirectType::Ask { slot, address: to },
            requires_asking: true,
        }
    }
}

/// Redirection handler for managing Redis cluster redirections
pub struct RedirectHandler {
    max_redirects: u8,
}

impl RedirectHandler {
    pub fn new(max_redirects: u8) -> Self {
        Self { max_redirects }
    }

    /// Handle a redirection response and return the next action
    pub fn handle_redirect(
        &self,
        redirect: RedirectType,
        current_redirects: u8,
    ) -> Result<RedirectAction, RedirectError> {
        if current_redirects >= self.max_redirects {
            return Ok(RedirectAction::TooManyRedirects);
        }

        match redirect {
            RedirectType::Moved { slot, address } => {
                let redirect_clone = RedirectType::Moved {
                    slot,
                    address: address.clone(),
                };
                Ok(RedirectAction::Retry {
                    address,
                    slot,
                    redirect_type: redirect_clone,
                    requires_asking: false,
                })
            }
            RedirectType::Ask { slot, address } => {
                let redirect_clone = RedirectType::Ask {
                    slot,
                    address: address.clone(),
                };
                Ok(RedirectAction::Retry {
                    address,
                    slot,
                    redirect_type: redirect_clone,
                    requires_asking: true, // ASK requires ASKING command
                })
            }
        }
    }

    /// Create ASKING command for ASK redirections
    pub fn create_asking_command() -> RespValue {
        use super::resp::RespEncoder;
        RespEncoder::create_command("ASKING", &[])
    }

    /// Create ASKING command as raw bytes (based on RCProxy)
    pub fn create_asking_command_bytes() -> Bytes {
        Bytes::from("*1\r\n$6\r\nASKING\r\n")
    }

    /// Validate Redis cluster node address format
    pub fn validate_node_address(address: &str) -> Result<(), RedirectError> {
        // Basic validation for "host:port" format
        if let Some(colon_pos) = address.rfind(':') {
            let host = &address[..colon_pos];
            let port_str = &address[colon_pos + 1..];

            if host.is_empty() {
                return Err(RedirectError::InvalidFormat("Empty host".to_string()));
            }

            if let Err(_) = port_str.parse::<u16>() {
                return Err(RedirectError::InvalidFormat(
                    "Invalid port number".to_string(),
                ));
            }

            Ok(())
        } else {
            Err(RedirectError::InvalidFormat(
                "Missing port separator".to_string(),
            ))
        }
    }

    /// Handle redirection based on RCProxy's approach
    pub async fn handle_redirection(
        &self,
        redirect: RedirectType,
        context: &mut RedirectionContext,
    ) -> Result<Redirection, RedirectError> {
        match redirect {
            RedirectType::Moved { slot, address } => {
                Self::validate_node_address(&address)?;
                context.add_redirect(&address)?;

                log::info!(
                    "Handling MOVED redirection for slot {} to {} (redirect #{}/{})",
                    slot,
                    address,
                    context.redirect_count,
                    context.max_redirects
                );

                Ok(Redirection::new_move(slot, address))
            }
            RedirectType::Ask { slot, address } => {
                Self::validate_node_address(&address)?;
                context.add_redirect(&address)?;

                log::info!(
                    "Handling ASK redirection for slot {} to {} (redirect #{}/{})",
                    slot,
                    address,
                    context.redirect_count,
                    context.max_redirects
                );

                Ok(Redirection::new_ask(slot, address))
            }
        }
    }
}

/// Action to take based on redirection
#[derive(Debug, Clone)]
pub enum RedirectAction {
    /// Retry the command with the specified address
    Retry {
        address: String,
        slot: u16,
        redirect_type: RedirectType,
        requires_asking: bool, // True for ASK redirections
    },
    /// Too many redirects - give up
    TooManyRedirects,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_moved_redirect() {
        let error = RespValue::Error("MOVED 3999 127.0.0.1:6381".to_string());
        let redirect = RedirectParser::parse_redirect(&error).unwrap();

        match redirect {
            RedirectType::Moved { slot, address } => {
                assert_eq!(slot, 3999);
                assert_eq!(address, "127.0.0.1:6381");
            }
            _ => panic!("Expected MOVED redirect"),
        }
    }

    #[test]
    fn test_parse_ask_redirect() {
        let error = RespValue::Error("ASK 3999 127.0.0.1:6381".to_string());
        let redirect = RedirectParser::parse_redirect(&error).unwrap();

        match redirect {
            RedirectType::Ask { slot, address } => {
                assert_eq!(slot, 3999);
                assert_eq!(address, "127.0.0.1:6381");
            }
            _ => panic!("Expected ASK redirect"),
        }
    }

    #[test]
    fn test_is_redirect() {
        let moved = RespValue::Error("MOVED 3999 127.0.0.1:6381".to_string());
        let ask = RespValue::Error("ASK 3999 127.0.0.1:6381".to_string());
        let normal_error = RespValue::Error("ERR unknown command".to_string());
        let ok = RespValue::SimpleString("OK".to_string());

        assert!(RedirectParser::is_redirect(&moved));
        assert!(RedirectParser::is_redirect(&ask));
        assert!(!RedirectParser::is_redirect(&normal_error));
        assert!(!RedirectParser::is_redirect(&ok));
    }

    #[test]
    fn test_extract_slot() {
        let moved = RespValue::Error("MOVED 12345 127.0.0.1:6381".to_string());
        let slot = RedirectParser::extract_slot(&moved);
        assert_eq!(slot, Some(12345));

        let normal_error = RespValue::Error("ERR unknown command".to_string());
        let no_slot = RedirectParser::extract_slot(&normal_error);
        assert_eq!(no_slot, None);
    }

    #[test]
    fn test_extract_address() {
        let ask = RespValue::Error("ASK 3999 192.168.1.100:6379".to_string());
        let address = RedirectParser::extract_address(&ask);
        assert_eq!(address, Some("192.168.1.100:6379".to_string()));
    }

    #[test]
    fn test_redirect_handler() {
        let handler = RedirectHandler::new(3);
        let redirect = RedirectType::Moved {
            slot: 1000,
            address: "127.0.0.1:6381".to_string(),
        };

        let action = handler.handle_redirect(redirect, 0).unwrap();
        match action {
            RedirectAction::Retry {
                address,
                slot,
                requires_asking,
                ..
            } => {
                assert_eq!(address, "127.0.0.1:6381");
                assert_eq!(slot, 1000);
                assert!(!requires_asking); // MOVED doesn't require ASKING
            }
            _ => panic!("Expected retry action"),
        }
    }

    #[test]
    fn test_too_many_redirects() {
        let handler = RedirectHandler::new(2);
        let redirect = RedirectType::Ask {
            slot: 1000,
            address: "127.0.0.1:6381".to_string(),
        };

        let action = handler.handle_redirect(redirect, 2).unwrap();
        match action {
            RedirectAction::TooManyRedirects => {
                // Expected
            }
            _ => panic!("Expected too many redirects"),
        }
    }

    #[test]
    fn test_asking_command() {
        let asking = RedirectHandler::create_asking_command();
        if let RespValue::Array(Some(elements)) = asking {
            assert_eq!(elements.len(), 1);
            if let RespValue::BulkString(Some(cmd)) = &elements[0] {
                assert_eq!(cmd, "ASKING");
            } else {
                panic!("Expected ASKING bulk string");
            }
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_invalid_redirect_format() {
        let invalid = RespValue::Error("MOVED 3999".to_string()); // Missing address
        let result = RedirectParser::parse_redirect(&invalid);
        assert!(result.is_err());

        let invalid2 = RespValue::Error("MOVED abc 127.0.0.1:6381".to_string()); // Invalid slot
        let result2 = RedirectParser::parse_redirect(&invalid2);
        assert!(result2.is_err());
    }

    #[test]
    fn test_parse_redirect_bytes_rcproxy_style() {
        // Test RCProxy-style parsing with raw bytes
        let moved_data = b"MOVED 3999 127.0.0.1:6381\r\n";
        let redirect = RedirectParser::parse_redirect_bytes(moved_data).unwrap();

        match redirect {
            RedirectType::Moved { slot, address } => {
                assert_eq!(slot, 3999);
                assert_eq!(address, "127.0.0.1:6381");
            }
            _ => panic!("Expected MOVED redirect"),
        }

        let ask_data = b"ASK 12345 192.168.1.100:6380\r\n";
        let redirect = RedirectParser::parse_redirect_bytes(ask_data).unwrap();

        match redirect {
            RedirectType::Ask { slot, address } => {
                assert_eq!(slot, 12345);
                assert_eq!(address, "192.168.1.100:6380");
            }
            _ => panic!("Expected ASK redirect"),
        }
    }

    #[test]
    fn test_parse_redirect_raw() {
        let response = b"-MOVED 3999 127.0.0.1:6381\r\n";
        let redirect = RedirectParser::parse_redirect_raw(response).unwrap();
        assert!(matches!(redirect, RedirectType::Moved { .. }));

        // Non-error response
        let response = b"+OK\r\n";
        let redirect = RedirectParser::parse_redirect_raw(response);
        assert!(redirect.is_none());
    }

    #[test]
    fn test_redirection_context() {
        let mut context = RedirectionContext::new(3999, 3);

        assert_eq!(context.original_slot, 3999);
        assert_eq!(context.redirect_count, 0);
        assert!(context.can_redirect());

        // Add redirects up to the limit
        assert!(context.add_redirect("127.0.0.1:6380").is_ok());
        assert_eq!(context.redirect_count, 1);
        assert!(context.can_redirect());

        assert!(context.add_redirect("127.0.0.1:6381").is_ok());
        assert_eq!(context.redirect_count, 2);
        assert!(context.can_redirect());

        assert!(context.add_redirect("127.0.0.1:6382").is_ok());
        assert_eq!(context.redirect_count, 3);
        assert!(!context.can_redirect());

        // Exceed the limit
        assert!(context.add_redirect("127.0.0.1:6383").is_err());
    }

    #[test]
    fn test_redirection_new() {
        let moved = Redirection::new_move(3999, "127.0.0.1:6381".to_string());
        assert!(!moved.requires_asking);
        assert!(matches!(moved.target, RedirectType::Moved { .. }));

        let ask = Redirection::new_ask(3999, "127.0.0.1:6381".to_string());
        assert!(ask.requires_asking);
        assert!(matches!(ask.target, RedirectType::Ask { .. }));
    }

    #[test]
    fn test_create_asking_command_bytes() {
        let asking_cmd = RedirectHandler::create_asking_command_bytes();
        assert_eq!(asking_cmd, Bytes::from("*1\r\n$6\r\nASKING\r\n"));
    }

    #[test]
    fn test_validate_node_address() {
        // Valid addresses
        assert!(RedirectHandler::validate_node_address("127.0.0.1:6379").is_ok());
        assert!(RedirectHandler::validate_node_address("redis.example.com:6380").is_ok());

        // Invalid addresses
        assert!(RedirectHandler::validate_node_address("127.0.0.1").is_err()); // Missing port
        assert!(RedirectHandler::validate_node_address(":6379").is_err()); // Empty host
        assert!(RedirectHandler::validate_node_address("127.0.0.1:invalid").is_err());
        // Invalid port
    }

    #[tokio::test]
    async fn test_handle_redirection() {
        let handler = RedirectHandler::new(3);
        let mut context = RedirectionContext::new(3999, 3);

        // Test MOVED redirection
        let moved = RedirectType::Moved {
            slot: 3999,
            address: "127.0.0.1:6381".to_string(),
        };
        let redirection = handler
            .handle_redirection(moved, &mut context)
            .await
            .unwrap();
        assert!(!redirection.requires_asking);
        assert_eq!(context.redirect_count, 1);

        // Test ASK redirection
        let ask = RedirectType::Ask {
            slot: 3999,
            address: "192.168.1.100:6380".to_string(),
        };
        let redirection = handler.handle_redirection(ask, &mut context).await.unwrap();
        assert!(redirection.requires_asking);
        assert_eq!(context.redirect_count, 2);
    }

    #[test]
    fn test_parse_redirect_edge_cases() {
        // Empty data
        assert!(RedirectParser::parse_redirect_bytes(b"").is_none());

        // No space after pattern
        assert!(RedirectParser::parse_redirect_bytes(b"MOVED3999 127.0.0.1:6381\r\n").is_none());

        // Missing address
        assert!(RedirectParser::parse_redirect_bytes(b"MOVED 3999\r\n").is_none());

        // Slot number at boundary using parse_redirect_raw for full response
        let data = b"-MOVED 16383 127.0.0.1:6381\r\n";
        let redirect = RedirectParser::parse_redirect_raw(data).unwrap();
        match redirect {
            RedirectType::Moved { slot, .. } => assert_eq!(slot, 16383),
            _ => panic!("Expected MOVED redirect"),
        }

        // Slot 0 using parse_redirect_raw for full response
        let data = b"-ASK 0 127.0.0.1:6381\r\n";
        let redirect = RedirectParser::parse_redirect_raw(data).unwrap();
        match redirect {
            RedirectType::Ask { slot, .. } => assert_eq!(slot, 0),
            _ => panic!("Expected ASK redirect"),
        }
    }
}
