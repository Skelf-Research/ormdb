<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSessionStore } from './stores/session'
import Terminal from './components/Terminal.vue'
import QueryEditor from './components/QueryEditor.vue'
import SchemaExplorer from './components/SchemaExplorer.vue'
import MetricsDashboard from './components/MetricsDashboard.vue'
import ClusterStatusPanel from './components/ClusterStatusPanel.vue'
import ConnectionSettings from './components/ConnectionSettings.vue'

const session = useSessionStore()
const activeTab = ref<'terminal' | 'editor' | 'metrics' | 'cluster'>('terminal')
const showSettings = ref(false)

onMounted(async () => {
  await session.createSession()
})

async function startDemoMode() {
  // Delete current session if any
  if (session.id) {
    await session.deleteSession()
  }
  // Create new demo session
  await session.createSession(true)
  // Switch to query editor tab to show sample queries
  activeTab.value = 'editor'
}
</script>

<template>
  <div class="app-container">
    <!-- Header -->
    <header class="header">
      <div class="logo">
        <span class="logo-icon">◈</span>
        <span class="logo-text">ORMDB Studio</span>
      </div>
      <div class="header-right">
        <button v-if="!session.isDemo" class="demo-btn" @click="startDemoMode">
          Try Demo Mode
        </button>
        <span v-else class="demo-badge">Demo Mode</span>
        <div class="session-info" v-if="session.id">
          <span class="session-badge">Session: {{ session.id.slice(0, 8) }}...</span>
        </div>
        <button class="settings-btn" @click="showSettings = true" title="Settings">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
          </svg>
        </button>
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
            :class="['tab', { active: activeTab === 'metrics' }]"
            @click="activeTab = 'metrics'"
          >
            Metrics
          </button>
          <button
            :class="['tab', { active: activeTab === 'cluster' }]"
            @click="activeTab = 'cluster'"
          >
            Cluster
          </button>
        </div>

        <!-- Tab Content -->
        <div class="tab-content">
          <Terminal v-if="activeTab === 'terminal'" />
          <QueryEditor v-else-if="activeTab === 'editor'" />
          <MetricsDashboard v-else-if="activeTab === 'metrics'" />
          <ClusterStatusPanel v-else-if="activeTab === 'cluster'" />
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

    <!-- Settings Modal -->
    <ConnectionSettings v-if="showSettings" @close="showSettings = false" />
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

.header-right {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.session-badge {
  padding: 0.25rem 0.75rem;
  background: var(--color-bg-tertiary);
  border-radius: 1rem;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.demo-btn {
  padding: 0.5rem 1rem;
  background: linear-gradient(135deg, var(--color-primary), var(--color-accent));
  border: none;
  border-radius: 6px;
  color: var(--color-bg);
  font-size: 0.85rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s, transform 0.2s;
}

.demo-btn:hover {
  opacity: 0.9;
  transform: translateY(-1px);
}

.demo-badge {
  padding: 0.25rem 0.75rem;
  background: linear-gradient(135deg, var(--color-primary), var(--color-accent));
  border-radius: 1rem;
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--color-bg);
}

.settings-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  background: var(--color-bg-tertiary);
  border: none;
  border-radius: 8px;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
}

.settings-btn:hover {
  background: var(--color-bg);
  color: var(--color-text);
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
