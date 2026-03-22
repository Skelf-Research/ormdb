const BASE_URL = ''

interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: {
    code: string
    message: string
  }
}

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
  async createSession(): Promise<SessionResponse> {
    return request('/api/session', { method: 'POST' })
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
}

// WebSocket helper for terminal
export function createTerminalWebSocket(sessionId: string): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const host = window.location.host
  return new WebSocket(`${protocol}//${host}/ws/terminal/${sessionId}`)
}
