/// Utility functions and helpers
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique ID based on timestamp and random component
pub fn generate_id(prefix: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let random: u32 = rand::random();
    format!("{}-{}-{:x}", prefix, timestamp, random)
}

/// Calculate CRC16 checksum (used for Redis slot calculation)
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Extract hash tag from Redis key for consistent hashing
pub fn extract_hash_tag(key: &str) -> &str {
    if let (Some(start), Some(end)) = (key.find('{'), key.find('}')) {
        if end > start + 1 {
            return &key[start + 1..end];
        }
    }
    key
}

/// Parse socket address with error handling
pub fn parse_socket_addr(addr: &str) -> Result<std::net::SocketAddr, std::net::AddrParseError> {
    addr.parse()
}

/// Format duration for human-readable output
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m{}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// Format byte size for human-readable output
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16() {
        // These values should match Redis cluster slot calculation
        assert_eq!(crc16(b"123456789"), 12739);
        assert_eq!(crc16(b"foo"), 44950);
    }

    #[test]
    fn test_extract_hash_tag() {
        assert_eq!(extract_hash_tag("foo{bar}baz"), "bar");
        assert_eq!(extract_hash_tag("no_tag"), "no_tag");
        assert_eq!(extract_hash_tag("empty{}tag"), "empty{}tag");
        assert_eq!(extract_hash_tag("{user1000}.following"), "user1000");
    }

    #[test]
    fn test_format_duration() {
        use std::time::Duration;

        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h1m1s");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_generate_id() {
        let id1 = generate_id("test");
        let id2 = generate_id("test");

        assert!(id1.starts_with("test-"));
        assert!(id2.starts_with("test-"));
        assert_ne!(id1, id2); // Should be unique
    }
}
