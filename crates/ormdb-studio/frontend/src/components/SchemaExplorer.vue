<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useSchemaStore } from '../stores/schema'
import { useSessionStore } from '../stores/session'

const schema = useSchemaStore()
const session = useSessionStore()

onMounted(() => {
  if (session.id) {
    schema.fetchSchema()
  }
})

watch(() => session.id, (newId) => {
  if (newId) {
    schema.fetchSchema()
  }
})
</script>

<template>
  <div class="schema-explorer">
    <div class="explorer-header">
      <span>Schema Explorer</span>
      <button class="refresh-btn" @click="schema.fetchSchema" :disabled="schema.loading">
        ⟳
      </button>
    </div>

    <div class="explorer-content">
      <!-- Loading -->
      <div v-if="schema.loading" class="loading">
        Loading schema...
      </div>

      <!-- Error -->
      <div v-else-if="schema.error" class="error">
        {{ schema.error }}
      </div>

      <!-- Empty -->
      <div v-else-if="schema.entities.length === 0" class="empty">
        <p>No entities defined</p>
        <p class="hint">Create entities using the terminal:</p>
        <code>.entity User { name: String, email: String }</code>
        <p class="hint types">Types: String, Int, Int64, Float, Bool, Uuid, Timestamp, Bytes</p>
        <p class="hint types">Optional: field: Type? &nbsp; Array: field: Type[]</p>
      </div>

      <!-- Entity List -->
      <div v-else class="entity-list">
        <div
          v-for="entity in schema.entities"
          :key="entity.name"
          class="entity-item"
        >
          <div class="entity-header">
            <span class="entity-icon">◇</span>
            <span class="entity-name">{{ entity.name }}</span>
          </div>
          <div class="entity-fields">
            <div
              v-for="field in entity.fields"
              :key="field.name"
              class="field-item"
            >
              <span class="field-name">{{ field.name }}</span>
              <span class="field-type">{{ field.type }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.schema-explorer {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.explorer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--color-bg-tertiary);
  font-size: 0.9rem;
  font-weight: 500;
}

.refresh-btn {
  padding: 0.25rem 0.5rem;
  background: transparent;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 1rem;
}

.refresh-btn:hover:not(:disabled) {
  color: var(--color-primary);
}

.explorer-content {
  flex: 1;
  overflow-y: auto;
  padding: 0.5rem;
}

.loading,
.error,
.empty {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 0.85rem;
}

.error {
  color: var(--color-error);
}

.empty .hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
}

.empty .hint.types {
  margin-top: 0.25rem;
  font-size: 0.7rem;
  color: var(--color-text-muted, #666);
}

.empty code {
  display: block;
  margin-top: 0.5rem;
  padding: 0.5rem;
  background: var(--color-bg);
  border-radius: 4px;
  font-size: 0.75rem;
  text-align: left;
  color: var(--color-primary);
}

.entity-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.entity-item {
  background: var(--color-bg);
  border-radius: 6px;
  overflow: hidden;
}

.entity-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  cursor: pointer;
}

.entity-header:hover {
  background: var(--color-bg-tertiary);
}

.entity-icon {
  color: var(--color-primary);
}

.entity-name {
  font-weight: 500;
  font-size: 0.9rem;
}

.entity-fields {
  padding: 0 0.75rem 0.5rem;
}

.field-item {
  display: flex;
  justify-content: space-between;
  padding: 0.25rem 0;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.field-name {
  color: var(--color-text);
}

.field-type {
  color: var(--color-accent);
  font-family: 'Fira Code', monospace;
  font-size: 0.75rem;
}
</style>
