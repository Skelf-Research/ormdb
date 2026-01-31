//! Migration state management and persistence.
//!
//! Tracks the progress of migrations for crash recovery and status reporting.

use super::error::MigrationError;
use crate::storage::key::current_timestamp;
use rkyv::{Archive, Deserialize, Serialize};

/// State of a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration created but not started.
    Pending,
    /// Expand phase in progress.
    Expanding,
    /// Backfill phase in progress.
    Backfilling,
    /// Contract phase in progress.
    Contracting,
    /// Migration completed successfully.
    Complete,
    /// Migration failed.
    Failed,
    /// Migration was rolled back.
    RolledBack,
}

impl std::fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationStatus::Pending => write!(f, "pending"),
            MigrationStatus::Expanding => write!(f, "expanding"),
            MigrationStatus::Backfilling => write!(f, "backfilling"),
            MigrationStatus::Contracting => write!(f, "contracting"),
            MigrationStatus::Complete => write!(f, "complete"),
            MigrationStatus::Failed => write!(f, "failed"),
            MigrationStatus::RolledBack => write!(f, "rolled_back"),
        }
    }
}

/// Status of a single step within a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
pub enum StepStatus {
    /// Step not yet started.
    Pending,
    /// Step in progress.
    InProgress,
    /// Step completed successfully.
    Complete,
    /// Step failed.
    Failed,
    /// Step was skipped.
    Skipped,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::InProgress => write!(f, "in_progress"),
            StepStatus::Complete => write!(f, "complete"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

/// Progress of a single step within a migration.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct StepProgress {
    /// The index of this step in the migration plan.
    pub step_index: usize,
    /// Current status of the step.
    pub status: StepStatus,
    /// When the step started (microseconds since epoch).
    pub started_at: Option<u64>,
    /// When the step completed (microseconds since epoch).
    pub completed_at: Option<u64>,
    /// Number of records/items processed.
    pub processed_count: u64,
    /// Total number of records/items to process (if known).
    pub total_count: Option<u64>,
    /// ID of the last processed entity (for resumption).
    pub last_processed_id: Option<[u8; 16]>,
    /// Error message if the step failed.
    pub error: Option<String>,
}

impl StepProgress {
    /// Create a new step progress entry.
    pub fn new(step_index: usize) -> Self {
        Self {
            step_index,
            status: StepStatus::Pending,
            started_at: None,
            completed_at: None,
            processed_count: 0,
            total_count: None,
            last_processed_id: None,
            error: None,
        }
    }

    /// Mark the step as started.
    pub fn start(&mut self) {
        self.status = StepStatus::InProgress;
        self.started_at = Some(current_timestamp());
    }

    /// Mark the step as completed.
    pub fn complete(&mut self) {
        self.status = StepStatus::Complete;
        self.completed_at = Some(current_timestamp());
    }

    /// Mark the step as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = StepStatus::Failed;
        self.completed_at = Some(current_timestamp());
        self.error = Some(error.into());
    }

    /// Mark the step as skipped.
    pub fn skip(&mut self) {
        self.status = StepStatus::Skipped;
        self.completed_at = Some(current_timestamp());
    }

    /// Update progress.
    pub fn update_progress(&mut self, processed: u64, total: Option<u64>, last_id: Option<[u8; 16]>) {
        self.processed_count = processed;
        self.total_count = total;
        self.last_processed_id = last_id;
    }

    /// Calculate percentage complete.
    pub fn percent_complete(&self) -> Option<f64> {
        self.total_count.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.processed_count as f64 / total as f64) * 100.0
            }
        })
    }
}

/// Persistent migration state.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct MigrationState {
    /// Unique migration ID.
    pub migration_id: [u8; 16],
    /// Source schema version.
    pub from_version: u64,
    /// Target schema version.
    pub to_version: u64,
    /// Current status of the migration.
    pub status: MigrationStatus,
    /// Index of the current step being executed.
    pub current_step_index: usize,
    /// When the migration started (microseconds since epoch).
    pub started_at: Option<u64>,
    /// When the migration completed (microseconds since epoch).
    pub completed_at: Option<u64>,
    /// Error message if the migration failed.
    pub error: Option<String>,
    /// Progress of each step.
    pub step_progress: Vec<StepProgress>,
}

impl MigrationState {
    /// Create a new migration state.
    pub fn new(migration_id: [u8; 16], from_version: u64, to_version: u64, step_count: usize) -> Self {
        let step_progress = (0..step_count).map(StepProgress::new).collect();

        Self {
            migration_id,
            from_version,
            to_version,
            status: MigrationStatus::Pending,
            current_step_index: 0,
            started_at: None,
            completed_at: None,
            error: None,
            step_progress,
        }
    }

    /// Start the migration.
    pub fn start(&mut self) {
        self.status = MigrationStatus::Expanding;
        self.started_at = Some(current_timestamp());
    }

    /// Transition to backfilling phase.
    pub fn start_backfilling(&mut self) {
        self.status = MigrationStatus::Backfilling;
    }

    /// Transition to contracting phase.
    pub fn start_contracting(&mut self) {
        self.status = MigrationStatus::Contracting;
    }

    /// Mark the migration as complete.
    pub fn complete(&mut self) {
        self.status = MigrationStatus::Complete;
        self.completed_at = Some(current_timestamp());
    }

    /// Mark the migration as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = MigrationStatus::Failed;
        self.completed_at = Some(current_timestamp());
        self.error = Some(error.into());
    }

    /// Mark the migration as rolled back.
    pub fn rollback(&mut self) {
        self.status = MigrationStatus::RolledBack;
        self.completed_at = Some(current_timestamp());
    }

    /// Advance to the next step.
    pub fn advance_step(&mut self) {
        self.current_step_index += 1;
    }

    /// Get the current step progress.
    pub fn current_step(&self) -> Option<&StepProgress> {
        self.step_progress.get(self.current_step_index)
    }

    /// Get the current step progress (mutable).
    pub fn current_step_mut(&mut self) -> Option<&mut StepProgress> {
        self.step_progress.get_mut(self.current_step_index)
    }

    /// Check if the migration is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            MigrationStatus::Complete | MigrationStatus::Failed | MigrationStatus::RolledBack
        )
    }

    /// Check if the migration can be resumed.
    pub fn can_resume(&self) -> bool {
        matches!(
            self.status,
            MigrationStatus::Expanding | MigrationStatus::Backfilling | MigrationStatus::Contracting
        )
    }

    /// Serialize the state to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MigrationError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(|e| MigrationError::Serialization(e.to_string()))
    }

    /// Deserialize state from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MigrationError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| MigrationError::Deserialization(e.to_string()))
    }
}

/// Backfill job state for crash recovery.
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct BackfillJobState {
    /// Unique job ID.
    pub job_id: [u8; 16],
    /// Associated migration ID.
    pub migration_id: [u8; 16],
    /// Entity type being backfilled.
    pub entity_name: String,
    /// Field being backfilled.
    pub field_name: String,
    /// Batch size for processing.
    pub batch_size: usize,
    /// ID of the last processed entity (for resumption).
    pub last_processed_id: Option<[u8; 16]>,
    /// Number of records processed so far.
    pub processed_count: u64,
    /// Total number of records to process (if known).
    pub total_count: Option<u64>,
    /// When the job started (microseconds since epoch).
    pub started_at: u64,
    /// When the job was last updated (microseconds since epoch).
    pub last_updated_at: u64,
}

impl BackfillJobState {
    /// Create a new backfill job state.
    pub fn new(
        job_id: [u8; 16],
        migration_id: [u8; 16],
        entity_name: impl Into<String>,
        field_name: impl Into<String>,
        batch_size: usize,
    ) -> Self {
        let now = current_timestamp();
        Self {
            job_id,
            migration_id,
            entity_name: entity_name.into(),
            field_name: field_name.into(),
            batch_size,
            last_processed_id: None,
            processed_count: 0,
            total_count: None,
            started_at: now,
            last_updated_at: now,
        }
    }

    /// Update the job state with progress.
    pub fn update(&mut self, last_id: [u8; 16], count: u64) {
        self.last_processed_id = Some(last_id);
        self.processed_count = count;
        self.last_updated_at = current_timestamp();
    }

    /// Calculate percentage complete.
    pub fn percent_complete(&self) -> Option<f64> {
        self.total_count.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.processed_count as f64 / total as f64) * 100.0
            }
        })
    }

    /// Serialize the state to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, MigrationError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|v| v.to_vec())
            .map_err(|e| MigrationError::Serialization(e.to_string()))
    }

    /// Deserialize state from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MigrationError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| MigrationError::Deserialization(e.to_string()))
    }
}

/// Migration state store for persistence.
pub struct MigrationStateStore {
    tree: sled::Tree,
}

impl MigrationStateStore {
    /// Tree name for migration state.
    pub const TREE_NAME: &'static str = "migration:state";

    /// Open or create the migration state store.
    pub fn open(db: &sled::Db) -> Result<Self, MigrationError> {
        let tree = db
            .open_tree(Self::TREE_NAME)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(Self { tree })
    }

    /// Save a migration state.
    pub fn save(&self, state: &MigrationState) -> Result<(), MigrationError> {
        let key = Self::migration_key(&state.migration_id);
        let value = state.to_bytes()?;
        self.tree
            .insert(key, value)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(())
    }

    /// Load a migration state.
    pub fn load(&self, migration_id: &[u8; 16]) -> Result<Option<MigrationState>, MigrationError> {
        let key = Self::migration_key(migration_id);
        match self
            .tree
            .get(key)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?
        {
            Some(bytes) => Ok(Some(MigrationState::from_bytes(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Delete a migration state.
    pub fn delete(&self, migration_id: &[u8; 16]) -> Result<(), MigrationError> {
        let key = Self::migration_key(migration_id);
        self.tree
            .remove(key)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(())
    }

    /// List all migration states.
    pub fn list(&self) -> Result<Vec<MigrationState>, MigrationError> {
        let mut states = Vec::new();
        for result in self.tree.iter() {
            let (key, value) = result
                .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;

            // Only process migration keys (not backfill job keys)
            if key.starts_with(b"migration:") {
                states.push(MigrationState::from_bytes(&value)?);
            }
        }
        Ok(states)
    }

    /// Get active migration (if any).
    pub fn get_active(&self) -> Result<Option<MigrationState>, MigrationError> {
        for state in self.list()? {
            if !state.is_terminal() {
                return Ok(Some(state));
            }
        }
        Ok(None)
    }

    /// Save a backfill job state.
    pub fn save_backfill_job(&self, state: &BackfillJobState) -> Result<(), MigrationError> {
        let key = Self::backfill_key(&state.job_id);
        let value = state.to_bytes()?;
        self.tree
            .insert(key, value)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(())
    }

    /// Load a backfill job state.
    pub fn load_backfill_job(
        &self,
        job_id: &[u8; 16],
    ) -> Result<Option<BackfillJobState>, MigrationError> {
        let key = Self::backfill_key(job_id);
        match self
            .tree
            .get(key)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?
        {
            Some(bytes) => Ok(Some(BackfillJobState::from_bytes(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Delete a backfill job state.
    pub fn delete_backfill_job(&self, job_id: &[u8; 16]) -> Result<(), MigrationError> {
        let key = Self::backfill_key(job_id);
        self.tree
            .remove(key)
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(())
    }

    /// Flush changes to disk.
    pub fn flush(&self) -> Result<(), MigrationError> {
        self.tree
            .flush()
            .map_err(|e| MigrationError::Storage(crate::error::Error::Storage(e)))?;
        Ok(())
    }

    fn migration_key(id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(26);
        key.extend_from_slice(b"migration:");
        key.extend_from_slice(id);
        key
    }

    fn backfill_key(id: &[u8; 16]) -> Vec<u8> {
        let mut key = Vec::with_capacity(25);
        key.extend_from_slice(b"backfill:");
        key.extend_from_slice(id);
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_migration_id() -> [u8; 16] {
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    }

    #[test]
    fn test_migration_state_lifecycle() {
        let id = create_migration_id();
        let mut state = MigrationState::new(id, 1, 2, 3);

        assert_eq!(state.status, MigrationStatus::Pending);
        assert!(!state.is_terminal());
        assert!(!state.can_resume());

        state.start();
        assert_eq!(state.status, MigrationStatus::Expanding);
        assert!(state.started_at.is_some());
        assert!(state.can_resume());

        state.start_backfilling();
        assert_eq!(state.status, MigrationStatus::Backfilling);

        state.start_contracting();
        assert_eq!(state.status, MigrationStatus::Contracting);

        state.complete();
        assert_eq!(state.status, MigrationStatus::Complete);
        assert!(state.is_terminal());
        assert!(state.completed_at.is_some());
    }

    #[test]
    fn test_migration_state_failure() {
        let id = create_migration_id();
        let mut state = MigrationState::new(id, 1, 2, 3);

        state.start();
        state.fail("Something went wrong");

        assert_eq!(state.status, MigrationStatus::Failed);
        assert!(state.is_terminal());
        assert_eq!(state.error.as_deref(), Some("Something went wrong"));
    }

    #[test]
    fn test_migration_state_rollback() {
        let id = create_migration_id();
        let mut state = MigrationState::new(id, 1, 2, 3);

        state.start();
        state.rollback();

        assert_eq!(state.status, MigrationStatus::RolledBack);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_step_progress() {
        let mut step = StepProgress::new(0);

        assert_eq!(step.status, StepStatus::Pending);
        assert!(step.started_at.is_none());

        step.start();
        assert_eq!(step.status, StepStatus::InProgress);
        assert!(step.started_at.is_some());

        step.update_progress(50, Some(100), Some([0u8; 16]));
        assert_eq!(step.processed_count, 50);
        assert_eq!(step.percent_complete(), Some(50.0));

        step.complete();
        assert_eq!(step.status, StepStatus::Complete);
        assert!(step.completed_at.is_some());
    }

    #[test]
    fn test_step_progress_failure() {
        let mut step = StepProgress::new(0);

        step.start();
        step.fail("Failed to process");

        assert_eq!(step.status, StepStatus::Failed);
        assert_eq!(step.error.as_deref(), Some("Failed to process"));
    }

    #[test]
    fn test_backfill_job_state() {
        let job_id = [1u8; 16];
        let migration_id = [2u8; 16];

        let mut job = BackfillJobState::new(job_id, migration_id, "User", "email", 1000);

        assert_eq!(job.processed_count, 0);
        assert!(job.last_processed_id.is_none());

        job.update([3u8; 16], 500);
        job.total_count = Some(1000);

        assert_eq!(job.processed_count, 500);
        assert_eq!(job.percent_complete(), Some(50.0));
    }

    #[test]
    fn test_migration_state_serialization() {
        let id = create_migration_id();
        let mut state = MigrationState::new(id, 1, 2, 3);
        state.start();

        let bytes = state.to_bytes().unwrap();
        let restored = MigrationState::from_bytes(&bytes).unwrap();

        assert_eq!(restored.migration_id, state.migration_id);
        assert_eq!(restored.from_version, state.from_version);
        assert_eq!(restored.to_version, state.to_version);
        assert_eq!(restored.status, state.status);
    }

    #[test]
    fn test_backfill_job_serialization() {
        let job_id = [1u8; 16];
        let migration_id = [2u8; 16];

        let job = BackfillJobState::new(job_id, migration_id, "User", "email", 1000);

        let bytes = job.to_bytes().unwrap();
        let restored = BackfillJobState::from_bytes(&bytes).unwrap();

        assert_eq!(restored.job_id, job.job_id);
        assert_eq!(restored.entity_name, job.entity_name);
        assert_eq!(restored.field_name, job.field_name);
    }

    #[test]
    fn test_state_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        let store = MigrationStateStore::open(&db).unwrap();

        let id = create_migration_id();
        let mut state = MigrationState::new(id, 1, 2, 3);
        state.start();

        // Save and load
        store.save(&state).unwrap();
        let loaded = store.load(&id).unwrap().unwrap();

        assert_eq!(loaded.migration_id, state.migration_id);
        assert_eq!(loaded.status, state.status);

        // Get active
        let active = store.get_active().unwrap();
        assert!(active.is_some());

        // List
        let all = store.list().unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        store.delete(&id).unwrap();
        assert!(store.load(&id).unwrap().is_none());
    }

    #[test]
    fn test_backfill_job_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        let store = MigrationStateStore::open(&db).unwrap();

        let job_id = [1u8; 16];
        let migration_id = [2u8; 16];
        let job = BackfillJobState::new(job_id, migration_id, "User", "email", 1000);

        // Save and load
        store.save_backfill_job(&job).unwrap();
        let loaded = store.load_backfill_job(&job_id).unwrap().unwrap();

        assert_eq!(loaded.job_id, job.job_id);
        assert_eq!(loaded.entity_name, job.entity_name);

        // Delete
        store.delete_backfill_job(&job_id).unwrap();
        assert!(store.load_backfill_job(&job_id).unwrap().is_none());
    }
}
