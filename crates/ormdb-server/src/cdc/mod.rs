//! Change Data Capture (CDC) processing.
//!
//! This module provides a background task that processes CDC events from the
//! changelog and publishes them to the PubSubManager for subscriber delivery.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info};

use ormdb_proto::replication::ChangeLogEntry;

use crate::pubsub::PubSubManager;

/// CDC event processor that bridges the changelog to the PubSubManager.
///
/// This processor receives change log entries from a channel and publishes
/// them as events to matching subscribers via the PubSubManager.
pub struct CDCProcessor {
    /// Receiver for changelog entries.
    rx: mpsc::Receiver<ChangeLogEntry>,
    /// Pub-sub manager for event distribution.
    pubsub: Arc<PubSubManager>,
}

impl CDCProcessor {
    /// Create a new CDC processor.
    pub fn new(rx: mpsc::Receiver<ChangeLogEntry>, pubsub: Arc<PubSubManager>) -> Self {
        Self { rx, pubsub }
    }

    /// Run the CDC processor as a background task.
    ///
    /// This will process events until the channel is closed.
    pub async fn run(mut self) {
        info!("CDC processor started");

        while let Some(entry) = self.rx.recv().await {
            self.process_entry(&entry).await;
        }

        info!("CDC processor stopped (channel closed)");
    }

    /// Process a single changelog entry.
    async fn process_entry(&self, entry: &ChangeLogEntry) {
        debug!(
            lsn = entry.lsn,
            entity = %entry.entity_type,
            change_type = ?entry.change_type,
            "processing CDC entry"
        );

        self.pubsub
            .publish_event(
                &entry.entity_type,
                entry.entity_id,
                entry.change_type,
                entry.changed_fields.clone(),
                entry.schema_version,
            )
            .await;
    }
}

/// CDC channel sender for submitting changelog entries.
pub type CDCSender = mpsc::Sender<ChangeLogEntry>;

/// CDC channel receiver for processing changelog entries.
pub type CDCReceiver = mpsc::Receiver<ChangeLogEntry>;

/// Create a new CDC channel with the given buffer size.
pub fn channel(buffer_size: usize) -> (CDCSender, CDCReceiver) {
    mpsc::channel(buffer_size)
}

/// Handle for a running CDC processor task.
pub struct CDCHandle {
    tx: CDCSender,
}

impl CDCHandle {
    /// Create a new CDC handle.
    pub fn new(tx: CDCSender) -> Self {
        Self { tx }
    }

    /// Submit a changelog entry for processing.
    ///
    /// This is non-blocking and will return an error if the channel is full.
    pub fn try_send(&self, entry: ChangeLogEntry) -> Result<(), mpsc::error::TrySendError<ChangeLogEntry>> {
        self.tx.try_send(entry)
    }

    /// Submit a changelog entry, waiting if the channel is full.
    pub async fn send(&self, entry: ChangeLogEntry) -> Result<(), mpsc::error::SendError<ChangeLogEntry>> {
        self.tx.send(entry).await
    }

    /// Get a reference to the sender.
    pub fn sender(&self) -> &CDCSender {
        &self.tx
    }

    /// Clone the sender for use in another context.
    pub fn clone_sender(&self) -> CDCSender {
        self.tx.clone()
    }
}

impl Clone for CDCHandle {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

/// Start a CDC processor and return a handle for sending events.
///
/// This spawns a background task that processes CDC events.
pub fn start_processor(pubsub: Arc<PubSubManager>, buffer_size: usize) -> CDCHandle {
    let (tx, rx) = channel(buffer_size);
    let processor = CDCProcessor::new(rx, pubsub);

    tokio::spawn(async move {
        processor.run().await;
    });

    CDCHandle::new(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ormdb_proto::ChangeType;

    fn create_test_entry(entity: &str, id: [u8; 16], lsn: u64) -> ChangeLogEntry {
        ChangeLogEntry {
            lsn,
            timestamp: lsn * 1000,
            entity_type: entity.to_string(),
            entity_id: id,
            change_type: ChangeType::Insert,
            changed_fields: vec!["name".to_string()],
            before_data: None,
            after_data: Some(vec![1, 2, 3, 4]),
            schema_version: 1,
        }
    }

    #[tokio::test]
    async fn test_cdc_processor_processes_entries() {
        let pubsub = Arc::new(PubSubManager::new());
        let (tx, rx) = channel(10);

        // Spawn processor
        let processor = CDCProcessor::new(rx, pubsub.clone());
        let handle = tokio::spawn(async move {
            processor.run().await;
        });

        // Send an entry
        let entry = create_test_entry("User", [1u8; 16], 1);
        tx.send(entry).await.unwrap();

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Close the channel
        drop(tx);

        // Wait for processor to finish
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cdc_handle_try_send() {
        let pubsub = Arc::new(PubSubManager::new());
        let handle = start_processor(pubsub, 10);

        let entry = create_test_entry("User", [1u8; 16], 1);
        assert!(handle.try_send(entry).is_ok());
    }

    #[tokio::test]
    async fn test_cdc_handle_clone() {
        let pubsub = Arc::new(PubSubManager::new());
        let handle1 = start_processor(pubsub, 10);
        let handle2 = handle1.clone();

        let entry1 = create_test_entry("User", [1u8; 16], 1);
        let entry2 = create_test_entry("Post", [2u8; 16], 2);

        assert!(handle1.try_send(entry1).is_ok());
        assert!(handle2.try_send(entry2).is_ok());
    }
}
