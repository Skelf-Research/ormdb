import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api/client'

export const useSessionStore = defineStore('session', () => {
  const id = ref<string | null>(null)
  const connected = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function createSession() {
    loading.value = true
    error.value = null

    try {
      const response = await api.createSession()
      id.value = response.session.id
      connected.value = true
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
