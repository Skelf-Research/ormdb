<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useSessionStore } from '../stores/session'
import { api, ReplicationStatus, CompactionResult } from '../api/client'

const session = useSessionStore()
const status = ref<ReplicationStatus | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const compacting = ref(false)
const lastCompaction = ref<CompactionResult | null>(null)

let pollingInterval: ReturnType<typeof setInterval> | null = null

async function fetchStatus() {
  if (!session.id) return

  loading.value = true
  error.value = null

  try {
    const response = await api.getReplicationStatus(session.id)
    status.value = response.replication
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to fetch status'
  } finally {
    loading.value = false
  }
}

async function triggerCompaction() {
  if (!session.id) return

  compacting.value = true
  error.value = null

  try {
    const response = await api.compact(session.id)
    lastCompaction.value = response.compaction
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Compaction failed'
  } finally {
    compacting.value = false
  }
}

function getRoleBadgeClass(role: string) {
  switch (role) {
    case 'primary':
      return 'role-primary'
    case 'replica':
      return 'role-replica'
    default:
      return 'role-standalone'
  }
}

function formatLsn(lsn: number): string {
  return lsn.toString(16).toUpperCase().padStart(16, '0')
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

onMounted(() => {
  fetchStatus()
  pollingInterval = setInterval(fetchStatus, 5000) // Poll every 5 seconds
})

onUnmounted(() => {
  if (pollingInterval) {
    clearInterval(pollingInterval)
  }
})
</script>

<template>
  <div class="cluster-status-panel">
    <div class="panel-header">
      <h2>Cluster Status</h2>
      <button class="refresh-btn" @click="fetchStatus" :disabled="loading">
        {{ loading ? 'Refreshing...' : 'Refresh' }}
      </button>
    </div>

    <!-- Error -->
    <div v-if="error" class="error-banner">
      {{ error }}
    </div>

    <!-- Loading -->
    <div v-else-if="loading && !status" class="loading-state">
      Loading cluster status...
    </div>

    <!-- Status -->
    <div v-else-if="status" class="status-content">
      <!-- Role Section -->
      <div class="status-section">
        <h3>Node Role</h3>
        <div class="role-display">
          <span :class="['role-badge', getRoleBadgeClass(status.role)]">
            {{ status.role.charAt(0).toUpperCase() + status.role.slice(1) }}
          </span>
          <span v-if="status.primary_addr" class="primary-addr">
            Connected to: {{ status.primary_addr }}
          </span>
        </div>
      </div>

      <!-- LSN Section -->
      <div class="status-section">
        <h3>Log Sequence Number</h3>
        <div class="lsn-display">
          <code class="lsn-value">{{ formatLsn(status.current_lsn) }}</code>
          <span class="lsn-label">Current LSN</span>
        </div>
      </div>

      <!-- Replication Lag (for replicas) -->
      <div v-if="status.role === 'replica'" class="status-section">
        <h3>Replication Lag</h3>
        <div class="lag-grid">
          <div class="lag-item">
            <span class="lag-value" :class="{ warning: status.lag_entries > 100 }">
              {{ status.lag_entries.toLocaleString() }}
            </span>
            <span class="lag-label">Entries behind</span>
          </div>
          <div class="lag-item">
            <span class="lag-value" :class="{ warning: status.lag_ms > 1000 }">
              {{ status.lag_ms.toLocaleString() }}ms
            </span>
            <span class="lag-label">Time lag</span>
          </div>
        </div>
      </div>

      <!-- Standalone Mode Info -->
      <div v-if="status.role === 'standalone'" class="info-section">
        <div class="info-box">
          <h4>Standalone Mode</h4>
          <p>
            This session is running in standalone mode with an isolated, temporary database.
            Data will not be replicated and will be lost when the session ends.
          </p>
          <p>
            To connect to a replicated cluster, use the connection settings to specify
            a server address.
          </p>
        </div>
      </div>

      <!-- Storage Management -->
      <div class="status-section">
        <h3>Storage Management</h3>
        <div class="storage-info">
          <p class="storage-desc">
            Storage is automatically compacted every 5 minutes. Old versions (>1 hour) and
            excess versions (>10 per entity) are cleaned up to save space.
          </p>
          <button class="compact-btn" @click="triggerCompaction" :disabled="compacting">
            {{ compacting ? 'Compacting...' : 'Compact Now' }}
          </button>
        </div>

        <!-- Last Compaction Result -->
        <div v-if="lastCompaction" class="compaction-result">
          <h4>Last Compaction</h4>
          <div class="compaction-stats">
            <div class="compaction-stat">
              <span class="stat-value">{{ lastCompaction.versions_removed }}</span>
              <span class="stat-label">Versions Removed</span>
            </div>
            <div class="compaction-stat">
              <span class="stat-value">{{ lastCompaction.tombstones_removed }}</span>
              <span class="stat-label">Tombstones Removed</span>
            </div>
            <div class="compaction-stat">
              <span class="stat-value">{{ formatBytes(lastCompaction.bytes_reclaimed) }}</span>
              <span class="stat-label">Bytes Reclaimed</span>
            </div>
            <div class="compaction-stat">
              <span class="stat-value">{{ lastCompaction.duration_ms }}ms</span>
              <span class="stat-label">Duration</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Primary Mode Info -->
      <div v-if="status.role === 'primary'" class="status-section">
        <h3>Primary Node</h3>
        <div class="info-box">
          <p>
            This node is the primary and accepts both reads and writes.
            Changes are replicated to all connected replicas.
          </p>
        </div>
      </div>

      <!-- Replica Mode Info -->
      <div v-if="status.role === 'replica'" class="status-section">
        <h3>Replica Node</h3>
        <div class="info-box">
          <p>
            This node is a read-only replica. Write operations will be
            forwarded to the primary node.
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cluster-status-panel {
  padding: 1.5rem;
  overflow-y: auto;
  height: 100%;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

.panel-header h2 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text);
  margin: 0;
}

.refresh-btn {
  padding: 0.5rem 1rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-bg-tertiary);
  color: var(--color-text);
  border-radius: 6px;
  font-size: 0.85rem;
  cursor: pointer;
  transition: all 0.2s;
}

.refresh-btn:hover:not(:disabled) {
  background: var(--color-bg-tertiary);
}

.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.error-banner {
  padding: 0.75rem 1rem;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid var(--color-error);
  border-radius: 6px;
  color: var(--color-error);
  margin-bottom: 1.5rem;
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--color-text-secondary);
}

.status-content {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.status-section {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem 1.25rem;
}

.status-section h3 {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 0 0 0.75rem 0;
}

.role-display {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.role-badge {
  display: inline-block;
  padding: 0.5rem 1rem;
  border-radius: 6px;
  font-size: 1rem;
  font-weight: 600;
}

.role-primary {
  background: rgba(59, 130, 246, 0.1);
  color: #3b82f6;
  border: 1px solid rgba(59, 130, 246, 0.3);
}

.role-replica {
  background: rgba(139, 92, 246, 0.1);
  color: #8b5cf6;
  border: 1px solid rgba(139, 92, 246, 0.3);
}

.role-standalone {
  background: rgba(107, 114, 128, 0.1);
  color: #9ca3af;
  border: 1px solid rgba(107, 114, 128, 0.3);
}

.primary-addr {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
}

.lsn-display {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.lsn-value {
  font-family: 'Fira Code', monospace;
  font-size: 1.25rem;
  color: var(--color-text);
  background: var(--color-bg);
  padding: 0.5rem 1rem;
  border-radius: 4px;
  display: inline-block;
}

.lsn-label {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
}

.lag-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1rem;
}

.lag-item {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.lag-value {
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}

.lag-value.warning {
  color: #f59e0b;
}

.lag-label {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
}

.info-section {
  margin-top: 1rem;
}

.info-box {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem 1.25rem;
  border-left: 3px solid var(--color-primary);
}

.info-box h4 {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text);
  margin: 0 0 0.5rem 0;
}

.info-box p {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
  margin: 0 0 0.5rem 0;
  line-height: 1.5;
}

.info-box p:last-child {
  margin-bottom: 0;
}

/* Storage Management */
.storage-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.storage-desc {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
  margin: 0;
  flex: 1;
}

.compact-btn {
  padding: 0.5rem 1rem;
  background: var(--color-bg-tertiary);
  color: var(--color-text);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 6px;
  font-size: 0.85rem;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}

.compact-btn:hover:not(:disabled) {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-bg);
}

.compact-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.compaction-result {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--color-bg-tertiary);
}

.compaction-result h4 {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--color-text-secondary);
  margin: 0 0 0.75rem 0;
}

.compaction-stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 1rem;
}

@media (max-width: 768px) {
  .compaction-stats {
    grid-template-columns: repeat(2, 1fr);
  }
}

.compaction-stat {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.compaction-stat .stat-value {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}

.compaction-stat .stat-label {
  font-size: 0.7rem;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.03em;
}
</style>
