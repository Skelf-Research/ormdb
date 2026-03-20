//! Server transport layer using async-nng.
//!
//! Provides TCP and IPC transport for the ORMDB server using NNG's REP socket.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use async_nng::AsyncContext;
use nng::options::Options;
use nng::{Message, Protocol, Socket};

use ormdb_proto::framing::encode_frame;
use ormdb_proto::{Request, Response};

use crate::config::ServerConfig;
use crate::error::Error;
use crate::handler::RequestHandler;

/// Transport metrics for monitoring.
#[derive(Debug)]
pub struct TransportMetrics {
    /// Total number of requests received.
    pub requests_total: AtomicU64,
    /// Number of successful requests.
    pub requests_success: AtomicU64,
    /// Number of failed requests.
    pub requests_failed: AtomicU64,
    /// Number of bytes received.
    pub bytes_received: AtomicU64,
    /// Number of bytes sent.
    pub bytes_sent: AtomicU64,
    /// Server start time.
    pub started_at: Instant,
}

impl TransportMetrics {
    /// Create new metrics.
    fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_success: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    /// Record a successful request.
    fn record_success(&self, received_bytes: usize, sent_bytes: usize) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_success.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(received_bytes as u64, Ordering::Relaxed);
        self.bytes_sent.fetch_add(sent_bytes as u64, Ordering::Relaxed);
    }

    /// Record a failed request.
    fn record_failure(&self, received_bytes: usize, sent_bytes: usize) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(received_bytes as u64, Ordering::Relaxed);
        self.bytes_sent.fetch_add(sent_bytes as u64, Ordering::Relaxed);
    }

    /// Get the uptime duration.
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Get total requests count.
    pub fn total_requests(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    /// Get successful requests count.
    pub fn successful_requests(&self) -> u64 {
        self.requests_success.load(Ordering::Relaxed)
    }

    /// Get failed requests count.
    pub fn failed_requests(&self) -> u64 {
        self.requests_failed.load(Ordering::Relaxed)
    }

    /// Get total bytes received.
    pub fn total_bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get total bytes sent.
    pub fn total_bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }
}

impl Default for TransportMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Server transport that handles incoming connections.
pub struct Transport {
    socket: Socket,
    handler: Arc<RequestHandler>,
    max_message_size: usize,
    metrics: Arc<TransportMetrics>,
    request_timeout: Duration,
    worker_count: usize,
}

impl Transport {
    /// Create a new transport with the given configuration and request handler.
    pub fn new(config: &ServerConfig, handler: Arc<RequestHandler>) -> Result<Self, Error> {
        // Create REP socket
        let socket = Socket::new(Protocol::Rep0)
            .map_err(|e| Error::Transport(format!("failed to create socket: {}", e)))?;

        // Set socket options
        socket
            .set_opt::<nng::options::RecvMaxSize>(config.max_message_size)
            .map_err(|e| Error::Transport(format!("failed to set max message size: {}", e)))?;

        // Bind to TCP address if configured
        if let Some(tcp_addr) = &config.tcp_address {
            socket
                .listen(tcp_addr)
                .map_err(|e| Error::Transport(format!("failed to listen on {}: {}", tcp_addr, e)))?;

            tracing::info!(address = %tcp_addr, "listening on TCP");
        }

        // Bind to IPC address if configured
        if let Some(ipc_addr) = &config.ipc_address {
            socket
                .listen(ipc_addr)
                .map_err(|e| Error::Transport(format!("failed to listen on {}: {}", ipc_addr, e)))?;

            tracing::info!(address = %ipc_addr, "listening on IPC");
        }

        Ok(Self {
            socket,
            handler,
            max_message_size: config.max_message_size,
            metrics: Arc::new(TransportMetrics::new()),
            request_timeout: config.request_timeout,
            worker_count: config.transport_workers.max(1),
        })
    }

    /// Get a reference to the transport metrics.
    pub fn metrics(&self) -> &TransportMetrics {
        &self.metrics
    }

    /// Run the transport loop, processing incoming requests.
    pub async fn run(&self) -> Result<(), Error> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let _handles = self.spawn_worker_threads(stop_flag)?;

        tracing::info!("transport ready, accepting requests");
        std::future::pending::<()>().await;
        Ok(())
    }

    /// Run the transport with graceful shutdown support.
    pub async fn run_until_shutdown(
        &self,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), Error> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let handles = self.spawn_worker_threads(stop_flag.clone())?;

        tracing::info!("transport ready, accepting requests");

        let _ = shutdown.recv().await;
        tracing::info!(
            total_requests = self.metrics.total_requests(),
            successful = self.metrics.successful_requests(),
            failed = self.metrics.failed_requests(),
            bytes_received = self.metrics.total_bytes_received(),
            bytes_sent = self.metrics.total_bytes_sent(),
            uptime_secs = self.metrics.uptime().as_secs(),
            "shutdown signal received, stopping transport"
        );

        stop_flag.store(true, Ordering::SeqCst);
        let _ = tokio::task::spawn_blocking(move || {
            for handle in handles {
                let _ = handle.join();
            }
        })
        .await;

        Ok(())
    }

    /// Process a raw message and return the response bytes.
    fn process_message(&self, data: &[u8]) -> Vec<u8> {
        self.worker().process_message_with_status(data).0
    }

    /// Process a raw message and return (response bytes, is_success).
    fn process_message_with_status(&self, data: &[u8]) -> (Vec<u8>, bool) {
        self.worker().process_message_with_status(data)
    }

    fn worker(&self) -> TransportWorker {
        TransportWorker::new(self.handler.clone(), self.max_message_size)
    }

    fn spawn_worker_threads(
        &self,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<Vec<thread::JoinHandle<()>>, Error> {
        let mut handles = Vec::with_capacity(self.worker_count);
        for worker_id in 0..self.worker_count {
            let socket = self.socket.clone();
            let worker = self.worker();
            let metrics = self.metrics.clone();
            let request_timeout = self.request_timeout;
            let stop_flag = stop_flag.clone();

            let handle = thread::Builder::new()
                .name(format!("ormdb-transport-{}", worker_id))
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("failed to build transport worker runtime");

                    runtime.block_on(async move {
                        let mut ctx = match AsyncContext::try_from(&socket) {
                            Ok(ctx) => ctx,
                            Err(e) => {
                                tracing::error!(error = %e, worker_id, "failed to create async context");
                                return;
                            }
                        };

                        loop {
                            if stop_flag.load(Ordering::SeqCst) {
                                tracing::info!(worker_id, "transport worker stopping");
                                return;
                            }

                            match ctx.receive(Some(Duration::from_secs(1))).await {
                                Ok(msg) => {
                                    let received_bytes = msg.len();
                                    let start = Instant::now();
                                    let (response_bytes, is_success) =
                                        worker.process_message_with_status(msg.as_slice());
                                    let elapsed = start.elapsed();
                                    let sent_bytes = response_bytes.len();

                                    let response_msg = Message::from(response_bytes.as_slice());

                                    if let Err((_, e)) = ctx.send(response_msg, None).await {
                                        tracing::error!(error = %e, worker_id, "failed to send response");
                                        metrics.record_failure(received_bytes, 0);
                                    } else if is_success {
                                        metrics.record_success(received_bytes, sent_bytes);
                                    } else {
                                        metrics.record_failure(received_bytes, sent_bytes);
                                    }

                                    if elapsed > request_timeout {
                                        tracing::warn!(
                                            worker_id,
                                            duration_ms = elapsed.as_millis() as u64,
                                            timeout_ms = request_timeout.as_millis() as u64,
                                            "request exceeded timeout"
                                        );
                                    }
                                }
                                Err(nng::Error::TimedOut) => {
                                    continue;
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, worker_id, "receive error");
                                }
                            }
                        }
                    });
                })
                .map_err(|e| Error::Transport(format!("failed to spawn transport worker: {}", e)))?;

            handles.push(handle);
        }

        Ok(handles)
    }
}

struct TransportWorker {
    handler: Arc<RequestHandler>,
    max_message_size: usize,
}

impl TransportWorker {
    fn new(handler: Arc<RequestHandler>, max_message_size: usize) -> Self {
        Self {
            handler,
            max_message_size,
        }
    }

    /// Process a raw message and return (response bytes, is_success).
    fn process_message_with_status(&self, data: &[u8]) -> (Vec<u8>, bool) {
        // Decode and process the request
        let (response, is_success) = match self.decode_and_handle(data) {
            Ok(response) => {
                let is_ok = response.status.is_ok();
                (response, is_ok)
            }
            Err(e) => {
                tracing::error!(error = %e, "request processing error");
                // Return error response with request ID 0 (unknown)
                let response = Response::error(0, ormdb_proto::error_codes::INTERNAL, e.to_string());
                (response, false)
            }
        };

        // Serialize response
        let bytes = match self.encode_response(&response) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!(error = %e, "failed to encode response");
                // Try to send a minimal error response
                self.encode_minimal_error(&e.to_string())
            }
        };

        (bytes, is_success)
    }

    /// Decode a request and dispatch to handler.
    fn decode_and_handle(&self, data: &[u8]) -> Result<Response, Error> {
        // Check message size
        if data.len() > self.max_message_size {
            return Err(Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                "message too large: {} bytes (max: {})",
                data.len(),
                self.max_message_size
            ))));
        }

        // Extract payload from framed message
        let payload = ormdb_proto::framing::extract_payload(data)?;

        // Copy to aligned buffer for rkyv (required for zero-copy access)
        let mut aligned: rkyv::util::AlignedVec<16> = rkyv::util::AlignedVec::new();
        aligned.extend_from_slice(payload);

        // Deserialize request using rkyv
        let request: Request =
            rkyv::from_bytes::<Request, rkyv::rancor::Error>(&aligned).map_err(|e| {
                Error::Protocol(ormdb_proto::Error::InvalidMessage(format!(
                    "failed to deserialize request: {}",
                    e
                )))
            })?;

        // Handle the request
        Ok(self.handler.handle(&request))
    }

    /// Encode a response to framed bytes.
    fn encode_response(&self, response: &Response) -> Result<Vec<u8>, Error> {
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(response).map_err(|e| {
            Error::Protocol(ormdb_proto::Error::Serialization(format!(
                "failed to serialize response: {}",
                e
            )))
        })?;

        encode_frame(&payload).map_err(|e| Error::Protocol(e))
    }

    /// Create a minimal error response when normal encoding fails.
    fn encode_minimal_error(&self, message: &str) -> Vec<u8> {
        let response = Response::error(0, ormdb_proto::error_codes::INTERNAL, message);

        // Try to encode, fall back to empty on failure
        match rkyv::to_bytes::<rkyv::rancor::Error>(&response) {
            Ok(payload) => match encode_frame(&payload) {
                Ok(framed) => framed,
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }
}


/// Create a transport that listens on the configured addresses.
pub fn create_transport(
    config: &ServerConfig,
    handler: Arc<RequestHandler>,
) -> Result<Transport, Error> {
    if !config.has_transport() {
        return Err(Error::Config(
            "no transport configured (need TCP or IPC address)".to_string(),
        ));
    }

    Transport::new(config, handler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use ormdb_core::catalog::{EntityDef, FieldDef, FieldType, ScalarType, SchemaBundle};
    use ormdb_proto::framing::MAX_MESSAGE_SIZE;

    fn setup_test_components() -> (tempfile::TempDir, Arc<RequestHandler>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        // Create schema
        let schema = SchemaBundle::new(1).with_entity(
            EntityDef::new("User", "id")
                .with_field(FieldDef::new("id", FieldType::Scalar(ScalarType::Uuid)))
                .with_field(FieldDef::new("name", FieldType::Scalar(ScalarType::String))),
        );
        db.catalog().apply_schema(schema).unwrap();

        let handler = Arc::new(RequestHandler::new(Arc::new(db)));
        (dir, handler)
    }

    #[test]
    fn test_transport_creation() {
        let (dir, handler) = setup_test_components();

        let ipc_path = format!("ipc://{}", dir.path().join("ormdb.sock").display());
        let config = ServerConfig::new(dir.path())
            .without_tcp()
            .with_ipc_address(ipc_path)
            .with_max_message_size(MAX_MESSAGE_SIZE);

        let transport = Transport::new(&config, handler);
        match transport {
            Ok(_) => {}
            Err(Error::Transport(msg)) if msg.contains("Permission denied") => {
                return;
            }
            Err(err) => panic!("transport creation failed: {err}"),
        }
    }

    #[test]
    fn test_transport_requires_address() {
        let (_dir, handler) = setup_test_components();

        let config = ServerConfig::new("/tmp/test").without_tcp();

        let result = create_transport(&config, handler);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_ping_message() {
        let (_dir, handler) = setup_test_components();
        let worker = TransportWorker::new(handler, MAX_MESSAGE_SIZE);

        // Create a ping request
        let request = Request::ping(42);
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
        let framed = encode_frame(&payload).unwrap();

        // Process it
        let (response_bytes, is_success) = worker.process_message_with_status(&framed);
        assert!(is_success);

        // Decode response - copy to aligned buffer for rkyv
        let response_payload = ormdb_proto::framing::extract_payload(&response_bytes).unwrap();
        let mut aligned: rkyv::util::AlignedVec<16> = rkyv::util::AlignedVec::new();
        aligned.extend_from_slice(response_payload);
        let response: Response =
            rkyv::from_bytes::<Response, rkyv::rancor::Error>(&aligned).unwrap();

        assert_eq!(response.id, 42);
        assert!(response.status.is_ok());
        assert!(matches!(
            response.payload,
            ormdb_proto::ResponsePayload::Pong
        ));
    }

    #[test]
    fn test_process_invalid_message() {
        let (_dir, handler) = setup_test_components();
        let worker = TransportWorker::new(handler, MAX_MESSAGE_SIZE);

        // Send garbage data
        let (response_bytes, is_success) = worker.process_message_with_status(b"invalid data");

        // Should return an error response
        assert!(!response_bytes.is_empty());
        assert!(!is_success);
    }

    #[test]
    fn test_process_messages_concurrently() {
        let (_dir, handler) = setup_test_components();

        let mut handles = Vec::new();
        for i in 0..8 {
            let handler = handler.clone();
            handles.push(std::thread::spawn(move || {
                let worker = TransportWorker::new(handler, MAX_MESSAGE_SIZE);
                let request_id = 100 + i as u64;
                let request = Request::ping(request_id);
                let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
                let framed = encode_frame(&payload).unwrap();

                let (response_bytes, is_success) = worker.process_message_with_status(&framed);
                assert!(is_success);

                let response_payload = ormdb_proto::framing::extract_payload(&response_bytes).unwrap();
                let mut aligned: rkyv::util::AlignedVec<16> = rkyv::util::AlignedVec::new();
                aligned.extend_from_slice(response_payload);
                let response: Response =
                    rkyv::from_bytes::<Response, rkyv::rancor::Error>(&aligned).unwrap();

                assert_eq!(response.id, request_id);
                assert!(response.status.is_ok());
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
