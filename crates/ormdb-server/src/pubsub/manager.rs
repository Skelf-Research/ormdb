//! Pub-sub manager for handling subscriptions and publishing events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use ormdb_proto::{ChangeEvent, ChangeType, Subscription};

use super::subscription::{SubscriptionEntry, SubscriptionFilter};
use crate::error::Error;

/// Manager for pub-sub subscriptions and event publishing.
///
/// This provides the infrastructure for change notifications. Full change
/// data capture (CDC) integration with the storage layer will be implemented
/// in Phase 6.
pub struct PubSubManager {
    /// Active subscriptions keyed by subscription ID.
    subscriptions: RwLock<HashMap<u64, SubscriptionEntry>>,
    /// Index of subscriptions by entity type.
    entity_index: RwLock<HashMap<String, Vec<u64>>>,
    /// Next subscription ID.
    next_subscription_id: AtomicU64,
    /// Queue for outgoing events (will be connected to NNG PUB socket later).
    event_queue: RwLock<Vec<ChangeEvent>>,
}

impl PubSubManager {
    /// Create a new pub-sub manager.
    pub fn new() -> Self {
        Self {
            subscriptions: RwLock::new(HashMap::new()),
            entity_index: RwLock::new(HashMap::new()),
            next_subscription_id: AtomicU64::new(1),
            event_queue: RwLock::new(Vec::new()),
        }
    }

    /// Subscribe to changes for an entity type.
    ///
    /// Returns the subscription ID.
    pub async fn subscribe(
        &self,
        client_id: &str,
        subscription: &Subscription,
    ) -> Result<u64, Error> {
        let subscription_id = self.next_subscription_id.fetch_add(1, Ordering::SeqCst);

        let filter = SubscriptionFilter::from_subscription(subscription);
        let entry = SubscriptionEntry::new(
            subscription_id,
            client_id,
            &subscription.entity,
            filter,
        );

        // Add to subscriptions map
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(subscription_id, entry);
        }

        // Add to entity index
        {
            let mut index = self.entity_index.write().await;
            index
                .entry(subscription.entity.clone())
                .or_default()
                .push(subscription_id);
        }

        tracing::debug!(
            subscription_id,
            client_id,
            entity = %subscription.entity,
            "subscription created"
        );

        Ok(subscription_id)
    }

    /// Unsubscribe from a subscription.
    pub async fn unsubscribe(&self, subscription_id: u64) -> Result<(), Error> {
        // Remove from subscriptions map
        let entry = {
            let mut subs = self.subscriptions.write().await;
            subs.remove(&subscription_id)
        };

        let entry = match entry {
            Some(e) => e,
            None => {
                return Err(Error::Database(format!(
                    "subscription {} not found",
                    subscription_id
                )));
            }
        };

        // Remove from entity index
        {
            let mut index = self.entity_index.write().await;
            if let Some(ids) = index.get_mut(&entry.entity) {
                ids.retain(|&id| id != subscription_id);
                if ids.is_empty() {
                    index.remove(&entry.entity);
                }
            }
        }

        tracing::debug!(
            subscription_id,
            client_id = %entry.client_id,
            entity = %entry.entity,
            events_sent = entry.events_sent,
            "subscription removed"
        );

        Ok(())
    }

    /// Publish a change event for an entity.
    ///
    /// This queues the event for delivery to matching subscriptions.
    /// In Phase 6, this will be connected to the storage layer's CDC.
    pub async fn publish_event(
        &self,
        entity: &str,
        entity_id: [u8; 16],
        change_type: ChangeType,
        changed_fields: Vec<String>,
        schema_version: u64,
    ) {
        // Find matching subscriptions
        let subscription_ids = {
            let index = self.entity_index.read().await;
            match index.get(entity) {
                Some(ids) => ids.clone(),
                None => return, // No subscriptions for this entity
            }
        };

        if subscription_ids.is_empty() {
            return;
        }

        // Create events for each matching subscription
        let mut events = Vec::new();
        for subscription_id in subscription_ids {
            let event = ChangeEvent {
                subscription_id,
                change_type,
                entity: entity.to_string(),
                entity_id,
                changed_fields: changed_fields.clone(),
                schema_version,
            };
            events.push(event);
        }

        // Queue events
        {
            let mut queue = self.event_queue.write().await;
            queue.extend(events);
        }

        tracing::trace!(
            entity,
            change_type = ?change_type,
            "published change event"
        );
    }

    /// Drain queued events for delivery.
    ///
    /// This is used by the transport layer to send events to subscribers.
    pub async fn drain_events(&self) -> Vec<ChangeEvent> {
        let mut queue = self.event_queue.write().await;
        std::mem::take(&mut *queue)
    }

    /// Get the number of active subscriptions.
    pub async fn subscription_count(&self) -> usize {
        self.subscriptions.read().await.len()
    }

    /// Get subscriptions for a specific entity.
    pub async fn subscriptions_for_entity(&self, entity: &str) -> Vec<u64> {
        let index = self.entity_index.read().await;
        index.get(entity).cloned().unwrap_or_default()
    }

    /// Get a subscription by ID.
    pub async fn get_subscription(&self, subscription_id: u64) -> Option<SubscriptionEntry> {
        let subs = self.subscriptions.read().await;
        subs.get(&subscription_id).cloned()
    }

    /// Remove all subscriptions for a client.
    pub async fn remove_client_subscriptions(&self, client_id: &str) {
        let to_remove: Vec<u64> = {
            let subs = self.subscriptions.read().await;
            subs.iter()
                .filter(|(_, entry)| entry.client_id == client_id)
                .map(|(&id, _)| id)
                .collect()
        };

        for subscription_id in to_remove {
            let _ = self.unsubscribe(subscription_id).await;
        }
    }
}

impl Default for PubSubManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared pub-sub manager handle.
pub type SharedPubSubManager = Arc<PubSubManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let manager = PubSubManager::new();

        let sub = Subscription::new("User");
        let id = manager.subscribe("client-1", &sub).await.unwrap();

        assert_eq!(manager.subscription_count().await, 1);
        assert_eq!(manager.subscriptions_for_entity("User").await, vec![id]);

        manager.unsubscribe(id).await.unwrap();

        assert_eq!(manager.subscription_count().await, 0);
        assert!(manager.subscriptions_for_entity("User").await.is_empty());
    }

    #[tokio::test]
    async fn test_publish_event() {
        let manager = PubSubManager::new();

        // Subscribe to User changes
        let sub = Subscription::new("User");
        let id = manager.subscribe("client-1", &sub).await.unwrap();

        // Publish an event
        manager
            .publish_event("User", [1u8; 16], ChangeType::Insert, vec!["name".to_string()], 1)
            .await;

        // Drain events
        let events = manager.drain_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subscription_id, id);
        assert_eq!(events[0].change_type, ChangeType::Insert);
        assert_eq!(events[0].entity, "User");

        // Queue should be empty now
        assert!(manager.drain_events().await.is_empty());
    }

    #[tokio::test]
    async fn test_no_event_without_subscription() {
        let manager = PubSubManager::new();

        // Publish an event without any subscriptions
        manager
            .publish_event("User", [1u8; 16], ChangeType::Insert, vec![], 1)
            .await;

        // No events should be queued
        assert!(manager.drain_events().await.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_subscriptions() {
        let manager = PubSubManager::new();

        // Two clients subscribe to User
        let sub = Subscription::new("User");
        let id1 = manager.subscribe("client-1", &sub).await.unwrap();
        let id2 = manager.subscribe("client-2", &sub).await.unwrap();

        assert_eq!(manager.subscription_count().await, 2);

        // Publish an event
        manager
            .publish_event("User", [1u8; 16], ChangeType::Update, vec![], 1)
            .await;

        // Both subscriptions should receive the event
        let events = manager.drain_events().await;
        assert_eq!(events.len(), 2);

        let ids: Vec<u64> = events.iter().map(|e| e.subscription_id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_remove_client_subscriptions() {
        let manager = PubSubManager::new();

        // Client 1 has two subscriptions
        manager.subscribe("client-1", &Subscription::new("User")).await.unwrap();
        manager.subscribe("client-1", &Subscription::new("Post")).await.unwrap();

        // Client 2 has one subscription
        let id3 = manager.subscribe("client-2", &Subscription::new("User")).await.unwrap();

        assert_eq!(manager.subscription_count().await, 3);

        // Remove client 1's subscriptions
        manager.remove_client_subscriptions("client-1").await;

        assert_eq!(manager.subscription_count().await, 1);
        assert!(manager.get_subscription(id3).await.is_some());
    }
}
