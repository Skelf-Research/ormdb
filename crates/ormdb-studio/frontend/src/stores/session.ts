import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api/client'

const SESSION_STORAGE_KEY = 'ormdb-studio-session-id'

export const useSessionStore = defineStore('session', () => {
  const id = ref<string | null>(null)
  const connected = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function createSession() {
    loading.value = true
    error.value = null

    // Try to restore existing session from localStorage
    const savedSessionId = localStorage.getItem(SESSION_STORAGE_KEY)
    if (savedSessionId) {
      try {
        // Check if session still exists on server
        await api.getSession(savedSessionId)
        id.value = savedSessionId
        connected.value = true
        loading.value = false
        return
      } catch {
        // Session expired or doesn't exist, create new one
        localStorage.removeItem(SESSION_STORAGE_KEY)
      }
    }

    try {
      const response = await api.createSession()
      id.value = response.session.id
      connected.value = true
      // Save session ID to localStorage
      localStorage.setItem(SESSION_STORAGE_KEY, response.session.id)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to create session'
      connected.value = false
    } finally {
      loading.value = false
    }
  }

  async function deleteSession() {
    if (!id.value) return

    try {
      await api.deleteSession(id.value)
    } finally {
      localStorage.removeItem(SESSION_STORAGE_KEY)
      id.value = null
      connected.value = false
    }
  }

  return {
    id,
    connected,
    loading,
    error,
    createSession,
    deleteSession,
  }
})
