<script setup lang="ts">
import { computed } from 'vue'
import { useMetricsStore } from '../stores/metrics'
import { useSettingsStore } from '../stores/settings'
import MetricCard from './MetricCard.vue'
import RequestsPerSecondChart from './charts/RequestsPerSecondChart.vue'
import LatencyChart from './charts/LatencyChart.vue'
import CacheHitRateChart from './charts/CacheHitRateChart.vue'
import EntityCountsChart from './charts/EntityCountsChart.vue'

const metricsStore = useMetricsStore()
const settingsStore = useSettingsStore()

const formattedQueriesPerSec = computed(() => {
  return metricsStore.latestQueriesPerSec.toFixed(1)
})

const formattedMutationsPerSec = computed(() => {
  return metricsStore.latestMutationsPerSec.toFixed(1)
})

const formattedCacheRate = computed(() => {
  return `${metricsStore.cacheHitRate}%`
})

const avgLatencyMs = computed(() => {
  if (!metricsStore.currentMetrics) return '0'
  const us = metricsStore.currentMetrics.queries.avg_duration_us
  return (us / 1000).toFixed(2)
})

function toggleAutoRefresh() {
  settingsStore.toggleAutoRefreshMetrics()
}

function refreshNow() {
  metricsStore.fetchMetrics()
}
</script>

<template>
  <div class="metrics-dashboard">
    <!-- Header -->
    <div class="dashboard-header">
      <div class="header-left">
        <h2>Metrics Dashboard</h2>
        <span class="uptime">Uptime: {{ metricsStore.uptime }}</span>
      </div>
      <div class="header-right">
        <button class="refresh-btn" @click="refreshNow" :disabled="metricsStore.loading">
          Refresh
        </button>
        <label class="auto-refresh">
          <input
            type="checkbox"
            :checked="settingsStore.autoRefreshMetrics"
            @change="toggleAutoRefresh"
          />
          Auto-refresh
        </label>
      </div>
    </div>

    <!-- Error -->
    <div v-if="metricsStore.error" class="error-banner">
      {{ metricsStore.error }}
    </div>

    <!-- Metric Cards -->
    <div class="metric-cards">
      <MetricCard
        title="Total Queries"
        :value="metricsStore.totalQueries.toLocaleString()"
        :subtitle="`${formattedQueriesPerSec}/sec`"
        color="primary"
      />
      <MetricCard
        title="Total Mutations"
        :value="metricsStore.totalMutations.toLocaleString()"
        :subtitle="`${formattedMutationsPerSec}/sec`"
        color="success"
      />
      <MetricCard
        title="Cache Hit Rate"
        :value="formattedCacheRate"
        subtitle="Plan cache efficiency"
        color="warning"
      />
      <MetricCard
        title="Avg Latency"
        :value="`${avgLatencyMs}ms`"
        subtitle="Query execution time"
        color="primary"
      />
    </div>

    <!-- Charts Grid -->
    <div class="charts-grid">
      <div class="chart-item">
        <RequestsPerSecondChart />
      </div>
      <div class="chart-item">
        <LatencyChart />
      </div>
      <div class="chart-item">
        <CacheHitRateChart />
      </div>
      <div class="chart-item">
        <EntityCountsChart />
      </div>
    </div>

    <!-- Detailed Stats -->
    <div class="detailed-stats" v-if="metricsStore.currentMetrics">
      <div class="stats-section">
        <h3>Query Latencies</h3>
        <div class="stats-grid">
          <div class="stat-item">
            <span class="stat-label">P50</span>
            <span class="stat-value">{{ (metricsStore.currentMetrics.queries.p50_duration_us / 1000).toFixed(2) }}ms</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">P99</span>
            <span class="stat-value">{{ (metricsStore.currentMetrics.queries.p99_duration_us / 1000).toFixed(2) }}ms</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Max</span>
            <span class="stat-value">{{ (metricsStore.currentMetrics.queries.max_duration_us / 1000).toFixed(2) }}ms</span>
          </div>
        </div>
      </div>

      <div class="stats-section">
        <h3>Mutation Breakdown</h3>
        <div class="stats-grid">
          <div class="stat-item">
            <span class="stat-label">Inserts</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.mutations.inserts.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Updates</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.mutations.updates.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Deletes</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.mutations.deletes.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Upserts</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.mutations.upserts.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Rows Affected</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.mutations.rows_affected.toLocaleString() }}</span>
          </div>
        </div>
      </div>

      <div class="stats-section">
        <h3>Cache Statistics</h3>
        <div class="stats-grid">
          <div class="stat-item">
            <span class="stat-label">Hits</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.cache.hits.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Misses</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.cache.misses.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">Evictions</span>
            <span class="stat-value">{{ metricsStore.currentMetrics.cache.evictions.toLocaleString() }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.metrics-dashboard {
  padding: 1.5rem;
  overflow-y: auto;
  height: 100%;
}

.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

.header-left {
  display: flex;
  align-items: baseline;
  gap: 1rem;
}

.header-left h2 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text);
  margin: 0;
}

.uptime {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 1rem;
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

.auto-refresh {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.85rem;
  color: var(--color-text-secondary);
  cursor: pointer;
}

.auto-refresh input {
  cursor: pointer;
}

.error-banner {
  padding: 0.75rem 1rem;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid var(--color-error);
  border-radius: 6px;
  color: var(--color-error);
  margin-bottom: 1.5rem;
}

.metric-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 1rem;
  margin-bottom: 1.5rem;
}

@media (max-width: 1200px) {
  .metric-cards {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 768px) {
  .metric-cards {
    grid-template-columns: 1fr;
  }
}

.charts-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.chart-item {
  height: 250px;
}

@media (max-width: 1024px) {
  .charts-grid {
    grid-template-columns: 1fr;
  }
}

.detailed-stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
}

@media (max-width: 1024px) {
  .detailed-stats {
    grid-template-columns: 1fr;
  }
}

.stats-section {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem;
}

.stats-section h3 {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--color-text);
  margin: 0 0 0.75rem 0;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.stats-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
}

.stat-item {
  display: flex;
  flex-direction: column;
  min-width: 80px;
}

.stat-label {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.stat-value {
  font-size: 1rem;
  font-weight: 500;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}
</style>
