<script setup lang="ts">
import { ref, computed } from 'vue'
import { exportToCsv } from '../api/client'

const props = defineProps<{
  data: Record<string, any>[]
  loading?: boolean
}>()

// Pagination state
const currentPage = ref(1)
const pageSize = ref(25)
const pageSizeOptions = [10, 25, 50, 100]

// Sorting state
const sortColumn = ref<string | null>(null)
const sortDirection = ref<'asc' | 'desc'>('asc')

// Search filter
const searchQuery = ref('')

// Computed columns from data
const columns = computed(() => {
  if (!props.data || props.data.length === 0) return []
  return Object.keys(props.data[0])
})

// Filtered data
const filteredData = computed(() => {
  if (!props.data) return []
  if (!searchQuery.value.trim()) return props.data

  const query = searchQuery.value.toLowerCase()
  return props.data.filter(row =>
    columns.value.some(col => {
      const value = row[col]
      if (value === null || value === undefined) return false
      return String(value).toLowerCase().includes(query)
    })
  )
})

// Sorted data
const sortedData = computed(() => {
  if (!sortColumn.value) return filteredData.value

  const col = sortColumn.value
  const dir = sortDirection.value === 'asc' ? 1 : -1

  return [...filteredData.value].sort((a, b) => {
    const aVal = a[col]
    const bVal = b[col]

    // Handle nulls
    if (aVal === null || aVal === undefined) return 1
    if (bVal === null || bVal === undefined) return -1

    // Compare based on type
    if (typeof aVal === 'number' && typeof bVal === 'number') {
      return (aVal - bVal) * dir
    }

    return String(aVal).localeCompare(String(bVal)) * dir
  })
})

// Paginated data
const paginatedData = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  return sortedData.value.slice(start, end)
})

// Total pages
const totalPages = computed(() => {
  return Math.ceil(sortedData.value.length / pageSize.value)
})

// Page range for display
const pageRange = computed(() => {
  const total = totalPages.value
  const current = currentPage.value
  const range: (number | string)[] = []

  if (total <= 7) {
    for (let i = 1; i <= total; i++) range.push(i)
  } else {
    if (current <= 4) {
      for (let i = 1; i <= 5; i++) range.push(i)
      range.push('...')
      range.push(total)
    } else if (current >= total - 3) {
      range.push(1)
      range.push('...')
      for (let i = total - 4; i <= total; i++) range.push(i)
    } else {
      range.push(1)
      range.push('...')
      for (let i = current - 1; i <= current + 1; i++) range.push(i)
      range.push('...')
      range.push(total)
    }
  }

  return range
})

// Sort handler
function handleSort(column: string) {
  if (sortColumn.value === column) {
    sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortColumn.value = column
    sortDirection.value = 'asc'
  }
  currentPage.value = 1 // Reset to first page
}

// Page change handler
function goToPage(page: number | string) {
  if (typeof page === 'number' && page >= 1 && page <= totalPages.value) {
    currentPage.value = page
  }
}

// Page size change handler
function changePageSize(newSize: number) {
  pageSize.value = newSize
  currentPage.value = 1
}

// Export handler
function handleExport() {
  if (!props.data || props.data.length === 0) return
  exportToCsv(sortedData.value, `query-results-${Date.now()}.csv`)
}

// Format cell value for display
function formatValue(value: any): string {
  if (value === null) return 'null'
  if (value === undefined) return ''
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}
</script>

<template>
  <div class="results-table">
    <!-- Toolbar -->
    <div class="table-toolbar">
      <div class="toolbar-left">
        <input
          v-model="searchQuery"
          type="text"
          class="search-input"
          placeholder="Filter results..."
        />
        <span class="result-count">
          {{ sortedData.length }} row{{ sortedData.length !== 1 ? 's' : '' }}
        </span>
      </div>
      <div class="toolbar-right">
        <select
          :value="pageSize"
          @change="changePageSize(Number(($event.target as HTMLSelectElement).value))"
          class="page-size-select"
        >
          <option v-for="size in pageSizeOptions" :key="size" :value="size">
            {{ size }} per page
          </option>
        </select>
        <button class="export-btn" @click="handleExport" :disabled="!data || data.length === 0">
          Export CSV
        </button>
      </div>
    </div>

    <!-- Table -->
    <div class="table-wrapper">
      <table v-if="data && data.length > 0">
        <thead>
          <tr>
            <th
              v-for="col in columns"
              :key="col"
              @click="handleSort(col)"
              class="sortable"
            >
              <span class="th-content">
                {{ col }}
                <span v-if="sortColumn === col" class="sort-indicator">
                  {{ sortDirection === 'asc' ? ' ▲' : ' ▼' }}
                </span>
              </span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, idx) in paginatedData" :key="idx">
            <td v-for="col in columns" :key="col" :title="formatValue(row[col])">
              {{ formatValue(row[col]) }}
            </td>
          </tr>
        </tbody>
      </table>

      <div v-else-if="loading" class="empty-state">
        Loading...
      </div>

      <div v-else class="empty-state">
        No data to display
      </div>
    </div>

    <!-- Pagination -->
    <div v-if="totalPages > 1" class="pagination">
      <button
        class="page-btn"
        :disabled="currentPage === 1"
        @click="goToPage(currentPage - 1)"
      >
        Previous
      </button>

      <div class="page-numbers">
        <button
          v-for="page in pageRange"
          :key="page"
          :class="['page-num', { active: page === currentPage, ellipsis: page === '...' }]"
          :disabled="page === '...'"
          @click="goToPage(page)"
        >
          {{ page }}
        </button>
      </div>

      <button
        class="page-btn"
        :disabled="currentPage === totalPages"
        @click="goToPage(currentPage + 1)"
      >
        Next
      </button>
    </div>
  </div>
</template>

<style scoped>
.results-table {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-bg);
}

.table-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1rem;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-bg-tertiary);
  gap: 1rem;
}

.toolbar-left,
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.search-input {
  padding: 0.4rem 0.75rem;
  background: var(--color-bg);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.85rem;
  min-width: 200px;
}

.search-input:focus {
  outline: none;
  border-color: var(--color-primary);
}

.result-count {
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.page-size-select {
  padding: 0.4rem 0.5rem;
  background: var(--color-bg);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.8rem;
}

.export-btn {
  padding: 0.4rem 0.75rem;
  background: var(--color-primary);
  color: var(--color-bg);
  border: none;
  border-radius: 4px;
  font-size: 0.8rem;
  cursor: pointer;
  transition: opacity 0.2s;
}

.export-btn:hover:not(:disabled) {
  opacity: 0.9;
}

.export-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.table-wrapper {
  flex: 1;
  overflow: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
}

thead {
  position: sticky;
  top: 0;
  z-index: 1;
}

th {
  background: var(--color-bg-secondary);
  padding: 0.6rem 0.75rem;
  text-align: left;
  font-weight: 500;
  color: var(--color-text-secondary);
  border-bottom: 1px solid var(--color-bg-tertiary);
  white-space: nowrap;
}

th.sortable {
  cursor: pointer;
  user-select: none;
}

th.sortable:hover {
  color: var(--color-text);
}

.th-content {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.sort-indicator {
  font-size: 0.7rem;
  color: var(--color-primary);
}

td {
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
  color: var(--color-text);
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

tr:hover td {
  background: var(--color-bg-secondary);
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--color-text-secondary);
}

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.75rem;
  background: var(--color-bg-secondary);
  border-top: 1px solid var(--color-bg-tertiary);
  gap: 0.5rem;
}

.page-btn {
  padding: 0.4rem 0.75rem;
  background: var(--color-bg);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.page-btn:hover:not(:disabled) {
  background: var(--color-bg-tertiary);
}

.page-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.page-numbers {
  display: flex;
  gap: 0.25rem;
}

.page-num {
  min-width: 32px;
  padding: 0.4rem 0.5rem;
  background: var(--color-bg);
  border: 1px solid var(--color-bg-tertiary);
  border-radius: 4px;
  color: var(--color-text);
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.page-num:hover:not(:disabled):not(.ellipsis) {
  background: var(--color-bg-tertiary);
}

.page-num.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-bg);
}

.page-num.ellipsis {
  background: transparent;
  border-color: transparent;
  cursor: default;
}
</style>
