/// Configuration management for puerta

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Main puerta configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,
    /// Proxy mode configuration
    pub proxy: ProxyConfig,
    /// Health check configuration
    pub health: HealthConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address to listen on
    pub listen_addr: String,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout_sec: u64,
    /// Number of worker threads
    pub worker_threads: Option<usize>,
}

/// Proxy mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum ProxyConfig {
    #[serde(rename = "mongodb")]
    MongoDB {
        /// List of mongos endpoints
        mongos_endpoints: Vec<String>,
        /// Enable session affinity
        session_affinity: bool,
        /// Session timeout in seconds
        session_timeout_sec: u64,
    },
    #[serde(rename = "redis")]
    Redis {
        /// List of Redis cluster nodes
        cluster_nodes: Vec<String>,
        /// Slot refresh interval in seconds
        slot_refresh_interval_sec: u64,
        /// Maximum number of redirects to follow
        max_redirects: u8,
        /// Connection timeout in milliseconds
        connection_timeout_ms: u64,
    },
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Health check interval in seconds
    pub interval_sec: u64,
    /// Health check timeout in seconds
    pub timeout_sec: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (error, warn, info, debug, trace)
    pub level: String,
    /// Log format (json, text)
    pub format: String,
    /// Log to stdout
    pub stdout: bool,
    /// Log file path (optional)
    pub file: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "0.0.0.0:8080".to_string(),
                max_connections: 10000,
                connection_timeout_sec: 60,
                worker_threads: None, // Use system default
            },
            proxy: ProxyConfig::MongoDB {
                mongos_endpoints: vec!["127.0.0.1:27017".to_string()],
                session_affinity: true,
                session_timeout_sec: 3600,
            },
            health: HealthConfig {
                interval_sec: 10,
                timeout_sec: 5,
                failure_threshold: 3,
                success_threshold: 2,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "text".to_string(),
                stdout: true,
                file: None,
            },
        }
    }
}

impl Config {
    /// Load configuration from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        let config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializeError(e.to_string()))?;
        
        fs::write(path, content)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate server config
        if self.server.max_connections == 0 {
            return Err(ConfigError::ValidationError(
                "max_connections must be greater than 0".to_string(),
            ));
        }

        if self.server.connection_timeout_sec == 0 {
            return Err(ConfigError::ValidationError(
                "connection_timeout_sec must be greater than 0".to_string(),
            ));
        }

        // Validate proxy config
        match &self.proxy {
            ProxyConfig::MongoDB { mongos_endpoints, .. } => {
                if mongos_endpoints.is_empty() {
                    return Err(ConfigError::ValidationError(
                        "mongos_endpoints cannot be empty".to_string(),
                    ));
                }
                
                for endpoint in mongos_endpoints {
                    endpoint.parse::<std::net::SocketAddr>()
                        .map_err(|_| ConfigError::ValidationError(
                            format!("Invalid mongos endpoint: {}", endpoint)
                        ))?;
                }
            }
            ProxyConfig::Redis { cluster_nodes, max_redirects, .. } => {
                if cluster_nodes.is_empty() {
                    return Err(ConfigError::ValidationError(
                        "cluster_nodes cannot be empty".to_string(),
                    ));
                }
                
                for node in cluster_nodes {
                    node.parse::<std::net::SocketAddr>()
                        .map_err(|_| ConfigError::ValidationError(
                            format!("Invalid Redis node: {}", node)
                        ))?;
                }

                if *max_redirects == 0 {
                    return Err(ConfigError::ValidationError(
                        "max_redirects must be greater than 0".to_string(),
                    ));
                }
            }
        }

        // Validate health config
        if self.health.interval_sec == 0 {
            return Err(ConfigError::ValidationError(
                "health check interval_sec must be greater than 0".to_string(),
            ));
        }

        if self.health.timeout_sec == 0 {
            return Err(ConfigError::ValidationError(
                "health check timeout_sec must be greater than 0".to_string(),
            ));
        }

        if self.health.timeout_sec >= self.health.interval_sec {
            return Err(ConfigError::ValidationError(
                "health check timeout_sec must be less than interval_sec".to_string(),
            ));
        }

        // Validate logging config
        match self.logging.level.as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            _ => return Err(ConfigError::ValidationError(
                format!("Invalid log level: {}", self.logging.level)
            )),
        }

        match self.logging.format.as_str() {
            "json" | "text" => {}
            _ => return Err(ConfigError::ValidationError(
                format!("Invalid log format: {}", self.logging.format)
            )),
        }

        Ok(())
    }

    /// Create example configuration file
    pub fn create_example_config<P: AsRef<Path>>(path: P, mode: &str) -> Result<(), ConfigError> {
        let config = match mode {
            "mongodb" => Config {
                proxy: ProxyConfig::MongoDB {
                    mongos_endpoints: vec![
                        "10.0.1.10:27017".to_string(),
                        "10.0.1.11:27017".to_string(),
                        "10.0.1.12:27017".to_string(),
                    ],
                    session_affinity: true,
                    session_timeout_sec: 3600,
                },
                ..Default::default()
            },
            "redis" => Config {
                proxy: ProxyConfig::Redis {
                    cluster_nodes: vec![
                        "10.0.1.20:6379".to_string(),
                        "10.0.1.21:6379".to_string(),
                        "10.0.1.22:6379".to_string(),
                    ],
                    slot_refresh_interval_sec: 60,
                    max_redirects: 3,
                    connection_timeout_ms: 5000,
                },
                ..Default::default()
            },
            _ => return Err(ConfigError::ValidationError(
                "Mode must be 'mongodb' or 'redis'".to_string(),
            )),
        };

        config.save_to_file(path)
    }
}

/// Configuration error types
#[derive(Debug, thiserror::Error)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Test invalid max_connections
        config.server.max_connections = 0;
        assert!(config.validate().is_err());
        
        config.server.max_connections = 1000;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed_config: Config = toml::from_str(&toml_str).unwrap();
        assert!(parsed_config.validate().is_ok());
    }

    #[test]
    fn test_config_file_operations() {
        let config = Config::default();
        let temp_file = NamedTempFile::new().unwrap();
        
        // Test save and load
        config.save_to_file(temp_file.path()).unwrap();
        let loaded_config = Config::load_from_file(temp_file.path()).unwrap();
        assert!(loaded_config.validate().is_ok());
    }
}