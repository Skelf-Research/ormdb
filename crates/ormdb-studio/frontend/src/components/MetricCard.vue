<script setup lang="ts">
defineProps<{
  title: string
  value: string | number
  subtitle?: string
  trend?: 'up' | 'down' | 'neutral'
  color?: 'primary' | 'success' | 'warning' | 'error'
}>()
</script>

<template>
  <div :class="['metric-card', `metric-card--${color || 'primary'}`]">
    <div class="metric-header">
      <span class="metric-title">{{ title }}</span>
      <span v-if="trend" :class="['metric-trend', `metric-trend--${trend}`]">
        <span v-if="trend === 'up'">+</span>
        <span v-else-if="trend === 'down'">-</span>
      </span>
    </div>
    <div class="metric-value">{{ value }}</div>
    <div v-if="subtitle" class="metric-subtitle">{{ subtitle }}</div>
  </div>
</template>

<style scoped>
.metric-card {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem 1.25rem;
  border-left: 3px solid var(--color-primary);
}

.metric-card--primary {
  border-left-color: var(--color-primary);
}

.metric-card--success {
  border-left-color: var(--color-success, #10b981);
}

.metric-card--warning {
  border-left-color: var(--color-warning, #f59e0b);
}

.metric-card--error {
  border-left-color: var(--color-error, #ef4444);
}

.metric-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.metric-title {
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.metric-trend {
  font-size: 0.75rem;
  padding: 0.125rem 0.375rem;
  border-radius: 4px;
}

.metric-trend--up {
  background: rgba(16, 185, 129, 0.1);
  color: #10b981;
}

.metric-trend--down {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
}

.metric-trend--neutral {
  background: var(--color-bg-tertiary);
  color: var(--color-text-secondary);
}

.metric-value {
  font-size: 1.75rem;
  font-weight: 600;
  color: var(--color-text);
  font-variant-numeric: tabular-nums;
}

.metric-subtitle {
  font-size: 0.75rem;
  color: var(--color-text-secondary);
  margin-top: 0.25rem;
}
</style>
