<template>
  <div class="page">
    <section class="page-intro"><div><h2>Analyze market data</h2><p>Explore prices, returns, correlation, seasonality, volatility, volume and dividends.</p></div></section>
    <section class="panel analysis-controls">
      <label>Symbols<SearchSelect v-model="form.symbols" :options="symbols" :descriptions="names" :logos="logos" :selected-logos="selectedLogos" placeholder="Search stored instruments…" /></label>
      <label>Interval<select v-model="form.interval"><option v-for="item in availableIntervals" :key="item">{{ item }}</option></select></label>
      <label>Price<select v-model="form.price_col"><option value="open">Open</option><option value="high">High</option><option value="low">Low</option><option value="close">Close</option><option value="adj_close">Adjusted close</option></select></label>
      <label v-if="form.plot === 'volatility'">Window<input v-model.number="form.window" type="number" min="2" /></label>
    </section>
    <section class="panel chart-workspace">
      <div class="chart-tabs" role="tablist">
        <button v-for="item in plots" :key="item.id" :class="{ active: form.plot === item.id }" type="button" @click="selectPlot(item.id)"><component :is="item.icon" :size="16"/>{{ item.label }}</button>
      </div>
      <div class="chart-title"><div><h3>{{ current.label }}</h3><p>{{ current.description }}</p></div></div>
      <ChartPanel :figure="figure" :loading="loading" :error="error" />
    </section>
  </div>
</template>

<script setup>
import { BarChart3, CandlestickChart, ChartLine, ChartNoAxesCombined, CircleDollarSign, Grid3X3, ScatterChart, Waves } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { api, post } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import SearchSelect from '../components/search-select.vue'
import { instrumentLogoUrl, symbolsForAnalysis } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast'])
const plots = [
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
const storage = ref([])
const form = reactive({ symbols: [], interval: '1d', price_col: 'close', window: 21, plot: 'price' })
const figure = ref(null)
const loading = ref(false)
const error = ref('')
const symbols = computed(() => [...new Set(storage.value.map(row => row.symbol))])
const names = computed(() => Object.fromEntries(storage.value.map(row => [row.symbol, row.name || `${row.provider} · ${row.interval}`])))
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
let refreshTimer
let requestId = 0
let ready = false
async function plot() {
  const id = ++requestId
  loading.value = true; error.value = ''
  try {
    const result = await post('/api/analysis', { ...form, symbols: analysisSymbols.value })
    if (id === requestId) figure.value = result
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
watch(() => [form.symbols.join('|'), form.interval, form.price_col, form.window, form.plot], schedulePlot)
onMounted(async () => {
  storage.value = await api('/api/storage')
  const requested = JSON.parse(sessionStorage.getItem('backtide:analysis-symbols') || '[]')
  const requestedInterval = sessionStorage.getItem('backtide:analysis-interval')
  sessionStorage.removeItem('backtide:analysis-symbols')
  sessionStorage.removeItem('backtide:analysis-interval')
  form.symbols = requested.filter(symbol => symbols.value.includes(symbol))
  if (!form.symbols.length) form.symbols = symbols.value.slice(0, 2)
  form.interval = requestedInterval && availableIntervals.value.includes(requestedInterval)
    ? requestedInterval
    : availableIntervals.value.includes('1d') ? '1d' : availableIntervals.value[0] || '1d'
  ready = true
  if (form.symbols.length) plot()
})
onBeforeUnmount(() => window.clearTimeout(refreshTimer))
</script>
