//! Subscription tracking and filtering.

use std::time::Instant;

use ormdb_proto::query::Filter;

/// A filter for subscriptions.
#[derive(Debug, Clone)]
pub struct SubscriptionFilter {
    /// Optional filter expression.
    pub filter: Option<Filter>,
    /// Fields to include in change events.
    pub fields: Option<Vec<String>>,
    /// Whether to include related entity changes.
    pub include_relations: bool,
}

impl SubscriptionFilter {
    /// Create a new subscription filter.
    pub fn new() -> Self {
        Self {
            filter: None,
            fields: None,
            include_relations: false,
        }
    }

    /// Create a filter from a protocol subscription.
    pub fn from_subscription(sub: &ormdb_proto::Subscription) -> Self {
        Self {
            filter: sub.filter.clone(),
            fields: sub.fields.clone(),
            include_relations: sub.include_relations,
        }
    }
}

impl Default for SubscriptionFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// A subscription entry tracking an active subscription.
#[derive(Debug, Clone)]
pub struct SubscriptionEntry {
    /// Unique subscription ID.
    pub id: u64,
    /// Client identifier.
    pub client_id: String,
    /// Entity type being watched.
    pub entity: String,
    /// Filter for this subscription.
    pub filter: SubscriptionFilter,
    /// When the subscription was created.
    pub created_at: Instant,
    /// Number of events sent to this subscription.
    pub events_sent: u64,
}

impl SubscriptionEntry {
    /// Create a new subscription entry.
    pub fn new(
        id: u64,
        client_id: impl Into<String>,
        entity: impl Into<String>,
        filter: SubscriptionFilter,
    ) -> Self {
        Self {
            id,
            client_id: client_id.into(),
            entity: entity.into(),
            filter,
            created_at: Instant::now(),
            events_sent: 0,
        }
    }

    /// Get the age of this subscription.
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// Increment the events sent counter.
    pub fn record_event(&mut self) {
        self.events_sent += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_entry() {
        let filter = SubscriptionFilter::new();
        let entry = SubscriptionEntry::new(1, "client-123", "User", filter);

        assert_eq!(entry.id, 1);
        assert_eq!(entry.client_id, "client-123");
        assert_eq!(entry.entity, "User");
        assert_eq!(entry.events_sent, 0);
    }

    #[test]
    fn test_subscription_filter() {
        let filter = SubscriptionFilter {
            filter: None,
            fields: Some(vec!["id".to_string(), "name".to_string()]),
            include_relations: true,
        };

        assert!(filter.fields.is_some());
        assert!(filter.include_relations);
    }
}
