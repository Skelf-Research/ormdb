import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export const useSettingsStore = defineStore('settings', () => {
  // Server connection settings
  const serverAddress = ref(localStorage.getItem('ormdb-server-addr') || '')

  // Output preferences
  const outputFormat = ref<'table' | 'json'>(
    (localStorage.getItem('ormdb-output-format') as 'table' | 'json') || 'table'
  )

  // Theme preference
  const theme = ref<'dark' | 'light'>(
    (localStorage.getItem('ormdb-theme') as 'dark' | 'light') || 'dark'
  )

  // Metrics polling interval in milliseconds
  const metricsPollingInterval = ref(
    parseInt(localStorage.getItem('ormdb-metrics-interval') || '2000', 10)
  )

  // Auto-refresh settings
  const autoRefreshMetrics = ref(
    localStorage.getItem('ormdb-auto-refresh-metrics') !== 'false'
  )

  // Persist settings to localStorage
  function saveSettings() {
    localStorage.setItem('ormdb-server-addr', serverAddress.value)
    localStorage.setItem('ormdb-output-format', outputFormat.value)
    localStorage.setItem('ormdb-theme', theme.value)
    localStorage.setItem('ormdb-metrics-interval', metricsPollingInterval.value.toString())
    localStorage.setItem('ormdb-auto-refresh-metrics', autoRefreshMetrics.value.toString())
  }

  // Watch for changes and auto-save
  watch([serverAddress, outputFormat, theme, metricsPollingInterval, autoRefreshMetrics], () => {
    saveSettings()
  })

  function setServerAddress(address: string) {
    serverAddress.value = address
  }

  function setOutputFormat(format: 'table' | 'json') {
    outputFormat.value = format
  }

  function setTheme(newTheme: 'dark' | 'light') {
    theme.value = newTheme
  }

  function setMetricsPollingInterval(interval: number) {
    metricsPollingInterval.value = Math.max(1000, interval) // Minimum 1 second
  }

  function toggleAutoRefreshMetrics() {
    autoRefreshMetrics.value = !autoRefreshMetrics.value
  }

  function resetToDefaults() {
    serverAddress.value = ''
    outputFormat.value = 'table'
    theme.value = 'dark'
    metricsPollingInterval.value = 2000
    autoRefreshMetrics.value = true
    saveSettings()
  }

  return {
    // State
    serverAddress,
    outputFormat,
    theme,
    metricsPollingInterval,
    autoRefreshMetrics,

    // Actions
    saveSettings,
    setServerAddress,
    setOutputFormat,
    setTheme,
    setMetricsPollingInterval,
    toggleAutoRefreshMetrics,
    resetToDefaults,
  }
})
