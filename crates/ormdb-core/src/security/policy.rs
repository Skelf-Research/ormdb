//! Security policy storage.
//!
//! Persists RLS policies using sled.

use super::error::{SecurityError, SecurityResult};
use super::rls::RlsPolicy;

const POLICY_TREE_NAME: &[u8] = b"security:policies";
const RLS_PREFIX: &[u8] = b"rls:";

/// Policy store for persisting security configurations.
pub struct PolicyStore {
    tree: sled::Tree,
}

impl PolicyStore {
    /// Open the policy store.
    pub fn open(db: &sled::Db) -> SecurityResult<Self> {
        let tree = db
            .open_tree(POLICY_TREE_NAME)
            .map_err(|e| SecurityError::Storage(e.into()))?;
        Ok(Self { tree })
    }

    /// Get all RLS policies for an entity.
    pub fn get_rls_policies(&self, entity: &str) -> SecurityResult<Vec<RlsPolicy>> {
        let mut policies = Vec::new();
        let prefix = Self::rls_key_prefix(entity);

        for result in self.tree.scan_prefix(&prefix) {
            let (_, value) = result.map_err(|e| SecurityError::Storage(e.into()))?;
            let policy = Self::deserialize_policy(&value)?;
            policies.push(policy);
        }

        Ok(policies)
    }

    /// Get all RLS policies.
    pub fn get_all_rls_policies(&self) -> SecurityResult<Vec<RlsPolicy>> {
        let mut policies = Vec::new();

        for result in self.tree.scan_prefix(RLS_PREFIX) {
            let (_, value) = result.map_err(|e| SecurityError::Storage(e.into()))?;
            let policy = Self::deserialize_policy(&value)?;
            policies.push(policy);
        }

        Ok(policies)
    }

    /// Get a specific RLS policy by name.
    pub fn get_rls_policy(&self, name: &str) -> SecurityResult<Option<RlsPolicy>> {
        // Scan all policies to find by name (not optimal, but policies are few)
        for result in self.tree.scan_prefix(RLS_PREFIX) {
            let (_, value) = result.map_err(|e| SecurityError::Storage(e.into()))?;
            let policy = Self::deserialize_policy(&value)?;
            if policy.name == name {
                return Ok(Some(policy));
            }
        }
        Ok(None)
    }

    /// Save an RLS policy.
    pub fn put_rls_policy(&self, policy: &RlsPolicy) -> SecurityResult<()> {
        let key = Self::rls_key(&policy.entity, &policy.name);
        let value = Self::serialize_policy(policy)?;
        self.tree
            .insert(key, value)
            .map_err(|e| SecurityError::Storage(e.into()))?;
        Ok(())
    }

    /// Remove an RLS policy.
    pub fn remove_rls_policy(&self, entity: &str, name: &str) -> SecurityResult<bool> {
        let key = Self::rls_key(entity, name);
        let removed = self
            .tree
            .remove(key)
            .map_err(|e| SecurityError::Storage(e.into()))?;
        Ok(removed.is_some())
    }

    /// List all policy names.
    pub fn list_policy_names(&self) -> SecurityResult<Vec<String>> {
        let mut names = Vec::new();

        for result in self.tree.scan_prefix(RLS_PREFIX) {
            let (_, value) = result.map_err(|e| SecurityError::Storage(e.into()))?;
            let policy = Self::deserialize_policy(&value)?;
            names.push(policy.name);
        }

        Ok(names)
    }

    /// Clear all policies.
    pub fn clear(&self) -> SecurityResult<()> {
        self.tree
            .clear()
            .map_err(|e| SecurityError::Storage(e.into()))?;
        Ok(())
    }

    fn rls_key_prefix(entity: &str) -> Vec<u8> {
        let mut key = RLS_PREFIX.to_vec();
        key.extend_from_slice(entity.as_bytes());
        key.push(b':');
        key
    }

    fn rls_key(entity: &str, name: &str) -> Vec<u8> {
        let mut key = Self::rls_key_prefix(entity);
        key.extend_from_slice(name.as_bytes());
        key
    }

    fn serialize_policy(policy: &RlsPolicy) -> SecurityResult<Vec<u8>> {
        serde_json::to_vec(policy)
            .map_err(|e| SecurityError::AuditError(format!("serialization error: {}", e)))
    }

    fn deserialize_policy(bytes: &[u8]) -> SecurityResult<RlsPolicy> {
        serde_json::from_slice(bytes)
            .map_err(|e| SecurityError::AuditError(format!("deserialization error: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::rls::{PolicyType, RlsFilterExpr, RlsOperation};

    fn test_policy_store() -> (PolicyStore, sled::Db) {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let store = PolicyStore::open(&db).unwrap();
        (store, db)
    }

    #[test]
    fn test_store_and_retrieve_policy() {
        let (store, _db) = test_policy_store();

        let policy = RlsPolicy::new(
            "org_isolation",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        )
        .with_type(PolicyType::Permissive)
        .with_operations(vec![RlsOperation::Select]);

        store.put_rls_policy(&policy).unwrap();

        let retrieved = store.get_rls_policy("org_isolation").unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "org_isolation");
        assert_eq!(retrieved.entity, "Document");
        assert_eq!(retrieved.policy_type, PolicyType::Permissive);
    }

    #[test]
    fn test_get_policies_by_entity() {
        let (store, _db) = test_policy_store();

        let policy1 = RlsPolicy::new(
            "policy1",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );
        let policy2 = RlsPolicy::new(
            "policy2",
            "Document",
            RlsFilterExpr::attribute_eq("team_id", "user.team_id"),
        );
        let policy3 = RlsPolicy::new(
            "policy3",
            "User", // Different entity
            RlsFilterExpr::attribute_eq("id", "user.id"),
        );

        store.put_rls_policy(&policy1).unwrap();
        store.put_rls_policy(&policy2).unwrap();
        store.put_rls_policy(&policy3).unwrap();

        let doc_policies = store.get_rls_policies("Document").unwrap();
        assert_eq!(doc_policies.len(), 2);

        let user_policies = store.get_rls_policies("User").unwrap();
        assert_eq!(user_policies.len(), 1);
    }

    #[test]
    fn test_remove_policy() {
        let (store, _db) = test_policy_store();

        let policy = RlsPolicy::new(
            "test_policy",
            "Document",
            RlsFilterExpr::attribute_eq("org_id", "user.org_id"),
        );

        store.put_rls_policy(&policy).unwrap();
        assert!(store.get_rls_policy("test_policy").unwrap().is_some());

        let removed = store.remove_rls_policy("Document", "test_policy").unwrap();
        assert!(removed);

        assert!(store.get_rls_policy("test_policy").unwrap().is_none());
    }

    #[test]
    fn test_list_policy_names() {
        let (store, _db) = test_policy_store();

        let policy1 = RlsPolicy::new("alpha", "Doc", RlsFilterExpr::True);
        let policy2 = RlsPolicy::new("beta", "Doc", RlsFilterExpr::True);

        store.put_rls_policy(&policy1).unwrap();
        store.put_rls_policy(&policy2).unwrap();

        let names = store.list_policy_names().unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
    }

    #[test]
    fn test_clear_policies() {
        let (store, _db) = test_policy_store();

        let policy = RlsPolicy::new("test", "Doc", RlsFilterExpr::True);
        store.put_rls_policy(&policy).unwrap();

        assert!(!store.list_policy_names().unwrap().is_empty());

        store.clear().unwrap();

        assert!(store.list_policy_names().unwrap().is_empty());
    }
}
