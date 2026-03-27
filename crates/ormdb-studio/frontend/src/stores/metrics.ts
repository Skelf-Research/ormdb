import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import { api, MetricsData } from '../api/client'
import { useSessionStore } from './session'
import { useSettingsStore } from './settings'

export interface MetricsHistoryPoint {
  timestamp: number
  queriesPerSec: number
  mutationsPerSec: number
  avgLatencyMs: number
  cacheHitRate: number
}

const MAX_HISTORY_POINTS = 60 // 2 minutes at 2-second intervals

export const useMetricsStore = defineStore('metrics', () => {
  const sessionStore = useSessionStore()
  const settingsStore = useSettingsStore()

  // Current metrics data
  const currentMetrics = ref<MetricsData | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  // Historical data for charts
  const history = ref<MetricsHistoryPoint[]>([])

  // Previous values for rate calculation
  let previousQueryCount = 0
  let previousMutationCount = 0
  let previousTimestamp = 0

  // Polling state
  let pollingInterval: ReturnType<typeof setInterval> | null = null

  // Computed metrics
  const uptime = computed(() => {
    if (!currentMetrics.value) return '0s'
    const secs = currentMetrics.value.uptime_secs
    const hours = Math.floor(secs / 3600)
    const minutes = Math.floor((secs % 3600) / 60)
    const seconds = secs % 60
    if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`
    if (minutes > 0) return `${minutes}m ${seconds}s`
    return `${seconds}s`
  })

  const totalQueries = computed(() => currentMetrics.value?.queries.total_count ?? 0)
  const totalMutations = computed(() => currentMetrics.value?.mutations.total_count ?? 0)
  const cacheHitRate = computed(() => {
    if (!currentMetrics.value) return 0
    return Math.round(currentMetrics.value.cache.hit_rate * 100)
  })
  const totalEntities = computed(() => currentMetrics.value?.storage.total_entities ?? 0)

  // Latest rates (from most recent history point)
  const latestQueriesPerSec = computed(() => {
    if (history.value.length === 0) return 0
    return history.value[history.value.length - 1].queriesPerSec
  })

  const latestMutationsPerSec = computed(() => {
    if (history.value.length === 0) return 0
    return history.value[history.value.length - 1].mutationsPerSec
  })

  async function fetchMetrics() {
    if (!sessionStore.id) return

    loading.value = true
    error.value = null

    try {
      const response = await api.getMetrics(sessionStore.id)
      const newMetrics = response.metrics
      const now = Date.now()

      // Calculate rates if we have previous data
      if (previousTimestamp > 0) {
        const timeDelta = (now - previousTimestamp) / 1000 // seconds
        if (timeDelta > 0) {
          const queryDelta = newMetrics.queries.total_count - previousQueryCount
          const mutationDelta = newMetrics.mutations.total_count - previousMutationCount

          const historyPoint: MetricsHistoryPoint = {
            timestamp: now,
            queriesPerSec: Math.max(0, queryDelta / timeDelta),
            mutationsPerSec: Math.max(0, mutationDelta / timeDelta),
            avgLatencyMs: newMetrics.queries.avg_duration_us / 1000,
            cacheHitRate: newMetrics.cache.hit_rate * 100,
          }

          history.value.push(historyPoint)

          // Trim history to max size
          if (history.value.length > MAX_HISTORY_POINTS) {
            history.value = history.value.slice(-MAX_HISTORY_POINTS)
          }
        }
      }

      // Update previous values for next rate calculation
      previousQueryCount = newMetrics.queries.total_count
      previousMutationCount = newMetrics.mutations.total_count
      previousTimestamp = now

      currentMetrics.value = newMetrics
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch metrics'
    } finally {
      loading.value = false
    }
  }

  function startPolling() {
    stopPolling() // Clear any existing interval

    if (!settingsStore.autoRefreshMetrics) return

    // Fetch immediately
    fetchMetrics()

    // Set up polling
    pollingInterval = setInterval(() => {
      if (settingsStore.autoRefreshMetrics) {
        fetchMetrics()
      }
    }, settingsStore.metricsPollingInterval)
  }

  function stopPolling() {
    if (pollingInterval) {
      clearInterval(pollingInterval)
      pollingInterval = null
    }
  }

  function resetHistory() {
    history.value = []
    previousQueryCount = 0
    previousMutationCount = 0
    previousTimestamp = 0
  }

  // Watch for session changes
  watch(
    () => sessionStore.id,
    (newId) => {
      if (newId) {
        resetHistory()
        startPolling()
      } else {
        stopPolling()
        currentMetrics.value = null
        resetHistory()
      }
    },
    { immediate: true }
  )

  // Watch for settings changes
  watch(
    () => settingsStore.metricsPollingInterval,
    () => {
      if (sessionStore.id && settingsStore.autoRefreshMetrics) {
        startPolling() // Restart with new interval
      }
    }
  )

  watch(
    () => settingsStore.autoRefreshMetrics,
    (autoRefresh) => {
      if (autoRefresh && sessionStore.id) {
        startPolling()
      } else {
        stopPolling()
      }
    }
  )

  return {
    // State
    currentMetrics,
    loading,
    error,
    history,

    // Computed
    uptime,
    totalQueries,
    totalMutations,
    cacheHitRate,
    totalEntities,
    latestQueriesPerSec,
    latestMutationsPerSec,

    // Actions
    fetchMetrics,
    startPolling,
    stopPolling,
    resetHistory,
  }
})
