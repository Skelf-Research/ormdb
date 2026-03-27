const BASE_URL = ''

interface SessionResponse {
  success: boolean
  session: {
    id: string
    age_secs?: number
  }
}

interface SchemaResponse {
  success: boolean
  schema: {
    version: number
    entities: any[]
  }
}

interface QueryResponse {
  success: boolean
  data: any
}

// Metrics types
export interface MetricsResponse {
  success: boolean
  metrics: MetricsData
}

export interface MetricsData {
  uptime_secs: number
  queries: QueryMetrics
  mutations: MutationMetrics
  cache: CacheMetrics
  storage: StorageMetrics
}

export interface QueryMetrics {
  total_count: number
  avg_duration_us: number
  p50_duration_us: number
  p99_duration_us: number
  max_duration_us: number
  by_entity: EntityQueryCount[]
}

export interface EntityQueryCount {
  entity: string
  count: number
}

export interface MutationMetrics {
  total_count: number
  inserts: number
  updates: number
  deletes: number
  upserts: number
  rows_affected: number
}

export interface CacheMetrics {
  hits: number
  misses: number
  hit_rate: number
  size: number
  capacity: number
  evictions: number
}

export interface StorageMetrics {
  entity_counts: EntityCount[]
  total_entities: number
  size_bytes?: number
}

export interface EntityCount {
  entity: string
  count: number
}

// Explain types
export interface ExplainResponse {
  success: boolean
  explain: ExplainResult
}

export interface ExplainResult {
  plan: string
  cost: CostBreakdown
  joins: JoinInfo[]
  plan_cached: boolean
}

export interface CostBreakdown {
  total_cost: number
  estimated_rows: number
  io_cost: number
  cpu_cost: number
}

export interface JoinInfo {
  path: string
  strategy: string
  reason: string
}

// Replication types
export interface ReplicationStatusResponse {
  success: boolean
  replication: ReplicationStatus
}

export interface ReplicationStatus {
  role: 'primary' | 'replica' | 'standalone'
  primary_addr?: string
  current_lsn: number
  lag_entries: number
  lag_ms: number
}

// Schema apply types
export interface SchemaAppliedResponse {
  success: boolean
  version: number
}

// Compaction types
export interface CompactionResponse {
  success: boolean
  compaction: CompactionResult
}

export interface CompactionResult {
  versions_removed: number
  tombstones_removed: number
  bytes_reclaimed: number
  duration_ms: number
  entities_processed: number
  errors: number
  did_cleanup: boolean
}

async function request<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const response = await fetch(`${BASE_URL}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
  })

  const data = await response.json()

  if (!response.ok || !data.success) {
    throw new Error(data.error?.message || 'Request failed')
  }

  return data
}

export const api = {
  // Session management
  async createSession(demo: boolean = false): Promise<SessionResponse> {
    const url = demo ? '/api/session?demo=true' : '/api/session'
    return request(url, { method: 'POST' })
  },

  async getSession(id: string): Promise<SessionResponse> {
    return request(`/api/session/${id}`)
  },

  async deleteSession(id: string): Promise<{ success: boolean }> {
    return request(`/api/session/${id}`, { method: 'DELETE' })
  },

  // Schema
  async getSchema(sessionId: string): Promise<SchemaResponse> {
    return request(`/api/session/${sessionId}/schema`)
  },

  async listEntities(sessionId: string): Promise<{ success: boolean; entities: string[] }> {
    return request(`/api/session/${sessionId}/schema/entities`)
  },

  // Queries
  async executeQuery(sessionId: string, query: object): Promise<QueryResponse> {
    return request(`/api/session/${sessionId}/query`, {
      method: 'POST',
      body: JSON.stringify(query),
    })
  },

  async executeRawQuery(sessionId: string, query: string): Promise<QueryResponse> {
    return request(`/api/session/${sessionId}/query/raw`, {
      method: 'POST',
      body: JSON.stringify({ query }),
    })
  },

  // Mutations
  async executeMutation(sessionId: string, mutation: object): Promise<QueryResponse> {
    return request(`/api/session/${sessionId}/mutate`, {
      method: 'POST',
      body: JSON.stringify(mutation),
    })
  },

  // Health
  async health(): Promise<{ status: string; version: string }> {
    return request('/health')
  },

  // Metrics
  async getMetrics(sessionId: string): Promise<MetricsResponse> {
    return request(`/api/session/${sessionId}/metrics`)
  },

  // Explain query
  async explainQuery(sessionId: string, query: string): Promise<ExplainResponse> {
    return request(`/api/session/${sessionId}/explain`, {
      method: 'POST',
      body: JSON.stringify({ query }),
    })
  },

  // Replication status
  async getReplicationStatus(sessionId: string): Promise<ReplicationStatusResponse> {
    return request(`/api/session/${sessionId}/replication`)
  },

  // Apply schema
  async applySchema(sessionId: string, schema: string): Promise<SchemaAppliedResponse> {
    return request(`/api/session/${sessionId}/schema/apply`, {
      method: 'POST',
      body: JSON.stringify({ schema }),
    })
  },

  // Trigger storage compaction
  async compact(sessionId: string): Promise<CompactionResponse> {
    return request(`/api/session/${sessionId}/compact`, {
      method: 'POST',
    })
  },
}

// WebSocket helper for terminal
export function createTerminalWebSocket(sessionId: string): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const host = window.location.host
  return new WebSocket(`${protocol}//${host}/ws/terminal/${sessionId}`)
}

// CSV export helper
export function exportToCsv(data: Record<string, any>[], filename: string = 'export.csv'): void {
  if (!data || data.length === 0) {
    console.warn('No data to export')
    return
  }

  // Get headers from the first object
  const headers = Object.keys(data[0])

  // Build CSV content
  const csvRows: string[] = []

  // Add header row
  csvRows.push(headers.map(h => escapeCsvValue(h)).join(','))

  // Add data rows
  for (const row of data) {
    const values = headers.map(h => {
      const value = row[h]
      return escapeCsvValue(formatCsvValue(value))
    })
    csvRows.push(values.join(','))
  }

  const csvContent = csvRows.join('\n')

  // Create download
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
  const link = document.createElement('a')
  const url = URL.createObjectURL(blob)
  link.setAttribute('href', url)
  link.setAttribute('download', filename)
  link.style.visibility = 'hidden'
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

function escapeCsvValue(value: string): string {
  // If value contains comma, newline, or quote, wrap in quotes and escape quotes
  if (value.includes(',') || value.includes('\n') || value.includes('"')) {
    return '"' + value.replace(/"/g, '""') + '"'
  }
  return value
}

function formatCsvValue(value: any): string {
  if (value === null || value === undefined) {
    return ''
  }
  if (typeof value === 'object') {
    return JSON.stringify(value)
  }
  return String(value)
}
