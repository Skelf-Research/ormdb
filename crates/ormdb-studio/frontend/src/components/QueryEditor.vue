<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSessionStore } from '../stores/session'
import { api } from '../api/client'

const session = useSessionStore()
const query = ref('User.findMany()')
const result = ref<any>(null)
const error = ref<string | null>(null)
const loading = ref(false)
const resultFormat = ref<'table' | 'json'>('table')

async function executeQuery() {
  if (!session.id || !query.value.trim()) return

  loading.value = true
  error.value = null
  result.value = null

  try {
    const response = await api.executeRawQuery(session.id, query.value)
    result.value = response.data || response
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Query failed'
  } finally {
    loading.value = false
  }
}

function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    executeQuery()
  }
}
</script>

<template>
  <div class="query-editor">
    <!-- Editor Panel -->
    <div class="editor-panel">
      <div class="editor-header">
        <span>Query</span>
        <div class="editor-actions">
          <button class="run-button" @click="executeQuery" :disabled="loading">
            {{ loading ? 'Running...' : 'Run (Ctrl+Enter)' }}
          </button>
        </div>
      </div>
      <textarea
        v-model="query"
        class="editor-textarea"
        placeholder="Enter your query here..."
        @keydown="handleKeydown"
        spellcheck="false"
      ></textarea>
    </div>

    <!-- Results Panel -->
    <div class="results-panel">
      <div class="results-header">
        <span>Results</span>
        <div class="format-toggle">
          <button
            :class="['format-btn', { active: resultFormat === 'table' }]"
            @click="resultFormat = 'table'"
          >
            Table
          </button>
          <button
            :class="['format-btn', { active: resultFormat === 'json' }]"
            @click="resultFormat = 'json'"
          >
            JSON
          </button>
        </div>
      </div>

      <div class="results-content">
        <!-- Error -->
        <div v-if="error" class="error-message">
          {{ error }}
        </div>

        <!-- Loading -->
        <div v-else-if="loading" class="loading">
          Executing query...
        </div>

        <!-- Results -->
        <div v-else-if="result">
          <pre v-if="resultFormat === 'json'" class="json-result">{{ JSON.stringify(result, null, 2) }}</pre>
          <div v-else class="table-result">
            <p class="placeholder-text">Table view coming soon...</p>
            <pre class="json-result">{{ JSON.stringify(result, null, 2) }}</pre>
          </div>
        </div>

        <!-- Empty state -->
        <div v-else class="empty-state">
          Run a query to see results
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.query-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.editor-panel {
  display: flex;
  flex-direction: column;
  height: 40%;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.editor-header,
.results-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 1rem;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-bg-tertiary);
  font-size: 0.9rem;
  color: var(--color-text-secondary);
}

.editor-textarea {
  flex: 1;
  padding: 1rem;
  background: var(--color-bg);
  border: none;
  color: var(--color-text);
  font-family: 'Fira Code', monospace;
  font-size: 14px;
  resize: none;
  outline: none;
}

.run-button {
  padding: 0.5rem 1rem;
  background: var(--color-primary);
  color: var(--color-bg);
  border: none;
  border-radius: 4px;
  font-size: 0.85rem;
  cursor: pointer;
  transition: opacity 0.2s;
}

.run-button:hover:not(:disabled) {
  opacity: 0.9;
}

.run-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.results-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
}

.format-toggle {
  display: flex;
  gap: 0.25rem;
}

.format-btn {
  padding: 0.25rem 0.75rem;
  background: transparent;
  border: 1px solid var(--color-bg-tertiary);
  color: var(--color-text-secondary);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.format-btn:first-child {
  border-radius: 4px 0 0 4px;
}

.format-btn:last-child {
  border-radius: 0 4px 4px 0;
}

.format-btn.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-bg);
}

.results-content {
  flex: 1;
  padding: 1rem;
  overflow: auto;
}

.error-message {
  padding: 1rem;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid var(--color-error);
  border-radius: 4px;
  color: var(--color-error);
}

.loading,
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
}

.json-result {
  padding: 1rem;
  background: var(--color-bg-secondary);
  border-radius: 4px;
  font-family: 'Fira Code', monospace;
  font-size: 13px;
  overflow: auto;
  white-space: pre-wrap;
}

.placeholder-text {
  color: var(--color-text-secondary);
  margin-bottom: 1rem;
}
</style>
