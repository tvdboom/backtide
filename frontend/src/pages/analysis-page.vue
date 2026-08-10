<template>
  <div class="page">
    <section class="page-intro"><div><span class="eyebrow">Visual research</span><h2>Analyze market data</h2><p>Explore prices, returns, correlation, seasonality, volatility, volume and dividends.</p></div></section>
    <section class="panel analysis-controls">
      <label>Symbols<SearchSelect v-model="form.symbols" :options="symbols" :descriptions="names" placeholder="Search stored instruments…" /></label>
      <label>Interval<select v-model="form.interval"><option v-for="item in availableIntervals" :key="item">{{ item }}</option></select></label>
      <label>Price<select v-model="form.price_col"><option value="open">Open</option><option value="high">High</option><option value="low">Low</option><option value="close">Close</option><option value="adj_close">Adjusted close</option></select></label>
      <label v-if="form.plot === 'volatility'">Window<input v-model.number="form.window" type="number" min="2" /></label>
      <button class="primary" :disabled="loading || !form.symbols.length" @click="plot"><Play :size="16" /> Run analysis</button>
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
import { BarChart3, CandlestickChart, ChartLine, ChartNoAxesCombined, CircleDollarSign, Grid3X3, Play, ScatterChart, Waves } from 'lucide-vue-next'
import { computed, onMounted, reactive, ref } from 'vue'
import { api, post } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import SearchSelect from '../components/search-select.vue'
import { symbolsForAnalysis } from '../state'

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
const availableIntervals = computed(() => [...new Set(storage.value.filter(row => !form.symbols.length || form.symbols.includes(row.symbol)).map(row => row.interval))])
const current = computed(() => plots.find(item => item.id === form.plot))
const analysisSymbols = computed(() => symbolsForAnalysis(form.symbols, form.plot))
async function plot() {
  loading.value = true; error.value = ''
  try { figure.value = await post('/api/analysis', { ...form, symbols: analysisSymbols.value }) }
  catch (reason) { error.value = reason.message; emit('toast', reason.message, 'error') }
  finally { loading.value = false }
}
function selectPlot(value) { form.plot = value; if (form.symbols.length) plot() }
onMounted(async () => {
  storage.value = await api('/api/storage')
  const requested = JSON.parse(sessionStorage.getItem('backtide:analysis-symbols') || '[]')
  sessionStorage.removeItem('backtide:analysis-symbols')
  form.symbols = requested.filter(symbol => symbols.value.includes(symbol))
  if (!form.symbols.length) form.symbols = symbols.value.slice(0, 2)
  form.interval = availableIntervals.value.includes('1d') ? '1d' : availableIntervals.value[0] || '1d'
  if (form.symbols.length) plot()
})
</script>
