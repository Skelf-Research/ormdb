<script setup lang="ts">
import { ref, computed } from 'vue'
import { useSessionStore } from '../stores/session'
import { api, ExplainResult } from '../api/client'
import ResultsTable from './ResultsTable.vue'

const session = useSessionStore()
const query = ref('Movie.findMany()')
const result = ref<any>(null)
const explainResult = ref<ExplainResult | null>(null)
const error = ref<string | null>(null)
const loading = ref(false)
const explaining = ref(false)
const resultFormat = ref<'table' | 'json'>('table')
const showExplain = ref(false)
const selectedSample = ref('')
const durationMs = ref<number | null>(null)

// Sample queries for the movie database demo
const sampleQueries: Record<string, { label: string; query: string; category: string }> = {
  // Basic Queries
  all_movies: { label: 'All Movies', query: 'Movie.findMany()', category: 'Basic' },
  all_actors: { label: 'All Actors', query: 'Actor.findMany()', category: 'Basic' },
  all_directors: { label: 'All Directors', query: 'Director.findMany()', category: 'Basic' },
  all_genres: { label: 'All Genres', query: 'Genre.findMany()', category: 'Basic' },

  // Filtering
  movies_2020: { label: 'Movies from 2020+', query: 'Movie.findMany().where(year >= 2020)', category: 'Filtering' },
  high_rated: { label: 'Highly Rated (8+)', query: 'Movie.findMany().where(rating >= 8.0)', category: 'Filtering' },
  sci_fi: { label: 'Sci-Fi Movies', query: 'Movie.findMany().where(year >= 2010)', category: 'Filtering' },
  young_actors: { label: 'Actors Born After 1980', query: 'Actor.findMany().where(birth_year >= 1980)', category: 'Filtering' },

  // Sorting & Pagination
  top_5: { label: 'Top 5 by Rating', query: 'Movie.findMany().orderBy(rating.desc).limit(5)', category: 'Sorting' },
  newest_10: { label: '10 Newest Movies', query: 'Movie.findMany().orderBy(year.desc).limit(10)', category: 'Sorting' },
  actors_alpha: { label: 'Actors Alphabetically', query: 'Actor.findMany().orderBy(name.asc)', category: 'Sorting' },

  // Aggregations
  count_movies: { label: 'Count Movies', query: 'Movie.count()', category: 'Aggregations' },
  count_actors: { label: 'Count Actors', query: 'Actor.count()', category: 'Aggregations' },
  count_high_rated: { label: 'Count Highly Rated', query: 'Movie.count().where(rating >= 8.0)', category: 'Aggregations' },

  // Schema Commands
  schema: { label: 'List Entities', query: '.schema', category: 'Schema' },
  schema_movie: { label: 'Describe Movie', query: '.schema Movie', category: 'Schema' },
  help: { label: 'Show Help', query: '.help', category: 'Schema' },
}

// Group samples by category
const sampleCategories = computed(() => {
  const categories: Record<string, { key: string; label: string; query: string }[]> = {}
  for (const [key, value] of Object.entries(sampleQueries)) {
    if (!categories[value.category]) {
      categories[value.category] = []
    }
    categories[value.category].push({ key, label: value.label, query: value.query })
  }
  return categories
})

function loadSample() {
  if (selectedSample.value && sampleQueries[selectedSample.value]) {
    query.value = sampleQueries[selectedSample.value].query
    selectedSample.value = '' // Reset selection
  }
}

async function executeQuery() {
  if (!session.id || !query.value.trim()) return

  loading.value = true
  error.value = null
  result.value = null
  durationMs.value = null
  showExplain.value = false

  try {
    const response = await api.executeRawQuery(session.id, query.value) as any
    result.value = response.data || response
    durationMs.value = response.duration_ms ?? null
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Query failed'
  } finally {
    loading.value = false
  }
}

async function explainQuery() {
  if (!session.id || !query.value.trim()) return

  explaining.value = true
  error.value = null
  explainResult.value = null
  showExplain.value = true

  try {
    const response = await api.explainQuery(session.id, query.value)
    explainResult.value = response.explain
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Explain failed'
  } finally {
    explaining.value = false
  }
}

function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    executeQuery()
  }
}

// Convert result to array for table display
const tableData = computed(() => {
  if (!result.value) return []

  // Handle entity blocks from query results: [{entity: "Movie", rows: [...], count: N}]
  if (Array.isArray(result.value) && result.value.length > 0 && result.value[0].rows) {
    // Flatten all entity blocks into a single array of rows
    return result.value.flatMap((block: any) => block.rows || [])
  }

  // Handle direct array of rows
  if (Array.isArray(result.value)) return result.value

  // Handle {rows: [...]} structure
  if (result.value.rows && Array.isArray(result.value.rows)) return result.value.rows

  // Handle {data: [...]} structure
  if (result.value.data && Array.isArray(result.value.data)) return result.value.data

  // Handle aggregate results: {entity: "Movie", aggregations: [...]}
  if (result.value.aggregations && Array.isArray(result.value.aggregations)) {
    return result.value.aggregations.map((agg: any) => ({
      function: agg.function,
      field: agg.field || '*',
      value: agg.value
    }))
  }

  // Handle schema command results
  if (result.value.command) {
    if (result.value.entities) {
      return result.value.entities.map((e: string) => ({ entity: e }))
    }
    if (result.value.fields) {
      return result.value.fields
    }
  }

  // Single object result
  if (typeof result.value === 'object') return [result.value]
  return []
})
</script>

<template>
  <div class="query-editor">
    <!-- Editor Panel -->
    <div class="editor-panel">
      <div class="editor-header">
        <span>Query</span>
        <select v-model="selectedSample" @change="loadSample" class="sample-select">
          <option value="">Sample Queries...</option>
          <optgroup v-for="(samples, category) in sampleCategories" :key="category" :label="category">
            <option v-for="sample in samples" :key="sample.key" :value="sample.key">
              {{ sample.label }}
            </option>
          </optgroup>
        </select>
        <div class="editor-actions">
          <button class="explain-button" @click="explainQuery" :disabled="explaining || loading">
            {{ explaining ? 'Explaining...' : 'Explain' }}
          </button>
          <button class="run-button" @click="executeQuery" :disabled="loading || explaining">
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
        <div class="results-title">
          <span>{{ showExplain ? 'Query Plan' : 'Results' }}</span>
          <span v-if="durationMs !== null && !showExplain" class="duration-badge">
            {{ durationMs < 1 ? '< 1ms' : durationMs >= 1000 ? (durationMs / 1000).toFixed(2) + 's' : durationMs.toFixed(1) + 'ms' }}
          </span>
        </div>
        <div v-if="!showExplain" class="format-toggle">
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
        <button v-if="showExplain && result" class="back-btn" @click="showExplain = false">
          Back to Results
        </button>
      </div>

      <div class="results-content">
        <!-- Error -->
        <div v-if="error" class="error-message">
          {{ error }}
        </div>

        <!-- Explain Loading -->
        <div v-else-if="explaining" class="loading">
          Analyzing query plan...
        </div>

        <!-- Explain Result -->
        <div v-else-if="showExplain && explainResult" class="explain-result">
          <div class="explain-section">
            <h4>Execution Plan</h4>
            <pre class="plan-text">{{ explainResult.plan }}</pre>
          </div>

          <div class="explain-section">
            <h4>Cost Breakdown</h4>
            <div class="cost-grid">
              <div class="cost-item">
                <span class="cost-label">Total Cost</span>
                <span class="cost-value">{{ explainResult.cost.total_cost.toFixed(2) }}</span>
              </div>
              <div class="cost-item">
                <span class="cost-label">Estimated Rows</span>
                <span class="cost-value">{{ explainResult.cost.estimated_rows }}</span>
              </div>
              <div class="cost-item">
                <span class="cost-label">I/O Cost</span>
                <span class="cost-value">{{ explainResult.cost.io_cost.toFixed(2) }}</span>
              </div>
              <div class="cost-item">
                <span class="cost-label">CPU Cost</span>
                <span class="cost-value">{{ explainResult.cost.cpu_cost.toFixed(2) }}</span>
              </div>
            </div>
          </div>

          <div v-if="explainResult.joins.length > 0" class="explain-section">
            <h4>Join Strategies</h4>
            <div v-for="(join, idx) in explainResult.joins" :key="idx" class="join-item">
              <span class="join-path">{{ join.path }}</span>
              <span class="join-strategy">{{ join.strategy }}</span>
              <span class="join-reason">{{ join.reason }}</span>
            </div>
          </div>

          <div class="explain-section">
            <span :class="['cache-badge', { cached: explainResult.plan_cached }]">
              {{ explainResult.plan_cached ? 'Plan Cached' : 'Plan Not Cached' }}
            </span>
          </div>
        </div>

        <!-- Loading -->
        <div v-else-if="loading" class="loading">
          Executing query...
        </div>

        <!-- Results -->
        <div v-else-if="result" class="result-container">
          <pre v-if="resultFormat === 'json'" class="json-result">{{ JSON.stringify(result, null, 2) }}</pre>
          <ResultsTable v-else :data="tableData" :loading="loading" />
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

.editor-actions {
  display: flex;
  gap: 0.5rem;
}

.results-title {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.duration-badge {
  padding: 0.15rem 0.5rem;
  background: var(--color-bg-tertiary);
  border-radius: 4px;
  font-size: 0.75rem;
  font-family: 'Fira Code', monospace;
  color: var(--color-success);
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

.run-button,
.explain-button {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 4px;
  font-size: 0.85rem;
  cursor: pointer;
  transition: opacity 0.2s;
}

.run-button {
  background: var(--color-primary);
  color: var(--color-bg);
}

.explain-button {
  background: var(--color-bg-tertiary);
  color: var(--color-text);
}

.sample-select {
  padding: 0.5rem 0.75rem;
  background: var(--color-bg-tertiary);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.85rem;
  cursor: pointer;
  outline: none;
}

.sample-select:hover {
  border-color: var(--color-primary);
}

.sample-select:focus {
  border-color: var(--color-primary);
}

.run-button:hover:not(:disabled),
.explain-button:hover:not(:disabled) {
  opacity: 0.9;
}

.run-button:disabled,
.explain-button:disabled {
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

.back-btn {
  padding: 0.25rem 0.75rem;
  background: var(--color-bg-tertiary);
  border: none;
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.8rem;
  cursor: pointer;
}

.back-btn:hover {
  opacity: 0.9;
}

.results-content {
  flex: 1;
  overflow: auto;
}

.result-container {
  height: 100%;
}

.error-message {
  margin: 1rem;
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
  margin: 1rem;
  padding: 1rem;
  background: var(--color-bg-secondary);
  border-radius: 4px;
  font-family: 'Fira Code', monospace;
  font-size: 13px;
  overflow: auto;
  white-space: pre-wrap;
  color: var(--color-text);
}

/* Explain styles */
.explain-result {
  padding: 1rem;
}

.explain-section {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem;
  margin-bottom: 1rem;
}

.explain-section h4 {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--color-text);
  margin: 0 0 0.75rem 0;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.plan-text {
  font-family: 'Fira Code', monospace;
  font-size: 0.85rem;
  color: var(--color-text);
  white-space: pre-wrap;
  margin: 0;
}

.cost-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 1rem;
}

@media (max-width: 768px) {
  .cost-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.cost-item {
  display: flex;
  flex-direction: column;
}

.cost-label {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.cost-value {
  font-size: 1.25rem;
  font-weight: 500;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}

.join-item {
  display: flex;
  gap: 1rem;
  padding: 0.5rem 0;
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.join-item:last-child {
  border-bottom: none;
}

.join-path {
  font-family: 'Fira Code', monospace;
  color: var(--color-primary);
}

.join-strategy {
  font-weight: 500;
  color: var(--color-text);
}

.join-reason {
  color: var(--color-text-secondary);
  font-size: 0.85rem;
}

.cache-badge {
  display: inline-block;
  padding: 0.25rem 0.75rem;
  border-radius: 4px;
  font-size: 0.8rem;
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.cache-badge.cached {
  background: rgba(16, 185, 129, 0.1);
  color: #10b981;
}
</style>
