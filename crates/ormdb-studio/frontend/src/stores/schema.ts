import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '../api/client'
import { useSessionStore } from './session'

export interface Entity {
  name: string
  fields: Field[]
  relations: Relation[]
}

export interface Field {
  name: string
  type: string
  nullable: boolean
  primaryKey: boolean
}

export interface Relation {
  name: string
  target: string
  type: 'one-to-one' | 'one-to-many' | 'many-to-many'
}

export const useSchemaStore = defineStore('schema', () => {
  const entities = ref<Entity[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const entityNames = computed(() => entities.value.map((e) => e.name))

  async function fetchSchema() {
    const session = useSessionStore()
    if (!session.id) return

    loading.value = true
    error.value = null

    try {
      const response = await api.getSchema(session.id)
      entities.value = response.schema.entities || []
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch schema'
    } finally {
      loading.value = false
    }
  }

  function getEntity(name: string): Entity | undefined {
    return entities.value.find((e) => e.name === name)
  }

  function getEntityFields(name: string): Field[] {
    return getEntity(name)?.fields || []
  }

  return {
    entities,
    entityNames,
    loading,
    error,
    fetchSchema,
    getEntity,
    getEntityFields,
  }
})
