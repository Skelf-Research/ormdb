<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSessionStore } from './stores/session'
import Terminal from './components/Terminal.vue'
import QueryEditor from './components/QueryEditor.vue'
import SchemaExplorer from './components/SchemaExplorer.vue'

const session = useSessionStore()
const activeTab = ref<'terminal' | 'editor' | 'builder'>('terminal')

onMounted(async () => {
  await session.createSession()
})
</script>

<template>
  <div class="app-container">
    <!-- Header -->
    <header class="header">
      <div class="logo">
        <span class="logo-icon">◈</span>
        <span class="logo-text">ORMDB Studio</span>
      </div>
      <div class="session-info" v-if="session.id">
        <span class="session-badge">Session: {{ session.id.slice(0, 8) }}...</span>
      </div>
    </header>

    <!-- Main Content -->
    <div class="main-content">
      <!-- Sidebar - Schema Explorer -->
      <aside class="sidebar">
        <SchemaExplorer />
      </aside>

      <!-- Main Panel -->
      <main class="main-panel">
        <!-- Tab Bar -->
        <div class="tab-bar">
          <button
            :class="['tab', { active: activeTab === 'terminal' }]"
            @click="activeTab = 'terminal'"
          >
            Terminal
          </button>
          <button
            :class="['tab', { active: activeTab === 'editor' }]"
            @click="activeTab = 'editor'"
          >
            Query Editor
          </button>
          <button
            :class="['tab', { active: activeTab === 'builder' }]"
            @click="activeTab = 'builder'"
          >
            Query Builder
          </button>
        </div>

        <!-- Tab Content -->
        <div class="tab-content">
          <Terminal v-if="activeTab === 'terminal'" />
          <QueryEditor v-else-if="activeTab === 'editor'" />
          <div v-else-if="activeTab === 'builder'" class="coming-soon">
            <h2>Visual Query Builder</h2>
            <p>Drag and drop query builder coming soon...</p>
          </div>
        </div>
      </main>
    </div>

    <!-- Status Bar -->
    <footer class="status-bar">
      <div class="status-item">
        <span :class="['status-dot', session.connected ? 'connected' : 'disconnected']"></span>
        {{ session.connected ? 'Connected' : 'Connecting...' }}
      </div>
      <div class="status-item" v-if="session.id">
        Session: {{ session.id }}
      </div>
    </footer>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--color-bg);
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.logo {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.logo-icon {
  font-size: 1.5rem;
  color: var(--color-primary);
}

.logo-text {
  font-size: 1.25rem;
  font-weight: 600;
  background: linear-gradient(90deg, var(--color-primary), var(--color-accent));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.session-badge {
  padding: 0.25rem 0.75rem;
  background: var(--color-bg-tertiary);
  border-radius: 1rem;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.main-content {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.sidebar {
  width: 250px;
  background: var(--color-bg-secondary);
  border-right: 1px solid var(--color-bg-tertiary);
  overflow-y: auto;
}

.main-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tab-bar {
  display: flex;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-bg-tertiary);
}

.tab {
  padding: 0.75rem 1.5rem;
  background: none;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.tab:hover {
  color: var(--color-text);
  background: var(--color-bg-tertiary);
}

.tab.active {
  color: var(--color-primary);
  border-bottom: 2px solid var(--color-primary);
}

.tab-content {
  flex: 1;
  overflow: hidden;
}

.coming-soon {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
}

.coming-soon h2 {
  font-size: 1.5rem;
  margin-bottom: 0.5rem;
  color: var(--color-text);
}

.status-bar {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.5rem 1rem;
  background: var(--color-bg-secondary);
  border-top: 1px solid var(--color-bg-tertiary);
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.status-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.status-dot.connected {
  background: var(--color-success);
}

.status-dot.disconnected {
  background: var(--color-warning);
}
</style>
