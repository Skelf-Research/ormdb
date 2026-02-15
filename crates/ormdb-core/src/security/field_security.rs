//! Field-level security and masking.
//!
//! This module provides field-level access control and data masking
//! for sensitive fields.

use super::capability::SensitiveLevel;
use super::context::SecurityContext;
use ormdb_proto::Value;
use rkyv::{Archive, Deserialize, Serialize};

/// Sensitivity classification for a field.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum FieldSensitivity {
    /// Publicly accessible to any authenticated user.
    Public,
    /// Internal use only (requires authenticated context).
    Internal,
    /// Sensitive data (PII, requires SensitiveFieldAccess capability).
    Sensitive,
    /// Restricted (requires specific named capability).
    Restricted {
        /// The capability required to access this field.
        required_capability: String,
    },
}

impl Default for FieldSensitivity {
    fn default() -> Self {
        FieldSensitivity::Public
    }
}

impl FieldSensitivity {
    /// Convert to a SensitiveLevel for capability checking.
    pub fn to_sensitive_level(&self) -> Option<SensitiveLevel> {
        match self {
            FieldSensitivity::Public => None,
            FieldSensitivity::Internal => Some(SensitiveLevel::Internal),
            FieldSensitivity::Sensitive => Some(SensitiveLevel::Sensitive),
            FieldSensitivity::Restricted { .. } => Some(SensitiveLevel::Restricted),
        }
    }
}

/// Masking strategy for sensitive fields when access is denied.
#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum MaskingStrategy {
    /// Completely omit the field from results.
    Omit,
    /// Replace with null.
    Null,
    /// Replace with a fixed placeholder string.
    Redacted(String),
    /// Partial masking (show some characters).
    Partial {
        /// Number of characters to show.
        visible_chars: u32,
        /// Show characters from end (true) or beginning (false).
        from_end: bool,
        /// Character to use for masking.
        mask_char: char,
    },
    /// Hash the value (for audit/lookup without revealing).
    Hash,
}

impl Default for MaskingStrategy {
    fn default() -> Self {
        MaskingStrategy::Null
    }
}

/// Field security configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct FieldSecurity {
    /// Sensitivity level of the field.
    pub sensitivity: FieldSensitivity,
    /// Masking strategy when access is denied.
    pub masking: MaskingStrategy,
}

impl FieldSecurity {
    /// Create a public field (no restrictions).
    pub fn public() -> Self {
        Self {
            sensitivity: FieldSensitivity::Public,
            masking: MaskingStrategy::Null,
        }
    }

    /// Create an internal field.
    pub fn internal() -> Self {
        Self {
            sensitivity: FieldSensitivity::Internal,
            masking: MaskingStrategy::Null,
        }
    }

    /// Create a sensitive field with the given masking strategy.
    pub fn sensitive(masking: MaskingStrategy) -> Self {
        Self {
            sensitivity: FieldSensitivity::Sensitive,
            masking,
        }
    }

    /// Create a restricted field requiring a specific capability.
    pub fn restricted(capability: impl Into<String>, masking: MaskingStrategy) -> Self {
        Self {
            sensitivity: FieldSensitivity::Restricted {
                required_capability: capability.into(),
            },
            masking,
        }
    }

    /// Set the masking strategy.
    pub fn with_masking(mut self, masking: MaskingStrategy) -> Self {
        self.masking = masking;
        self
    }
}

/// Result of field processing.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldResult {
    /// Field is accessible, return the original value.
    Accessible(Value),
    /// Field should be masked.
    Masked(Value),
    /// Field should be completely omitted.
    Omit,
}

/// Field masker that applies security during result assembly.
pub struct FieldMasker;

impl FieldMasker {
    /// Check if a field is accessible in the given security context.
    pub fn is_accessible(security: &FieldSecurity, context: &SecurityContext) -> bool {
        // Admin can access everything
        if context.is_admin() {
            return true;
        }

        match &security.sensitivity {
            FieldSensitivity::Public => true,
            FieldSensitivity::Internal => context.is_authenticated(),
            FieldSensitivity::Sensitive => {
                context.can_access_sensitive(SensitiveLevel::Sensitive)
            }
            FieldSensitivity::Restricted { required_capability } => {
                context.capabilities.has_custom(required_capability)
                    || context.can_access_sensitive(SensitiveLevel::Restricted)
            }
        }
    }

    /// Mask a value according to the masking strategy.
    pub fn mask(value: &Value, strategy: &MaskingStrategy) -> Value {
        match strategy {
            MaskingStrategy::Omit => Value::Null, // Caller should handle omission
            MaskingStrategy::Null => Value::Null,
            MaskingStrategy::Redacted(placeholder) => Value::String(placeholder.clone()),
            MaskingStrategy::Partial {
                visible_chars,
                from_end,
                mask_char,
            } => Self::partial_mask(value, *visible_chars as usize, *from_end, *mask_char),
            MaskingStrategy::Hash => Self::hash_value(value),
        }
    }

    /// Process a field value - return accessible value, masked version, or omit indicator.
    pub fn process_field(
        value: &Value,
        security: &Option<FieldSecurity>,
        context: &SecurityContext,
    ) -> FieldResult {
        // No security config means public access
        let security = match security {
            Some(s) => s,
            None => return FieldResult::Accessible(value.clone()),
        };

        // Check accessibility
        if Self::is_accessible(security, context) {
            return FieldResult::Accessible(value.clone());
        }

        // Apply masking
        match &security.masking {
            MaskingStrategy::Omit => FieldResult::Omit,
            strategy => FieldResult::Masked(Self::mask(value, strategy)),
        }
    }

    /// Apply partial masking to a value.
    fn partial_mask(value: &Value, visible_chars: usize, from_end: bool, mask_char: char) -> Value {
        match value {
            Value::String(s) => {
                if s.len() <= visible_chars {
                    // String is too short, mask entirely
                    Value::String(mask_char.to_string().repeat(s.len()))
                } else if from_end {
                    // Show last N characters
                    let masked_len = s.len() - visible_chars;
                    let masked = mask_char.to_string().repeat(masked_len);
                    let visible = &s[masked_len..];
                    Value::String(format!("{}{}", masked, visible))
                } else {
                    // Show first N characters
                    let visible = &s[..visible_chars];
                    let masked = mask_char.to_string().repeat(s.len() - visible_chars);
                    Value::String(format!("{}{}", visible, masked))
                }
            }
            // Non-string values get nulled
            _ => Value::Null,
        }
    }

    /// Hash a value using blake3 cryptographic hash for audit purposes.
    ///
    /// Blake3 is a cryptographically secure hash function that is:
    /// - Collision resistant (unlike djb2)
    /// - Fast (faster than SHA-256)
    /// - Suitable for security-sensitive applications
    fn hash_value(value: &Value) -> Value {
        match value {
            Value::String(s) => {
                let hash = blake3::hash(s.as_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Int32(i) => {
                let hash = blake3::hash(&i.to_le_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Int64(i) => {
                let hash = blake3::hash(&i.to_le_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Bytes(b) => {
                let hash = blake3::hash(b);
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Float32(f) => {
                let hash = blake3::hash(&f.to_le_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Float64(f) => {
                let hash = blake3::hash(&f.to_le_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Bool(b) => {
                let hash = blake3::hash(&[*b as u8]);
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Uuid(u) => {
                let hash = blake3::hash(u);
                Value::String(format!("hash:{}", hash.to_hex()))
            }
            Value::Null => Value::String("hash:null".to_string()),
            // For other types (Decimal, Timestamp, Date, Time, etc.), convert to string first
            other => {
                let s = format!("{:?}", other);
                let hash = blake3::hash(s.as_bytes());
                Value::String(format!("hash:{}", hash.to_hex()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::capability::{Capability, CapabilitySet, EntityScope};

    fn context_with_sensitive() -> SecurityContext {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Read(EntityScope::All));
        caps.add(Capability::SensitiveFieldAccess(SensitiveLevel::Sensitive));
        SecurityContext::new("conn", "client", caps)
    }

    fn context_authenticated() -> SecurityContext {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Read(EntityScope::All));
        SecurityContext::new("conn", "client", caps)
    }

    #[test]
    fn test_field_accessibility_public() {
        let security = FieldSecurity::public();
        let anon = SecurityContext::anonymous();
        assert!(FieldMasker::is_accessible(&security, &anon));
    }

    #[test]
    fn test_field_accessibility_internal() {
        let security = FieldSecurity::internal();

        let anon = SecurityContext::anonymous();
        assert!(!FieldMasker::is_accessible(&security, &anon));

        let auth = context_authenticated();
        assert!(FieldMasker::is_accessible(&security, &auth));
    }

    #[test]
    fn test_field_accessibility_sensitive() {
        let security = FieldSecurity::sensitive(MaskingStrategy::Null);

        let auth = context_authenticated();
        assert!(!FieldMasker::is_accessible(&security, &auth));

        let sensitive = context_with_sensitive();
        assert!(FieldMasker::is_accessible(&security, &sensitive));
    }

    #[test]
    fn test_field_accessibility_restricted() {
        let security = FieldSecurity::restricted("view_ssn", MaskingStrategy::Null);

        let sensitive = context_with_sensitive();
        assert!(!FieldMasker::is_accessible(&security, &sensitive));

        let mut caps = CapabilitySet::new();
        caps.add(Capability::Custom("view_ssn".to_string()));
        let custom = SecurityContext::new("conn", "client", caps);
        assert!(FieldMasker::is_accessible(&security, &custom));
    }

    #[test]
    fn test_admin_bypasses_all() {
        let security = FieldSecurity::restricted("super_secret", MaskingStrategy::Omit);
        let admin = SecurityContext::admin("conn");
        assert!(FieldMasker::is_accessible(&security, &admin));
    }

    #[test]
    fn test_masking_null() {
        let value = Value::String("secret".to_string());
        let masked = FieldMasker::mask(&value, &MaskingStrategy::Null);
        assert_eq!(masked, Value::Null);
    }

    #[test]
    fn test_masking_redacted() {
        let value = Value::String("secret".to_string());
        let masked = FieldMasker::mask(&value, &MaskingStrategy::Redacted("[REDACTED]".to_string()));
        assert_eq!(masked, Value::String("[REDACTED]".to_string()));
    }

    #[test]
    fn test_masking_partial_from_end() {
        let value = Value::String("1234567890".to_string());
        let masked = FieldMasker::mask(
            &value,
            &MaskingStrategy::Partial {
                visible_chars: 4,
                from_end: true,
                mask_char: '*',
            },
        );
        assert_eq!(masked, Value::String("******7890".to_string()));
    }

    #[test]
    fn test_masking_partial_from_start() {
        let value = Value::String("1234567890".to_string());
        let masked = FieldMasker::mask(
            &value,
            &MaskingStrategy::Partial {
                visible_chars: 4,
                from_end: false,
                mask_char: '*',
            },
        );
        assert_eq!(masked, Value::String("1234******".to_string()));
    }

    #[test]
    fn test_masking_hash() {
        let value = Value::String("secret".to_string());
        let masked = FieldMasker::mask(&value, &MaskingStrategy::Hash);
        match masked {
            Value::String(s) => assert!(s.starts_with("hash:")),
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_process_field_accessible() {
        let security = FieldSecurity::public();
        let value = Value::String("hello".to_string());
        let ctx = SecurityContext::anonymous();

        let result = FieldMasker::process_field(&value, &Some(security), &ctx);
        assert_eq!(result, FieldResult::Accessible(value));
    }

    #[test]
    fn test_process_field_masked() {
        let security = FieldSecurity::sensitive(MaskingStrategy::Redacted("[HIDDEN]".to_string()));
        let value = Value::String("secret".to_string());
        let ctx = context_authenticated(); // No sensitive access

        let result = FieldMasker::process_field(&value, &Some(security), &ctx);
        assert_eq!(
            result,
            FieldResult::Masked(Value::String("[HIDDEN]".to_string()))
        );
    }

    #[test]
    fn test_process_field_omit() {
        let security = FieldSecurity::sensitive(MaskingStrategy::Omit);
        let value = Value::String("secret".to_string());
        let ctx = context_authenticated();

        let result = FieldMasker::process_field(&value, &Some(security), &ctx);
        assert_eq!(result, FieldResult::Omit);
    }

    #[test]
    fn test_process_field_no_security() {
        let value = Value::String("hello".to_string());
        let ctx = SecurityContext::anonymous();

        let result = FieldMasker::process_field(&value, &None, &ctx);
        assert_eq!(result, FieldResult::Accessible(value));
    }
}
