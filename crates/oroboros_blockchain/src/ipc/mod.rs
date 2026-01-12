//! # IPC (Unix Socket) Blockchain Listener
//!
//! ARCHITECT'S ORDER: No cloud RPC. Connect directly to local Geth/Reth node.
//!
//! ## Why IPC over HTTP?
//!
//! | Method      | Latency    | Why                                      |
//! |-------------|------------|------------------------------------------|
//! | HTTP RPC    | 50-500µs   | TCP handshake, JSON parsing              |
//! | WebSocket   | 10-50µs    | Persistent connection, still JSON        |
//! | **IPC**     | **1-5µs**  | Unix socket, no network stack            |
//!
//! IPC uses Unix domain sockets - no TCP, no network, just kernel pipes.
//!
//! ## Default Paths
//!
//! - Geth: `~/.ethereum/geth.ipc`
//! - Reth: `~/.local/share/reth/mainnet/reth.ipc`
//! - Anvil: `/tmp/anvil.ipc` (with `--ipc` flag)

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};

use crate::events::BlockchainEvent;

/// IPC connection configuration.
#[derive(Clone, Debug)]
pub struct IpcConfig {
    /// Path to the IPC socket file.
    pub socket_path: String,
    /// Read timeout for blocking operations.
    pub read_timeout: Option<Duration>,
    /// Write timeout for blocking operations.
    pub write_timeout: Option<Duration>,
    /// Size of the event channel buffer.
    pub channel_buffer: usize,
    /// Contract address to filter events (optional).
    pub contract_filter: Option<[u8; 20]>,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            // Default to Geth's IPC path
            socket_path: "/root/.ethereum/geth.ipc".to_string(),
            read_timeout: Some(Duration::from_millis(100)),
            write_timeout: Some(Duration::from_millis(100)),
            channel_buffer: 4096,
            contract_filter: None,
        }
    }
}

impl IpcConfig {
    /// Creates config for Geth.
    #[must_use]
    pub fn geth() -> Self {
        Self {
            socket_path: format!("{}/.ethereum/geth.ipc", std::env::var("HOME").unwrap_or_default()),
            ..Default::default()
        }
    }

    /// Creates config for Reth.
    #[must_use]
    pub fn reth() -> Self {
        Self {
            socket_path: format!("{}/.local/share/reth/mainnet/reth.ipc", std::env::var("HOME").unwrap_or_default()),
            ..Default::default()
        }
    }

    /// Creates config for local Anvil (testing).
    #[must_use]
    pub fn anvil() -> Self {
        Self {
            socket_path: "/tmp/anvil.ipc".to_string(),
            ..Default::default()
        }
    }

    /// Sets a custom socket path.
    #[must_use]
    pub fn with_socket_path(mut self, path: impl Into<String>) -> Self {
        self.socket_path = path.into();
        self
    }

    /// Sets a contract address filter.
    #[must_use]
    pub fn with_contract_filter(mut self, address: [u8; 20]) -> Self {
        self.contract_filter = Some(address);
        self
    }
}

/// Statistics for the IPC listener.
#[derive(Debug, Default)]
pub struct IpcStats {
    /// Total messages received.
    pub messages_received: AtomicU64,
    /// Total events parsed.
    pub events_parsed: AtomicU64,
    /// Average latency from event to callback (µs).
    pub avg_latency_us: AtomicU64,
    /// Maximum observed latency (µs).
    pub max_latency_us: AtomicU64,
    /// Connection errors.
    pub connection_errors: AtomicU64,
    /// Parse errors.
    pub parse_errors: AtomicU64,
}

/// High-performance IPC listener for local Ethereum nodes.
///
/// This listener connects directly to Geth/Reth via Unix socket,
/// bypassing all network overhead for minimum latency.
///
/// ## Usage
///
/// ```rust,ignore
/// let config = IpcConfig::geth()
///     .with_contract_filter(contract_address);
///
/// let listener = IpcListener::new(config)?;
/// let receiver = listener.subscribe();
///
/// // In game loop:
/// while let Ok((event, timestamp)) = receiver.try_recv() {
///     let latency = timestamp.elapsed();
///     // Process event...
/// }
/// ```
pub struct IpcListener {
    /// Configuration.
    config: IpcConfig,
    /// Event sender (used when streaming events).
    #[allow(dead_code)]
    sender: Sender<(BlockchainEvent, Instant)>,
    /// Event receiver (cloneable).
    receiver: Receiver<(BlockchainEvent, Instant)>,
    /// Whether the listener is running.
    running: Arc<AtomicBool>,
    /// Statistics.
    stats: Arc<IpcStats>,
}

impl IpcListener {
    /// Creates a new IPC listener with the given configuration.
    ///
    /// Note: This doesn't connect immediately. Call `connect()` or `start()` to connect.
    #[must_use]
    pub fn new(config: IpcConfig) -> Self {
        let (sender, receiver) = bounded(config.channel_buffer);

        Self {
            config,
            sender,
            receiver,
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(IpcStats::default()),
        }
    }

    /// Returns a clone of the event receiver.
    ///
    /// Multiple receivers can be created for fan-out.
    #[must_use]
    pub fn subscribe(&self) -> Receiver<(BlockchainEvent, Instant)> {
        self.receiver.clone()
    }

    /// Returns a reference to the statistics.
    #[must_use]
    pub fn stats(&self) -> Arc<IpcStats> {
        Arc::clone(&self.stats)
    }

    /// Checks if the listener is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stops the listener.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Tests connectivity to the IPC socket.
    ///
    /// Returns Ok(latency) if successful, Err otherwise.
    pub fn test_connection(&self) -> Result<Duration, IpcError> {
        let start = Instant::now();

        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        if let Some(timeout) = self.config.write_timeout {
            stream.set_write_timeout(Some(timeout))?;
        }
        if let Some(timeout) = self.config.read_timeout {
            stream.set_read_timeout(Some(timeout))?;
        }

        // Send a simple eth_blockNumber request
        let request = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
        writeln!(stream, "{}", request)?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        let latency = start.elapsed();

        if response.contains("result") {
            Ok(latency)
        } else {
            Err(IpcError::InvalidResponse(response))
        }
    }

    /// Subscribes to new pending transactions (for MEV).
    ///
    /// Returns the subscription ID if successful.
    pub fn subscribe_pending_transactions(&self) -> Result<String, IpcError> {
        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        let request = r#"{"jsonrpc":"2.0","method":"eth_subscribe","params":["newPendingTransactions"],"id":1}"#;
        writeln!(stream, "{}", request)?;
        stream.flush()?;

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        // Parse subscription ID from response
        if let Some(start) = response.find("\"result\":\"") {
            let start = start + 10;
            if let Some(end) = response[start..].find('"') {
                return Ok(response[start..start + end].to_string());
            }
        }

        Err(IpcError::SubscriptionFailed(response))
    }

    /// Subscribes to logs for a specific contract.
    ///
    /// This is the key method for getting NFT events.
    pub fn subscribe_logs(&self, contract_address: Option<[u8; 20]>) -> Result<String, IpcError> {
        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        let address = contract_address.or(self.config.contract_filter);
        let address_hex = address
            .map(|a| format!("0x{}", hex::encode(&a)))
            .unwrap_or_default();

        let request = if address_hex.is_empty() {
            r#"{"jsonrpc":"2.0","method":"eth_subscribe","params":["logs",{}],"id":1}"#.to_string()
        } else {
            format!(
                r#"{{"jsonrpc":"2.0","method":"eth_subscribe","params":["logs",{{"address":"{}"}}],"id":1}}"#,
                address_hex
            )
        };

        writeln!(stream, "{}", request)?;
        stream.flush()?;

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        if let Some(start) = response.find("\"result\":\"") {
            let start = start + 10;
            if let Some(end) = response[start..].find('"') {
                return Ok(response[start..start + end].to_string());
            }
        }

        Err(IpcError::SubscriptionFailed(response))
    }

    /// Gets the current block number via IPC.
    ///
    /// This is the fastest way to know the chain state.
    pub fn get_block_number(&self) -> Result<u64, IpcError> {
        let mut stream = UnixStream::connect(&self.config.socket_path)
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        let request = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
        writeln!(stream, "{}", request)?;
        stream.flush()?;

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        // Parse block number from response
        if let Some(start) = response.find("\"result\":\"0x") {
            let start = start + 12;
            if let Some(end) = response[start..].find('"') {
                let hex_str = &response[start..start + end];
                let block_num = u64::from_str_radix(hex_str, 16)
                    .map_err(|e| IpcError::ParseError(e.to_string()))?;
                return Ok(block_num);
            }
        }

        Err(IpcError::InvalidResponse(response))
    }

    /// Benchmarks the IPC latency by sending multiple requests.
    ///
    /// Returns (min, avg, max) latency in microseconds.
    pub fn benchmark_latency(&self, iterations: usize) -> Result<(u64, u64, u64), IpcError> {
        let mut latencies = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = self.get_block_number()?;
            latencies.push(start.elapsed().as_micros() as u64);
        }

        let min = *latencies.iter().min().unwrap_or(&0);
        let max = *latencies.iter().max().unwrap_or(&0);
        let avg = latencies.iter().sum::<u64>() / latencies.len().max(1) as u64;

        Ok((min, avg, max))
    }
}

/// IPC-specific errors.
#[derive(Debug, Clone)]
pub enum IpcError {
    /// Failed to connect to IPC socket.
    ConnectionFailed(String),
    /// Socket not found.
    SocketNotFound(String),
    /// Read/write timeout.
    Timeout,
    /// Invalid response from node.
    InvalidResponse(String),
    /// Failed to subscribe.
    SubscriptionFailed(String),
    /// Parse error.
    ParseError(String),
    /// IO error.
    IoError(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(s) => write!(f, "Connection failed: {}", s),
            Self::SocketNotFound(s) => write!(f, "Socket not found: {}", s),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::InvalidResponse(s) => write!(f, "Invalid response: {}", s),
            Self::SubscriptionFailed(s) => write!(f, "Subscription failed: {}", s),
            Self::ParseError(s) => write!(f, "Parse error: {}", s),
            Self::IoError(s) => write!(f, "IO error: {}", s),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            Self::SocketNotFound(e.to_string())
        } else if e.kind() == std::io::ErrorKind::TimedOut {
            Self::Timeout
        } else {
            Self::IoError(e.to_string())
        }
    }
}

/// Utility to encode hex strings.
mod hex {
    /// Encodes bytes to hex string (no 0x prefix).
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Decodes hex string to bytes.
    #[allow(dead_code)]
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("Invalid hex: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = IpcConfig::geth();
        assert!(config.socket_path.contains("geth.ipc"));

        let config = IpcConfig::anvil();
        assert_eq!(config.socket_path, "/tmp/anvil.ipc");
    }

    #[test]
    fn test_hex_encode_decode() {
        let bytes = [0xde, 0xad, 0xbe, 0xef];
        let encoded = hex::encode(&bytes);
        assert_eq!(encoded, "deadbeef");

        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(decoded, bytes);

        let decoded = hex::decode("0xdeadbeef").unwrap();
        assert_eq!(decoded, bytes);
    }
}
