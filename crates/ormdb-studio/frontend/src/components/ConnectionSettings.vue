<script setup lang="ts">
import { ref } from 'vue'
import { useSettingsStore } from '../stores/settings'

const emit = defineEmits<{
  (e: 'close'): void
}>()

const settings = useSettingsStore()

// Local state for form
const serverAddress = ref(settings.serverAddress)
const outputFormat = ref(settings.outputFormat)
const theme = ref(settings.theme)
const metricsInterval = ref(settings.metricsPollingInterval)
const autoRefresh = ref(settings.autoRefreshMetrics)

function saveSettings() {
  settings.setServerAddress(serverAddress.value)
  settings.setOutputFormat(outputFormat.value)
  settings.setTheme(theme.value)
  settings.setMetricsPollingInterval(metricsInterval.value)
  if (autoRefresh.value !== settings.autoRefreshMetrics) {
    settings.toggleAutoRefreshMetrics()
  }
  emit('close')
}

function resetDefaults() {
  settings.resetToDefaults()
  serverAddress.value = settings.serverAddress
  outputFormat.value = settings.outputFormat
  theme.value = settings.theme
  metricsInterval.value = settings.metricsPollingInterval
  autoRefresh.value = settings.autoRefreshMetrics
}

function handleBackdropClick(event: MouseEvent) {
  if (event.target === event.currentTarget) {
    emit('close')
  }
}
</script>

<template>
  <div class="modal-backdrop" @click="handleBackdropClick">
    <div class="modal-content">
      <div class="modal-header">
        <h2>Settings</h2>
        <button class="close-btn" @click="emit('close')">
          <span>&times;</span>
        </button>
      </div>

      <div class="modal-body">
        <!-- Connection Section -->
        <div class="settings-section">
          <h3>Connection</h3>
          <div class="form-group">
            <label for="server-address">Server Address</label>
            <input
              id="server-address"
              v-model="serverAddress"
              type="text"
              placeholder="http://localhost:8080"
              class="form-input"
            />
            <span class="form-hint">
              Leave empty for embedded mode (current session's temporary database)
            </span>
          </div>
        </div>

        <!-- Display Section -->
        <div class="settings-section">
          <h3>Display</h3>
          <div class="form-group">
            <label for="output-format">Default Output Format</label>
            <select id="output-format" v-model="outputFormat" class="form-select">
              <option value="table">Table</option>
              <option value="json">JSON</option>
            </select>
          </div>

          <div class="form-group">
            <label for="theme">Theme</label>
            <select id="theme" v-model="theme" class="form-select">
              <option value="dark">Dark</option>
              <option value="light">Light</option>
            </select>
          </div>
        </div>

        <!-- Metrics Section -->
        <div class="settings-section">
          <h3>Metrics</h3>
          <div class="form-group">
            <label for="metrics-interval">Polling Interval (ms)</label>
            <input
              id="metrics-interval"
              v-model.number="metricsInterval"
              type="number"
              min="1000"
              max="30000"
              step="500"
              class="form-input"
            />
            <span class="form-hint">
              How often to refresh metrics (minimum 1000ms)
            </span>
          </div>

          <div class="form-group">
            <label class="checkbox-label">
              <input type="checkbox" v-model="autoRefresh" />
              <span>Auto-refresh metrics</span>
            </label>
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn btn-secondary" @click="resetDefaults">
          Reset to Defaults
        </button>
        <div class="footer-right">
          <button class="btn btn-secondary" @click="emit('close')">
            Cancel
          </button>
          <button class="btn btn-primary" @click="saveSettings">
            Save Changes
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--color-bg);
  border-radius: 12px;
  width: 90%;
  max-width: 500px;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.5rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.modal-header h2 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text);
  margin: 0;
}

.close-btn {
  background: none;
  border: none;
  color: var(--color-text-secondary);
  font-size: 1.5rem;
  cursor: pointer;
  padding: 0;
  line-height: 1;
  transition: color 0.2s;
}

.close-btn:hover {
  color: var(--color-text);
}

.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 1.5rem;
}

.settings-section {
  margin-bottom: 1.5rem;
}

.settings-section:last-child {
  margin-bottom: 0;
}

.settings-section h3 {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 0 0 1rem 0;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.form-group {
  margin-bottom: 1rem;
}

.form-group:last-child {
  margin-bottom: 0;
}

.form-group label {
  display: block;
  font-size: 0.9rem;
  color: var(--color-text);
  margin-bottom: 0.5rem;
}

.form-input,
.form-select {
  width: 100%;
  padding: 0.6rem 0.75rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 6px;
  color: var(--color-text);
  font-size: 0.9rem;
  transition: border-color 0.2s;
}

.form-input:focus,
.form-select:focus {
  outline: none;
  border-color: var(--color-primary);
}

.form-select {
  cursor: pointer;
}

.form-hint {
  display: block;
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  margin-top: 0.25rem;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
}

.checkbox-label input {
  cursor: pointer;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.5rem;
  border-top: 1px solid var(--color-bg-tertiary);
  background: var(--color-bg-secondary);
}

.footer-right {
  display: flex;
  gap: 0.5rem;
}

.btn {
  padding: 0.6rem 1rem;
  border-radius: 6px;
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-primary {
  background: var(--color-primary);
  color: var(--color-bg);
  border: none;
}

.btn-primary:hover {
  opacity: 0.9;
}

.btn-secondary {
  background: var(--color-bg);
  color: var(--color-text);
  border: 1px solid var(--color-bg-tertiary);
}

.btn-secondary:hover {
  background: var(--color-bg-tertiary);
}
</style>
