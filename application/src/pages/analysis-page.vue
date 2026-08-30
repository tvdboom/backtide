<template>
  <div class="page">
    <section class="page-intro"><div><h2>Analyze market data</h2><p>Compare key metrics and explore prices, returns, correlation, seasonality, volatility, volume and dividends.</p></div></section>
    <section class="panel analysis-controls">
      <div class="field-label"><span>Symbols</span><SearchSelect v-model="form.symbols" :options="symbols" :descriptions="names" :logos="logos" :selected-logos="selectedLogos" :option-details="symbolDetails" clearable clear-label="symbols" label="Analysis symbols" placeholder="Search stored instruments…" /></div>
      <label>Interval<select v-model="form.interval"><option v-for="item in availableIntervals" :key="item">{{ item }}</option></select></label>
      <label>Price<select v-model="form.price_col"><option value="open">Open</option><option value="high">High</option><option value="low">Low</option><option value="close">Close</option><option value="adj_close">Adjusted close</option></select></label>
      <label v-if="form.plot === 'volatility'">Window<input v-model.number="form.window" type="number" min="2" /></label>
    </section>
    <section class="panel chart-workspace">
      <div class="chart-tabs" role="tablist">
        <button v-for="item in plots" :key="item.id" :class="{ active: form.plot === item.id }" type="button" @click="selectPlot(item.id)"><component :is="item.icon" :size="16"/>{{ item.label }}</button>
      </div>
      <div class="chart-title"><div><h3>{{ current.label }}</h3><p>{{ current.description }}</p></div></div>
      <div v-if="form.plot === 'metrics'" class="analysis-metrics">
        <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Calculating metrics…</p></div>
        <div v-else-if="error" class="empty-state error-state"><p>{{ error }}</p></div>
        <div v-else-if="!metrics.length" class="empty-state"><p>No metrics are available for the selected data.</p></div>
        <div v-else class="data-table-wrap">
          <table class="data-table analysis-metrics-table">
            <thead><tr><th>Stock</th><th v-for="column in metricColumns" :key="column.key" class="number">{{ column.label }}</th></tr></thead>
            <tbody>
              <tr v-for="row in metrics" :key="row.symbol">
                <td><span class="storage-instrument"><span class="order-symbol-logo"><img v-if="logos[row.symbol]" :src="logos[row.symbol]" alt=""/><span v-else aria-hidden="true">{{ row.symbol.slice(0, 1) }}</span></span><span><strong>{{ row.symbol }}</strong><small>{{ names[row.symbol] || row.symbol }}</small></span></span></td>
                <td v-for="column in metricColumns" :key="column.key" class="number">{{ formatMetric(row[column.key], column) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <ChartPanel v-else :figure="figure" :loading="loading" :error="error" />
    </section>
  </div>
</template>

<script setup>
import { BarChart3, CandlestickChart, ChartLine, ChartNoAxesCombined, CircleDollarSign, Gauge, Grid3X3, ScatterChart, Waves } from 'lucide-vue-next'
import { computed, onActivated, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { api, post } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import SearchSelect from '../components/search-select.vue'
import { instrumentLogoUrl, symbolsForAnalysis } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast'])
const plots = [
  { id: 'metrics', label: 'Metrics', icon: Gauge, description: 'Key performance and risk metrics for each selected stock.' },
  { id: 'price', label: 'Price', icon: ChartLine, description: 'Compare price histories across selected symbols.' },
  { id: 'candlestick', label: 'Candles', icon: CandlestickChart, description: 'Inspect open, high, low and close behavior.' },
  { id: 'returns', label: 'Returns', icon: BarChart3, description: 'Study the distribution of bar-to-bar returns.' },
  { id: 'correlation', label: 'Correlation', icon: Grid3X3, description: 'Compare pairwise return relationships.' },
  { id: 'seasonality', label: 'Seasonality', icon: ScatterChart, description: 'Find repeating calendar patterns.' },
  { id: 'volatility', label: 'Volatility', icon: Waves, description: 'Track rolling realized volatility.' },
  { id: 'volume', label: 'Volume', icon: ChartNoAxesCombined, description: 'Compare traded volume through time.' },
  { id: 'vwap', label: 'VWAP', icon: CircleDollarSign, description: 'Contrast price with volume-weighted value.' },
  { id: 'dividends', label: 'Dividends', icon: CircleDollarSign, description: 'Review historical cash distributions.' }
]
const metricDefinitions = {
  sharpe: { label: 'Sharpe', format: 'number' },
  cagr: { label: 'CAGR', format: 'signed-percent' },
  max_dd: { label: 'Max drawdown', format: 'percent' },
  win_rate: { label: 'Win rate', format: 'percent' },
  ann_volatility: { label: 'Annualized volatility', format: 'percent' },
  sortino: { label: 'Sortino', format: 'number' },
  total_bars: { label: 'Total bars', format: 'integer' }
}
const storage = ref([])
const form = reactive({ symbols: [], interval: '1d', price_col: 'close', window: 21, plot: 'metrics' })
const figure = ref(null)
const metrics = ref([])
const loading = ref(false)
const error = ref('')
const symbols = computed(() => [...new Set(storage.value.map(row => row.symbol))])
const names = computed(() => Object.fromEntries(storage.value.map(row => [row.symbol, row.name || `${row.provider} · ${row.interval}`])))
const symbolDetails = computed(() => Object.fromEntries(storage.value.map(row => [row.symbol, row])))
const instrumentTypes = computed(() => Object.fromEntries(storage.value.map(row => [
  row.symbol,
  row.instrument_type || 'stocks'
])))
const logos = computed(() => Object.fromEntries(symbols.value.map(symbol => [
  symbol,
  instrumentLogoUrl(symbol, instrumentTypes.value[symbol], props.bootstrap?.display?.logokit_api_key)
])))
const selectedLogos = computed(() => Object.fromEntries(form.symbols.map(symbol => [
  symbol,
  instrumentLogoUrl(
    symbol,
    instrumentTypes.value[symbol],
    props.bootstrap?.display?.logokit_api_key
  )
])))
const availableIntervals = computed(() => [...new Set(storage.value.filter(row => !form.symbols.length || form.symbols.includes(row.symbol)).map(row => row.interval))])
const current = computed(() => plots.find(item => item.id === form.plot))
const analysisSymbols = computed(() => symbolsForAnalysis(form.symbols, form.plot))
const metricColumns = computed(() => {
  const keys = [...new Set(metrics.value.flatMap(row => Object.keys(row)))]
  return keys.filter(key => key !== 'symbol').map(key => ({
    key,
    label: metricDefinitions[key]?.label || metricLabel(key),
    format: metricDefinitions[key]?.format || 'number'
  }))
})
let refreshTimer
let requestId = 0
let ready = false
let activatedOnce = false

function consumeAnalysisRequest() {
  const requestedSymbols = sessionStorage.getItem('backtide:analysis-symbols')
  const requestedInterval = sessionStorage.getItem('backtide:analysis-interval')
  sessionStorage.removeItem('backtide:analysis-symbols')
  sessionStorage.removeItem('backtide:analysis-interval')
  if (requestedSymbols === null && requestedInterval === null) return false

  const requested = JSON.parse(requestedSymbols || '[]')
  form.symbols = requested.filter(symbol => symbols.value.includes(symbol))
  form.interval = requestedInterval && availableIntervals.value.includes(requestedInterval)
    ? requestedInterval
    : availableIntervals.value.includes('1d') ? '1d' : availableIntervals.value[0] || '1d'
  return true
}

async function plot() {
  const id = ++requestId
  const plotName = form.plot
  loading.value = true; error.value = ''
  try {
    const result = await post('/api/analysis', { ...form, symbols: analysisSymbols.value })
    if (id === requestId) {
      if (plotName === 'metrics') metrics.value = result.rows || []
      else figure.value = result
    }
  } catch (reason) {
    if (id === requestId) { error.value = reason.message; emit('toast', reason.message, 'error') }
  } finally {
    if (id === requestId) loading.value = false
  }
}
function schedulePlot() {
  if (!ready) return
  window.clearTimeout(refreshTimer)
  if (!form.symbols.length) { figure.value = null; return }
  if (!availableIntervals.value.includes(form.interval)) {
    form.interval = availableIntervals.value.includes('1d') ? '1d' : availableIntervals.value[0] || '1d'
    return
  }
  refreshTimer = window.setTimeout(plot, 120)
}
function selectPlot(value) { form.plot = value }
function number(value) {
  return value !== null && value !== '' && Number.isFinite(Number(value))
    ? Number(value).toFixed(2)
    : '—'
}
function percent(value, signed = false) {
  if (value === null || value === '' || !Number.isFinite(Number(value))) return '—'
  const result = Number(value) * 100
  return `${signed && result > 0 ? '+' : ''}${result.toFixed(2)}%`
}
function metricLabel(key) {
  return key.replaceAll('_', ' ').replace(/^./, character => character.toUpperCase())
}
function formatMetric(value, column) {
  if (column.format === 'percent') return percent(value)
  if (column.format === 'signed-percent') return percent(value, true)
  if (column.format === 'integer') {
    return value !== null && value !== '' && Number.isFinite(Number(value))
      ? Math.trunc(Number(value)).toLocaleString()
      : '—'
  }
  return number(value)
}
watch(() => [form.symbols.join('|'), form.interval, form.price_col, form.window, form.plot], schedulePlot)
onMounted(async () => {
  storage.value = await api('/api/storage')
  if (!consumeAnalysisRequest()) {
    form.symbols = symbols.value.slice(0, 2)
    form.interval = availableIntervals.value.includes('1d')
      ? '1d'
      : availableIntervals.value[0] || '1d'
  }
  ready = true
  if (form.symbols.length) plot()
})
onActivated(() => {
  if (!activatedOnce) {
    activatedOnce = true
    return
  }
  if (!ready) return
  consumeAnalysisRequest()
})
onBeforeUnmount(() => window.clearTimeout(refreshTimer))
</script>
