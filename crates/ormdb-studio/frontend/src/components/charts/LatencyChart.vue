<script setup lang="ts">
import { computed } from 'vue'
import { Line } from 'vue-chartjs'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler
} from 'chart.js'
import { useMetricsStore } from '../../stores/metrics'

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler
)

const metricsStore = useMetricsStore()

const chartData = computed(() => {
  const history = metricsStore.history
  const labels = history.map((_, i) => {
    const secsAgo = (history.length - 1 - i) * 2
    return secsAgo === 0 ? 'now' : `-${secsAgo}s`
  })

  return {
    labels,
    datasets: [
      {
        label: 'Avg Latency (ms)',
        data: history.map(h => h.avgLatencyMs),
        borderColor: '#10b981',
        backgroundColor: 'rgba(16, 185, 129, 0.1)',
        fill: true,
        tension: 0.4,
        pointRadius: 0,
        pointHoverRadius: 4,
      },
    ],
  }
})

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  interaction: {
    mode: 'index' as const,
    intersect: false,
  },
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
      callbacks: {
        label: (context: any) => {
          return `${context.dataset.label}: ${context.parsed.y.toFixed(2)}ms`
        },
      },
    },
  },
  scales: {
    x: {
      grid: { color: '#1e293b' },
      ticks: { color: '#64748b', font: { size: 10 } },
    },
    y: {
      beginAtZero: true,
      grid: { color: '#1e293b' },
      ticks: {
        color: '#64748b',
        font: { size: 10 },
        callback: (value: any) => `${value}ms`,
      },
    },
  },
}
</script>

<template>
  <div class="chart-container">
    <h3 class="chart-title">Query Latency</h3>
    <div class="chart-wrapper">
      <Line v-if="metricsStore.history.length > 1" :data="chartData" :options="chartOptions" />
      <div v-else class="chart-empty">Collecting data...</div>
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
