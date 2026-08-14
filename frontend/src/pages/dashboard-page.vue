<template>
  <div class="page dashboard-page">
    <section class="hero-card">
      <div>
        <h2>Test the idea.<br><em>Trade the evidence.</em></h2>
        <p>Build strategies, study market behavior, and move into live trading from one focused workspace.</p>
        <div class="hero-actions">
          <button class="primary" @click="$emit('navigate', 'experiment')"><Beaker :size="17" /> New experiment</button>
          <button class="secondary" @click="$emit('navigate', 'live')"><Radio :size="17" /> Start live trading</button>
        </div>
      </div>
      <div class="market-visual" aria-hidden="true">
        <svg viewBox="0 0 420 220" preserveAspectRatio="none">
          <defs><linearGradient id="fill" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#2684ff" stop-opacity=".35"/><stop offset="1" stop-color="#2684ff" stop-opacity="0"/></linearGradient></defs>
          <path d="M0 180 C45 175 58 120 100 135 S157 170 190 105 S245 55 280 80 S340 130 420 25 L420 220 L0 220Z" fill="url(#fill)"/>
          <path d="M0 180 C45 175 58 120 100 135 S157 170 190 105 S245 55 280 80 S340 130 420 25" fill="none" stroke="#5ba1ff" stroke-width="3"/>
        </svg>
        <div class="floating-quote"><span>WORKSPACE</span><strong>{{ loading ? 'Loading…' : loadError ? 'Unavailable' : `${data?.metrics?.symbols || 0} instruments` }}</strong><small>{{ loading ? 'Checking local storage' : loadError ? 'Could not read local storage' : `${format(data?.metrics?.bars)} stored bars` }}</small></div>
      </div>
    </section>

    <div v-if="loadError" class="callout download-plan-error"><TriangleAlert :size="18" /><span>{{ loadError }}</span><button class="secondary" @click="load">Retry</button></div>

    <section class="metric-grid dashboard-metrics">
      <article v-for="metric in metrics" :key="metric.label" class="metric-card">
        <div class="metric-icon"><component :is="metric.icon" :size="19" /></div>
        <span>{{ metric.label }}</span><strong>{{ metric.value }}</strong><small>{{ metric.note }}</small>
      </article>
    </section>

    <section class="split-grid">
      <article class="panel">
        <div class="panel-header"><div><span class="eyebrow">Latest activity</span><h3>Recent experiments</h3></div><button class="text-button" @click="$emit('navigate', 'results')">View all <ArrowUpRight :size="15" /></button></div>
        <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading recent experiments…</p></div>
        <div v-else-if="!loadError && !data?.experiments?.length" class="empty-state"><Beaker/><p>No experiments yet.</p><button class="secondary" @click="$emit('navigate', 'experiment')">Create your first</button></div>
        <button v-for="experiment in data?.experiments" :key="experiment.id" class="activity-row" @click="openExperiment(experiment)">
          <span class="experiment-avatar" aria-hidden="true">{{ experiment.icon || '🧪' }}</span>
          <span><strong>{{ experiment.name }}</strong><small>{{ time(experiment.started_at) }}</small></span>
          <span class="primary-metric-value"><small>{{ experiment.primary_metric_name || 'Sharpe' }}</small><strong :class="metricTone(primaryMetricValue(experiment))">{{ formatResultMetric(primaryMetricValue(experiment), experiment.primary_metric_percentage) }}</strong></span>
          <span class="badge" :class="String(experiment.status).toLowerCase()">{{ experiment.status }}</span>
          <ChevronRight :size="17" />
        </button>
      </article>
      <article class="panel">
        <div class="panel-header"><div><span class="eyebrow">Market data</span><h3>Recently stored</h3></div><button class="text-button" @click="$emit('navigate', 'storage')">Manage <ArrowUpRight :size="15" /></button></div>
        <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading stored market data…</p></div>
        <div v-else-if="!loadError && !data?.storage?.length" class="empty-state"><Database/><p>Your local database is empty.</p><button class="secondary" @click="$emit('navigate', 'download')">Download data</button></div>
        <button v-for="row in data?.storage" :key="`${row.symbol}-${row.interval}-${row.provider}`" class="activity-row market-row" type="button" @click="openAnalysis(row)">
          <img v-if="logo(row.symbol, row.instrument_type)" :src="logo(row.symbol, row.instrument_type)" class="symbol-logo" alt="" />
          <span v-else class="asset-avatar">{{ row.symbol?.slice(0, 2) }}</span>
          <span><strong>{{ row.symbol }}</strong><small>{{ row.provider }} · {{ row.interval }}</small></span>
          <svg class="market-sparkline" viewBox="0 0 116 38" role="img" :aria-label="`${row.symbol} recent price trend`">
            <polyline :points="sparklinePoints(row.sparkline)" vector-effect="non-scaling-stroke" />
          </svg>
          <ChevronRight :size="17" />
        </button>
      </article>
    </section>

    <section class="panel dashboard-live-panel">
      <div class="panel-header"><div><span class="eyebrow">Live trading</span><h3>Recent live sessions</h3></div><button class="text-button" @click="$emit('navigate', 'live-history')">View all <ArrowUpRight :size="15" /></button></div>
      <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading recent live sessions…</p></div>
      <div v-else-if="!loadError && !data?.sessions?.length" class="empty-state"><History/><p>No live sessions yet.</p><button class="secondary" @click="$emit('navigate', 'live')">Start your first</button></div>
      <button v-for="session in data?.sessions" :key="session.id" class="activity-row live-session-row" type="button" @click="$emit('navigate', 'live-history')">
        <span class="live-session-avatar" aria-hidden="true"><Radio :size="17" /></span>
        <span><strong>{{ sessionSymbols(session) }}</strong><small>{{ time(session.started_at) }}</small></span>
        <span class="session-context"><small>{{ sessionMode(session) }}</small><StrategySummary :names="sessionStrategyNames(session)" /></span>
        <span class="primary-metric-value session-financial"><small>Starting equity</small><strong>{{ sessionStartingEquity(session) }}</strong></span>
        <span class="primary-metric-value session-financial"><small>Final P&amp;L</small><strong :class="metricTone(sessionFinalPnlAmount(session))">{{ sessionFinalPnl(session) }}</strong></span>
        <span class="badge" :class="sessionStatusTone(session.status)">{{ session.status }}</span>
        <ChevronRight :size="17" />
      </button>
    </section>
  </div>
</template>

<script setup>
import { ArrowUpRight, Beaker, ChevronRight, Database, FlaskConical, History, Radio, Rows3, Shapes, TriangleAlert, WalletCards } from 'lucide-vue-next'
import { computed, onActivated, onMounted, ref } from 'vue'
import { api } from '../api'
import StrategySummary from '../components/strategy-summary.vue'
import { formatConfiguredCurrency, formatConfiguredDateTime, formatResultMetric, instrumentLogoUrl } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const data = ref(null)
const loading = ref(true)
const loadError = ref('')
let activatedOnce = false
const metrics = computed(() => [
  { label: 'Experiments', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.experiments), note: 'stored locally', icon: FlaskConical },
  { label: 'Live sessions', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.sessions), note: 'stored locally', icon: History },
  { label: 'Instruments', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.symbols), note: 'ready to analyze', icon: WalletCards },
  { label: 'Market bars', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.bars), note: 'across all intervals', icon: Rows3 },
  { label: 'Data series', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.series), note: 'provider feeds', icon: Shapes }
])
function format(value) { return new Intl.NumberFormat('en', { notation: Number(value) > 99999 ? 'compact' : 'standard' }).format(value || 0) }
function time(value) { return formatConfiguredDateTime(value, props.bootstrap?.display, 'Recently') }
function sessionSymbols(session) {
  const symbols = session.config?.symbols || []
  return symbols.length ? symbols.join(', ') : session.config?.mode === 'replay' ? 'Replay session' : 'Live session'
}
function sessionMode(session) { return session.config?.mode === 'replay' ? 'Replay' : 'Live paper' }
function sessionStrategyNames(session) {
  return session.config?.strategies?.length
    ? session.config.strategies
    : session.config?.strategy ? [session.config.strategy] : []
}
function sessionStartingEquityAmount(session) {
  const strategyCount = sessionStrategyNames(session).length
  const initialCash = Number(session.config?.config?.initial_cash)
  return strategyCount && Number.isFinite(initialCash) ? strategyCount * initialCash : null
}
function sessionMoney(session, value) {
  return formatConfiguredCurrency(
    value,
    session.config?.config?.base_currency || 'USD',
    props.bootstrap?.display
  )
}
function sessionStartingEquity(session) {
  const value = sessionStartingEquityAmount(session)
  return value === null ? '—' : sessionMoney(session, value)
}
function sessionFinalPnlAmount(session) {
  const starting = sessionStartingEquityAmount(session)
  const finalEquity = Number(session.snapshot?.equity)
  return starting === null || !Number.isFinite(finalEquity) ? null : finalEquity - starting
}
function sessionFinalPnl(session) {
  const value = sessionFinalPnlAmount(session)
  return value === null ? '—' : sessionMoney(session, value)
}
function sessionStatusTone(status) {
  if (status === 'error') return 'error'
  if (status === 'running' || status === 'paused') return 'running'
  return 'neutral'
}
function primaryMetricValue(experiment) { return experiment.primary_metric_value ?? experiment.best_sharpe }
function metricTone(value) {
  if (value === null || value === undefined || value === '') return ''
  const number = Number(value)
  return Number.isFinite(number) ? (number > 0 ? 'positive' : number < 0 ? 'negative' : '') : ''
}
function sparklinePoints(values = []) {
  const points = values.map(Number).filter(Number.isFinite)
  if (!points.length) return ''
  const minimum = Math.min(...points)
  const range = Math.max(...points) - minimum || 1
  return points.map((value, index) => {
    const x = points.length === 1 ? 58 : index * 116 / (points.length - 1)
    const y = 35 - ((value - minimum) / range * 32)
    return `${x.toFixed(1)},${y.toFixed(1)}`
  }).join(' ')
}
function logo(symbol, type = '') {
  return instrumentLogoUrl(symbol, type, props.bootstrap.display.logokit_api_key)
}
function openExperiment(experiment) {
  sessionStorage.setItem('backtide:result-id', experiment.id)
  emit('navigate', 'results')
}
function openAnalysis(row) {
  sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify([row.symbol]))
  emit('navigate', 'analysis')
}
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    data.value = await api('/api/dashboard')
  } catch (error) {
    data.value = null
    loadError.value = `Could not load workspace data. ${error.message}`
    emit('toast', loadError.value, 'error')
  } finally {
    loading.value = false
  }
}
onMounted(load)
onActivated(() => {
  if (activatedOnce) load()
  activatedOnce = true
})
</script>
