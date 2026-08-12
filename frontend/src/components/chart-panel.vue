<template>
  <div class="chart-panel">
    <div v-if="loading" class="chart-state"><span class="spinner" /> Building chart…</div>
    <div v-else-if="error || drawError" class="chart-state error-state">{{ error || drawError }}</div>
    <div v-else-if="!figure" class="chart-state">{{ emptyMessage }}</div>
    <div ref="chart" class="plot" />
  </div>
</template>

<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  figure: { type: Object, default: null },
  loading: Boolean,
  error: { type: String, default: '' },
  emptyMessage: { type: String, default: 'Choose data to build this chart.' }
})

const chart = ref(null)
const drawError = ref('')
let plotly
let themeObserver

function themeColor(name, fallback) {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return value || fallback
}

async function draw() {
  if (!props.figure || !chart.value) return
  drawError.value = ''
  try {
    await nextTick()
    plotly ||= (await import('plotly.js-dist-min')).default
    const figureLayout = { ...(props.figure.layout || {}) }
    delete figureLayout.width
    const layout = {
      ...figureLayout,
      autosize: true,
      paper_bgcolor: 'transparent',
      plot_bgcolor: 'transparent',
      font: {
        color: themeColor('--chart-text', '#9ba9c4'),
        family: '-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif'
      },
      margin: { l: 56, r: 24, t: 32, b: 48, ...figureLayout.margin },
      legend: { bgcolor: 'rgba(0, 0, 0, 0)', ...figureLayout.legend },
      xaxis: {
        gridcolor: themeColor('--chart-grid', '#1c2940'),
        zerolinecolor: themeColor('--chart-zero', '#263550'),
        ...figureLayout.xaxis
      },
      yaxis: {
        gridcolor: themeColor('--chart-grid', '#1c2940'),
        zerolinecolor: themeColor('--chart-zero', '#263550'),
        ...figureLayout.yaxis
      }
    }
    await plotly.react(chart.value, props.figure.data || [], layout, {
      displaylogo: false,
      responsive: true,
      scrollZoom: true
    })
  } catch (error) {
    drawError.value = error instanceof Error ? error.message : 'The chart could not be drawn.'
  }
}

// Plotly mutates the layout and trace objects it receives. A deep watcher would
// observe those internal mutations and recursively redraw the same figure.
// API responses replace the figure object, so a shallow watch is sufficient.
watch(() => props.figure, draw)
onMounted(() => {
  themeObserver = new MutationObserver(draw)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
  draw()
})
onBeforeUnmount(() => {
  themeObserver?.disconnect()
  if (chart.value) plotly?.purge(chart.value)
})
</script>
