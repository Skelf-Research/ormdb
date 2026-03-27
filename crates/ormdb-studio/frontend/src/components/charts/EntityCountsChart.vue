<script setup lang="ts">
import { computed } from 'vue'
import { Bar } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  Title,
  Tooltip,
  Legend
} from 'chart.js'
import { useMetricsStore } from '../../stores/metrics'

ChartJS.register(
  CategoryScale,
  LinearScale,
  BarElement,
  Title,
  Tooltip,
  Legend
)

const metricsStore = useMetricsStore()

const chartData = computed(() => {
  const entityCounts = metricsStore.currentMetrics?.storage.entity_counts ?? []
  const queriesByEntity = metricsStore.currentMetrics?.queries.by_entity ?? []

  // Combine entity names from both sources
  const allEntities = new Set([
    ...entityCounts.map(e => e.entity),
    ...queriesByEntity.map(e => e.entity),
  ])

  const labels = Array.from(allEntities)
  const entityCountMap = new Map(entityCounts.map(e => [e.entity, e.count]))
  const queryCountMap = new Map(queriesByEntity.map(e => [e.entity, e.count]))

  return {
    labels,
    datasets: [
      {
        label: 'Rows',
        data: labels.map(l => entityCountMap.get(l) ?? 0),
        backgroundColor: 'rgba(59, 130, 246, 0.8)',
        borderColor: '#3b82f6',
        borderWidth: 1,
        borderRadius: 4,
      },
      {
        label: 'Queries',
        data: labels.map(l => queryCountMap.get(l) ?? 0),
        backgroundColor: 'rgba(139, 92, 246, 0.8)',
        borderColor: '#8b5cf6',
        borderWidth: 1,
        borderRadius: 4,
      },
    ],
  }
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  indexAxis: 'y' as const,
  plugins: {
    legend: {
      position: 'top' as const,
      labels: {
        color: '#94a3b8',
        boxWidth: 12,
        padding: 8,
        font: { size: 11 },
      },
    },
    tooltip: {
      backgroundColor: '#1e293b',
      titleColor: '#f1f5f9',
      bodyColor: '#cbd5e1',
      borderColor: '#334155',
      borderWidth: 1,
    },
  },
  scales: {
    x: {
      beginAtZero: true,
      grid: { color: '#1e293b' },
      ticks: { color: '#64748b', font: { size: 10 } },
    },
    y: {
      grid: { display: false },
      ticks: { color: '#94a3b8', font: { size: 11 } },
    },
  },
}

const hasData = computed(() => {
  const metrics = metricsStore.currentMetrics
  if (!metrics) return false
  return (
    metrics.storage.entity_counts.length > 0 ||
    metrics.queries.by_entity.length > 0
  )
})
</script>

<template>
  <div class="chart-container">
    <h3 class="chart-title">Entity Statistics</h3>
    <div class="chart-wrapper">
      <Bar v-if="hasData" :data="chartData" :options="chartOptions" />
      <div v-else class="chart-empty">No entities yet</div>
    </div>
  </div>
</template>

<style scoped>
.chart-container {
  background: var(--color-bg-secondary);
  border-radius: 8px;
  padding: 1rem;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.chart-title {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text);
  margin-bottom: 0.75rem;
}

.chart-wrapper {
  flex: 1;
  min-height: 0;
  position: relative;
}

.chart-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
  font-size: 0.85rem;
}
</style>
