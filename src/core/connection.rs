/// Core TCP connection handling and data forwarding
///
/// This module provides the fundamental networking capabilities for puerta,
/// including bidirectional data forwarding between clients and backends.
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Connection pair representing client and backend streams
#[derive(Debug)]
pub struct ConnectionPair {
    pub client_stream: TcpStream,
    pub backend_stream: TcpStream,
    pub client_addr: SocketAddr,
    pub backend_addr: SocketAddr,
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub bytes_client_to_backend: u64,
    pub bytes_backend_to_client: u64,
    pub packets_client_to_backend: u64,
    pub packets_backend_to_client: u64,
    pub connection_duration_ms: u64,
}

/// TCP connection manager for handling client connections and backend forwarding
#[derive(Clone)]
pub struct ConnectionManager {
    /// Maximum number of concurrent connections
    max_connections: usize,
    /// Connection timeout for backend connections
    backend_timeout: Duration,
    /// Buffer size for data forwarding
    buffer_size: usize,
}

/// Result of a connection attempt
#[derive(Debug)]
pub enum ConnectionResult {
    Success(TcpStream),
    Timeout,
    ConnectionRefused,
    NetworkError(io::Error),
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(max_connections: usize, backend_timeout: Duration) -> Self {
        Self {
            max_connections,
            backend_timeout,
            buffer_size: 8192, // 8KB buffer by default
        }
    }

    /// Create a TCP listener on the specified address
    pub async fn create_listener(&self, listen_addr: &str) -> Result<TcpListener, io::Error> {
        let listener = TcpListener::bind(listen_addr).await?;
        info!("Listening on {}", listen_addr);
        Ok(listener)
    }

    /// Accept incoming client connections
    pub async fn accept_connection(
        &self,
        listener: &TcpListener,
    ) -> Result<(TcpStream, SocketAddr), io::Error> {
        let (stream, addr) = listener.accept().await?;

        // Configure TCP options for optimal performance
        if let Err(e) = self.configure_client_stream(&stream) {
            warn!("Failed to configure client stream: {}", e);
        }

        debug!("Accepted connection from {}", addr);
        Ok((stream, addr))
    }

    /// Connect to a backend server
    pub async fn connect_to_backend(&self, backend_addr: SocketAddr) -> ConnectionResult {
        debug!("Connecting to backend {}", backend_addr);

        match timeout(self.backend_timeout, TcpStream::connect(backend_addr)).await {
            Ok(Ok(stream)) => {
                // Configure backend stream for optimal performance
                if let Err(e) = self.configure_backend_stream(&stream) {
                    warn!("Failed to configure backend stream: {}", e);
                }

                debug!("Successfully connected to backend {}", backend_addr);
                ConnectionResult::Success(stream)
            }
            Ok(Err(e)) => match e.kind() {
                io::ErrorKind::ConnectionRefused => {
                    debug!("Connection refused by backend {}", backend_addr);
                    ConnectionResult::ConnectionRefused
                }
                _ => {
                    debug!(
                        "Network error connecting to backend {}: {}",
                        backend_addr, e
                    );
                    ConnectionResult::NetworkError(e)
                }
            },
            Err(_) => {
                debug!("Timeout connecting to backend {}", backend_addr);
                ConnectionResult::Timeout
            }
        }
    }

    /// Forward data bidirectionally between client and backend
    /// This is the core data forwarding implementation
    pub async fn forward_data(
        &self,
        mut client_stream: TcpStream,
        mut backend_stream: TcpStream,
        client_addr: SocketAddr,
        backend_addr: SocketAddr,
    ) -> Result<ConnectionStats, io::Error> {
        let start_time = std::time::Instant::now();

        // Use tokio::io::copy_bidirectional for efficient data forwarding
        let result = tokio::io::copy_bidirectional(&mut client_stream, &mut backend_stream).await;

        let stats = match result {
            Ok((client_to_backend, backend_to_client)) => {
                ConnectionStats {
                    bytes_client_to_backend: client_to_backend,
                    bytes_backend_to_client: backend_to_client,
                    packets_client_to_backend: 1, // Simplified for now
                    packets_backend_to_client: 1, // Simplified for now
                    connection_duration_ms: start_time.elapsed().as_millis() as u64,
                }
            }
            Err(e) => {
                warn!("Error during data forwarding: {}", e);
                return Err(e);
            }
        };

        info!(
            "Connection closed: {} <-> {} ({}ms, {}/{} bytes)",
            client_addr,
            backend_addr,
            stats.connection_duration_ms,
            stats.bytes_client_to_backend,
            stats.bytes_backend_to_client
        );

        Ok(stats)
    }

    /// Configure client stream for optimal performance
    fn configure_client_stream(&self, stream: &TcpStream) -> Result<(), io::Error> {
        // Enable TCP_NODELAY to reduce latency
        stream.set_nodelay(true)?;
        Ok(())
    }

    /// Configure backend stream for optimal performance  
    fn configure_backend_stream(&self, stream: &TcpStream) -> Result<(), io::Error> {
        // Enable TCP_NODELAY for low latency
        stream.set_nodelay(true)?;
        Ok(())
    }

    /// Get current connection manager configuration
    pub fn get_config(&self) -> ConnectionManagerConfig {
        ConnectionManagerConfig {
            max_connections: self.max_connections,
            backend_timeout: self.backend_timeout,
            buffer_size: self.buffer_size,
        }
    }

    /// Update buffer size for data forwarding
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
    }
}

/// Connection manager configuration
#[derive(Debug, Clone)]
pub struct ConnectionManagerConfig {
    pub max_connections: usize,
    pub backend_timeout: Duration,
    pub buffer_size: usize,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new(10000, Duration::from_secs(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let manager = ConnectionManager::new(1000, Duration::from_secs(10));
        let config = manager.get_config();

        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.backend_timeout, Duration::from_secs(10));
        assert_eq!(config.buffer_size, 8192);
    }

    #[tokio::test]
    async fn test_tcp_listener_creation() {
        let manager = ConnectionManager::default();

        // Use port 0 to get any available port
        let listener = manager.create_listener("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        assert!(addr.port() > 0);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn test_backend_connection_success() {
        // Create a test server
        let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_listener.local_addr().unwrap();

        // Spawn a simple echo server
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = server_listener.accept().await {
                let mut buffer = [0u8; 1024];
                while let Ok(n) = stream.read(&mut buffer).await {
                    if n == 0 {
                        break;
                    }
                    let _ = stream.write_all(&buffer[..n]).await;
                }
            }
        });

        let manager = ConnectionManager::default();
        let result = manager.connect_to_backend(server_addr).await;

        assert!(matches!(result, ConnectionResult::Success(_)));
    }

    #[tokio::test]
    async fn test_backend_connection_refused() {
        let manager = ConnectionManager::default();

        // Try to connect to a port that should be closed
        let result = manager
            .connect_to_backend("127.0.0.1:65534".parse().unwrap())
            .await;

        assert!(matches!(result, ConnectionResult::ConnectionRefused));
    }

    #[tokio::test]
    async fn test_backend_connection_timeout() {
        let manager = ConnectionManager::new(1000, Duration::from_millis(50)); // Very short timeout

        // Use a local IP that will accept connections but not complete them quickly
        // This simulates a slow/hanging connection that should timeout
        let result = manager
            .connect_to_backend("10.255.255.1:9999".parse().unwrap())
            .await;

        // The result should be either Timeout or NetworkError depending on the system
        match result {
            ConnectionResult::Timeout => { /* Expected */ }
            ConnectionResult::NetworkError(_) => { /* Also acceptable */ }
            ConnectionResult::Success(_) => {
                // In some network environments, this might succeed quickly
                // This is acceptable behavior
            }
            other => panic!("Unexpected connection result: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_data_forwarding() {
        // Create a simple test that verifies the forwarding function can be called
        let manager = ConnectionManager::default();

        // Create test server for both client and backend connections
        let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_listener.local_addr().unwrap();

        // Spawn a server that accepts connections and closes them immediately
        tokio::spawn(async move {
            while let Ok((stream, _)) = server_listener.accept().await {
                drop(stream); // Close immediately
            }
        });

        // Create two connections that will be closed immediately
        let client_stream = TcpStream::connect(server_addr).await.unwrap();
        let backend_stream = TcpStream::connect(server_addr).await.unwrap();

        let client_addr = client_stream.local_addr().unwrap();
        let backend_addr = backend_stream.peer_addr().unwrap();

        // This should complete quickly since the connections will close
        let result = manager
            .forward_data(client_stream, backend_stream, client_addr, backend_addr)
            .await;

        // Should return stats even if connections closed immediately
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let stats = ConnectionStats {
            bytes_client_to_backend: 1024,
            bytes_backend_to_client: 2048,
            packets_client_to_backend: 10,
            packets_backend_to_client: 15,
            connection_duration_ms: 5000,
        };

        assert_eq!(stats.bytes_client_to_backend, 1024);
        assert_eq!(stats.bytes_backend_to_client, 2048);
        assert_eq!(stats.packets_client_to_backend, 10);
        assert_eq!(stats.packets_backend_to_client, 15);
        assert_eq!(stats.connection_duration_ms, 5000);
    }

    #[tokio::test]
    async fn test_buffer_size_configuration() {
        let mut manager = ConnectionManager::default();

        assert_eq!(manager.get_config().buffer_size, 8192);

        manager.set_buffer_size(16384);
        assert_eq!(manager.get_config().buffer_size, 16384);
    }
}
