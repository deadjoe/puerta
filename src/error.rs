/// Unified error handling for Puerta proxy
/// 
/// This module provides a comprehensive error type system that covers all
/// error scenarios in the Puerta proxy, including network errors, protocol
/// errors, configuration errors, and operational errors.

use std::fmt;
use std::io;
use std::net::AddrParseError;
use thiserror::Error;

/// Main error type for Puerta proxy operations
#[derive(Debug, Error)]
pub enum PuertaError {
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(#[from] io::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// Protocol parsing errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Backend connection errors
    #[error("Backend error: {message}")]
    Backend { message: String },

    /// Health check errors
    #[error("Health check failed: {message}")]
    HealthCheck { message: String },

    /// Session affinity errors
    #[error("Session affinity error: {message}")]
    SessionAffinity { message: String },

    /// Load balancing errors
    #[error("Load balancing error: {message}")]
    LoadBalancing { message: String },

    /// Redis-specific errors
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),

    /// MongoDB-specific errors
    #[error("MongoDB error: {0}")]
    MongoDB(#[from] MongoDBError),

    /// Address parsing errors
    #[error("Address parsing error: {0}")]
    AddressParse(#[from] AddrParseError),

    /// Timeout errors
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String },

    /// Internal errors (should not happen in normal operation)
    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Configuration-specific errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Serialize error: {0}")]
    SerializeError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Redis-specific errors
#[derive(Debug, Error)]
pub enum RedisError {
    #[error("RESP protocol error: {message}")]
    ProtocolError { message: String },

    #[error("Cluster topology error: {message}")]
    ClusterError { message: String },

    #[error("Redis command error: {command} - {message}")]
    CommandError { command: String, message: String },

    #[error("Slot mapping error: {message}")]
    SlotMappingError { message: String },

    #[error("Redirection error: {redirect_type} to {target}")]
    RedirectionError { redirect_type: String, target: String },
}

/// MongoDB-specific errors
#[derive(Debug, Error)]
pub enum MongoDBError {
    #[error("Wire protocol error: {message}")]
    ProtocolError { message: String },

    #[error("Authentication error: {message}")]
    AuthError { message: String },

    #[error("Database operation error: {message}")]
    DatabaseError { message: String },

    #[error("Replica set error: {message}")]
    ReplicaSetError { message: String },
}

/// Result type alias for Puerta operations
pub type PuertaResult<T> = Result<T, PuertaError>;

/// Convenience methods for creating specific error types
impl PuertaError {
    /// Create a backend error
    pub fn backend<S: Into<String>>(message: S) -> Self {
        PuertaError::Backend {
            message: message.into(),
        }
    }

    /// Create a health check error
    pub fn health_check<S: Into<String>>(message: S) -> Self {
        PuertaError::HealthCheck {
            message: message.into(),
        }
    }

    /// Create a session affinity error
    pub fn session_affinity<S: Into<String>>(message: S) -> Self {
        PuertaError::SessionAffinity {
            message: message.into(),
        }
    }

    /// Create a load balancing error
    pub fn load_balancing<S: Into<String>>(message: S) -> Self {
        PuertaError::LoadBalancing {
            message: message.into(),
        }
    }

    /// Create a protocol error
    pub fn protocol<S: Into<String>>(message: S) -> Self {
        PuertaError::Protocol(message.into())
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S) -> Self {
        PuertaError::Timeout {
            operation: operation.into(),
        }
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(message: S) -> Self {
        PuertaError::Internal {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable (can retry)
    pub fn is_recoverable(&self) -> bool {
        match self {
            PuertaError::Network(_) => true,
            PuertaError::Backend { .. } => true,
            PuertaError::HealthCheck { .. } => true,
            PuertaError::Timeout { .. } => true,
            PuertaError::Redis(RedisError::RedirectionError { .. }) => true,
            PuertaError::Redis(RedisError::ClusterError { .. }) => true,
            _ => false,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            PuertaError::Config(_) => ErrorSeverity::Critical,
            PuertaError::Internal { .. } => ErrorSeverity::Critical,
            PuertaError::Network(_) => ErrorSeverity::Warning,
            PuertaError::Backend { .. } => ErrorSeverity::Warning,
            PuertaError::HealthCheck { .. } => ErrorSeverity::Info,
            PuertaError::Timeout { .. } => ErrorSeverity::Warning,
            _ => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels for logging and monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Critical errors that require immediate attention
    Critical,
    /// Errors that affect functionality but don't crash the system
    Error,
    /// Warnings about potential issues
    Warning,
    /// Informational messages about recoverable issues
    Info,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Info => write!(f, "INFO"),
        }
    }
}

/// Convenience methods for creating Redis errors
impl RedisError {
    pub fn protocol<S: Into<String>>(message: S) -> Self {
        RedisError::ProtocolError {
            message: message.into(),
        }
    }

    pub fn cluster<S: Into<String>>(message: S) -> Self {
        RedisError::ClusterError {
            message: message.into(),
        }
    }

    pub fn command<S: Into<String>>(command: S, message: S) -> Self {
        RedisError::CommandError {
            command: command.into(),
            message: message.into(),
        }
    }

    pub fn slot_mapping<S: Into<String>>(message: S) -> Self {
        RedisError::SlotMappingError {
            message: message.into(),
        }
    }

    pub fn redirection<S: Into<String>>(redirect_type: S, target: S) -> Self {
        RedisError::RedirectionError {
            redirect_type: redirect_type.into(),
            target: target.into(),
        }
    }
}

/// Convenience methods for creating MongoDB errors
impl MongoDBError {
    pub fn protocol<S: Into<String>>(message: S) -> Self {
        MongoDBError::ProtocolError {
            message: message.into(),
        }
    }

    pub fn auth<S: Into<String>>(message: S) -> Self {
        MongoDBError::AuthError {
            message: message.into(),
        }
    }

    pub fn database<S: Into<String>>(message: S) -> Self {
        MongoDBError::DatabaseError {
            message: message.into(),
        }
    }

    pub fn replica_set<S: Into<String>>(message: S) -> Self {
        MongoDBError::ReplicaSetError {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = PuertaError::backend("Backend unavailable");
        assert!(matches!(error, PuertaError::Backend { .. }));
        assert_eq!(error.to_string(), "Backend error: Backend unavailable");
    }

    #[test]
    fn test_error_severity() {
        let config_error = PuertaError::Config(ConfigError::ValidationError("test".to_string()));
        assert_eq!(config_error.severity(), ErrorSeverity::Critical);

        let network_error = PuertaError::Network(io::Error::new(io::ErrorKind::ConnectionRefused, "test"));
        assert_eq!(network_error.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_error_recoverability() {
        let network_error = PuertaError::Network(io::Error::new(io::ErrorKind::ConnectionRefused, "test"));
        assert!(network_error.is_recoverable());

        let config_error = PuertaError::Config(ConfigError::ValidationError("test".to_string()));
        assert!(!config_error.is_recoverable());
    }

    #[test]
    fn test_redis_error_creation() {
        let redis_error = RedisError::protocol("Invalid RESP format");
        let puerta_error = PuertaError::Redis(redis_error);
        assert!(matches!(puerta_error, PuertaError::Redis(_)));
    }

    #[test]
    fn test_mongodb_error_creation() {
        let mongodb_error = MongoDBError::protocol("Invalid wire protocol message");
        let puerta_error = PuertaError::MongoDB(mongodb_error);
        assert!(matches!(puerta_error, PuertaError::MongoDB(_)));
    }
}
