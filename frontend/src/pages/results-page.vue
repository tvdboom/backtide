<template>
  <div class="page">
    <section class="page-intro">
      <div><span class="eyebrow">Backtest output</span><h2>Experiment results</h2><p>Compare the complete experiment first, then inspect metrics, trades, and execution for each strategy.</p></div>
      <button class="primary" @click="$emit('navigate', 'experiment')"><Plus :size="16" /> New experiment</button>
    </section>
    <section v-if="activeJobs.length" class="panel running-banner">
      <span class="spinner"/><div><strong>{{ activeJobs.length }} experiment{{ activeJobs.length === 1 ? '' : 's' }} running</strong><small>Results appear here automatically when processing completes.</small></div>
      <button class="danger secondary" @click="abort"><Square :size="14"/> Abort</button>
    </section>
    <section class="results-layout">
      <aside class="panel experiment-list">
        <label class="search-box"><Search :size="16"/><input v-model="search" placeholder="Search results…" @input="debouncedLoad" /></label>
        <button v-for="item in experiments" :key="item.id" :class="{ active: selectedId === item.id }" @click="open(item.id)">
          <span class="asset-avatar">{{ (item.name || 'E').slice(0, 2).toUpperCase() }}</span>
          <span><strong>{{ item.name }}</strong><small>{{ date(item.started_at) }} · {{ item.status }}</small></span><ChevronRight :size="16"/>
        </button>
        <div v-if="!experiments.length" class="empty-state"><FlaskConical/><p>No completed experiments.</p></div>
      </aside>
      <section class="result-detail">
        <div v-if="loading" class="panel loading-screen"><span class="spinner"/> Loading results…</div>
        <div v-else-if="detailError" class="panel empty-state large"><TriangleAlert :size="34"/><h3>Could not load experiment</h3><p>{{ detailError }}</p><button class="secondary" @click="open(selectedId)">Retry</button></div>
        <div v-else-if="!detail" class="panel empty-state large"><Gauge :size="34"/><h3>Select an experiment</h3><p>Choose a run to inspect its performance.</p></div>
        <template v-else>
          <article class="panel result-heading">
            <div>
              <h2>{{ detail.experiment.name }}</h2>
              <div class="result-title"><span class="badge" :class="String(detail.experiment.status).toLowerCase()">{{ detail.experiment.status }}</span><span v-for="tag in tags(detail.experiment.tags)" :key="tag" class="badge neutral">{{ tag }}</span></div>
              <p v-if="experimentDescription">{{ experimentDescription }}</p>
            </div>
            <div class="result-actions">
              <button class="secondary" :disabled="!detail.config" @click="reuseSetup"><CopyPlus :size="15"/> Reuse setup</button>
              <button class="secondary" :disabled="!detail.config" @click="documentView = 'config'"><FileCode2 :size="15"/> Config</button>
              <button class="secondary" :disabled="!detail.logs" @click="documentView = 'logs'"><ScrollText :size="15"/> Logs</button>
              <button class="icon-button danger" aria-label="Delete experiment" @click="requestDelete"><Trash2 :size="17"/></button>
            </div>
          </article>

          <div class="result-section-heading">
            <div><span class="eyebrow">Full result</span><h3>Experiment overview</h3></div>
            <p>Every strategy is shown together so performance and risk remain directly comparable.</p>
          </div>
          <article class="panel chart-workspace result-workspace">
            <div class="chart-tabs"><button v-for="tabItem in overviewTabs" :key="tabItem.id" :class="{ active: overviewTab === tabItem.id }" @click="overviewTab = tabItem.id">{{ tabItem.label }}</button></div>
            <div class="result-plot-options">
              <label v-if="overviewTab === 'pnl'" class="toggle-label"><span>Normalize</span><input v-model="overviewOptions.normalize" class="toggle" type="checkbox" @change="loadOverviewPlot"/></label>
              <label v-if="overviewTab === 'pnl'" class="toggle-label"><span>Drawdown</span><input v-model="overviewOptions.drawdown" class="toggle" type="checkbox" @change="loadOverviewPlot"/></label>
              <label v-if="['rolling_returns', 'rolling_sharpe'].includes(overviewTab)">Window<input v-model.number="overviewOptions.window" type="number" min="2" max="365" @change="loadOverviewPlot"/></label>
              <label v-if="['pnl_histogram', 'trade_duration'].includes(overviewTab)">Bins<input v-model.number="overviewOptions.bins" type="number" min="5" max="100" @change="loadOverviewPlot"/></label>
              <label v-if="overviewTab === 'trade_duration'">Unit<select v-model="overviewOptions.unit" @change="loadOverviewPlot"><option>auto</option><option>minutes</option><option>hours</option><option>days</option></select></label>
            </div>
            <ChartPanel :figure="overviewFigure" :loading="overviewLoading" :error="overviewError" />
          </article>

          <div class="result-section-heading strategy-heading">
            <div><span class="eyebrow">Run details</span><h3>Strategies</h3></div>
            <p>Select a strategy to inspect its own metrics, trades, orders, and chart annotations.</p>
          </div>
          <div class="strategy-switcher"><button v-for="(run, index) in detail.runs" :key="run.strategy_id" :class="{ active: strategy === index }" @click="strategy = index">{{ run.strategy_name }}<span v-if="run.is_benchmark" class="badge neutral">Benchmark</span></button></div>
          <section v-if="activeRun" class="metric-grid result-metrics">
            <article v-for="metricItem in headlineMetrics" :key="metricItem.label" class="metric-card"><span>{{ metricItem.label }}</span><strong :class="tone(metricItem.raw)">{{ metricItem.value }}</strong><small>{{ metricItem.note }}</small></article>
          </section>
          <article class="panel chart-workspace result-workspace">
            <div class="chart-tabs"><button v-for="tabItem in strategyTabs" :key="tabItem.id" :class="{ active: strategyTab === tabItem.id }" @click="strategyTab = tabItem.id">{{ tabItem.label }}</button></div>
            <div v-if="isStrategyPlot && strategyTab === 'price'" class="result-plot-options">
              <label>Symbol<select v-model="strategyOptions.symbol" @change="loadStrategyPlot"><option v-for="symbol in tradedSymbols" :key="symbol">{{ symbol }}</option></select></label>
            </div>
            <ChartPanel v-if="isStrategyPlot" :figure="strategyFigure" :loading="strategyLoading" :error="strategyError" />
            <div v-else class="data-table-wrap result-table">
              <table class="data-table"><thead><tr><th v-for="column in tableColumns" :key="column">{{ label(column) }}</th></tr></thead><tbody><tr v-for="(row, index) in tableRows" :key="index"><td v-for="column in tableColumns" :key="column">{{ cell(row[column]) }}</td></tr></tbody></table>
              <div v-if="!tableRows.length" class="empty-state"><p>No {{ strategyTabLabel.toLowerCase() }} for this run.</p></div>
            </div>
          </article>
          <article v-if="activeRun?.error" class="callout error-state"><TriangleAlert/><span>{{ activeRun.error }}</span></article>
        </template>
      </section>
    </section>
    <div v-if="documentView" class="modal-layer" @mousedown.self="documentView = ''">
      <article class="modal panel document-modal"><div class="panel-header"><div><span class="eyebrow">Experiment artifact</span><h3>{{ documentView === 'config' ? 'Saved configuration' : 'Engine logs' }}</h3></div><button class="icon-button" @click="documentView = ''"><X/></button></div><pre>{{ documentView === 'config' ? detail.config : detail.logs }}</pre></article>
    </div>
    <ConfirmationModal
      :open="Boolean(pendingDelete)"
      :title="pendingDelete ? `Delete ${pendingDelete.name}?` : ''"
      message="Are you sure you want to delete this experiment and its saved results? This action cannot be undone."
      :busy="deleting"
      @cancel="pendingDelete = null"
      @confirm="destroy"
    />
  </div>
</template>

<script setup>
import { ChevronRight, CopyPlus, FileCode2, FlaskConical, Gauge, Plus, ScrollText, Search, Square, Trash2, TriangleAlert, X } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { api, post, query, remove } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import ConfirmationModal from '../components/confirmation-modal.vue'
import { formatResultMetric } from '../state'

defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const experiments = ref([]), jobs = ref([]), detail = ref(null), selectedId = ref(''), search = ref(''), loading = ref(false), strategy = ref(0), documentView = ref('')
const detailError = ref('')
const deleting = ref(false)
const pendingDelete = ref(null)
const overviewTab = ref('pnl'), overviewFigure = ref(null), overviewLoading = ref(false), overviewError = ref('')
const strategyTab = ref('mae_mfe'), strategyFigure = ref(null), strategyLoading = ref(false), strategyError = ref('')
const overviewOptions = reactive({ normalize: false, drawdown: true, window: 30, bins: 40, unit: 'auto' })
const strategyOptions = reactive({ symbol: '' })
const overviewTabs = [
  { id: 'pnl', label: 'PnL' }, { id: 'cash', label: 'Cash' }, { id: 'pnl_histogram', label: 'PnL histogram' },
  { id: 'rolling_returns', label: 'Rolling returns' }, { id: 'rolling_sharpe', label: 'Rolling Sharpe' },
  { id: 'trade_duration', label: 'Trade duration' }, { id: 'trade_pnl', label: 'Trade PnL' }
]
const strategyTabs = [
  { id: 'mae_mfe', label: 'MAE / MFE' }, { id: 'position_size', label: 'Position size' },
  { id: 'price', label: 'Trades on price' }, { id: 'trades', label: 'Trades' },
  { id: 'orders', label: 'Orders' }, { id: 'metrics', label: 'All metrics' }
]
const strategyPlotIds = new Set(strategyTabs.slice(0, 3).map(item => item.id))
let pollTimer, searchTimer
const activeJobs = computed(() => jobs.value.filter(job => job.kind === 'experiment' && ['queued', 'running'].includes(job.status)))
const activeRun = computed(() => detail.value?.runs?.[strategy.value])
const experimentDescription = computed(() => String(detail.value?.experiment?.description || '').trim())
const isStrategyPlot = computed(() => strategyPlotIds.has(strategyTab.value))
const strategyTabLabel = computed(() => strategyTabs.find(item => item.id === strategyTab.value)?.label || '')
const tradedSymbols = computed(() => [...new Set((activeRun.value?.trades || []).map(item => item.symbol))])
const headlineMetrics = computed(() => {
  const metrics = activeRun.value?.metrics || {}
  return [metric('Total return', metrics.total_return ?? metrics.return, true, 'net performance'), metric('CAGR', metrics.cagr, true, 'annualized'), metric('Sharpe ratio', metrics.sharpe_ratio ?? metrics.sharpe, false, 'risk adjusted'), metric('Max drawdown', metrics.max_drawdown, true, 'peak to trough'), metric('Win rate', metrics.win_rate, true, 'profitable trades'), metric('Trades', activeRun.value?.trades?.length || 0, false, 'completed')]
})
const tableRows = computed(() => strategyTab.value === 'trades' ? activeRun.value?.trades || [] : strategyTab.value === 'orders' ? activeRun.value?.orders || [] : Object.entries(activeRun.value?.metrics || {}).map(([metricName, value]) => ({ metric: metricName, value })))
const tableColumns = computed(() => Object.keys(tableRows.value[0] || {}))

function metric(labelText, raw, percent, note) { return { label: labelText, raw: Number(raw), value: formatResultMetric(raw, percent), note } }
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
const tags = value => Array.isArray(value) ? value : String(value || '').split(',').map(item => item.trim()).filter(Boolean)
function date(value) { return value ? new Date(Number(value) * (Number(value) < 1e12 ? 1000 : 1)).toLocaleDateString() : 'Unknown date' }
function label(value) { return value.replaceAll('_', ' ').replace(/^./, match => match.toUpperCase()) }
function cell(value) { if (value && typeof value === 'object') return JSON.stringify(value); if (typeof value === 'number') return value.toLocaleString(undefined, { maximumFractionDigits: 4 }); return value ?? '—' }
async function load() {
  experiments.value = await query('/api/experiments', { search: search.value })
  const requested = sessionStorage.getItem('backtide:result-id')
  sessionStorage.removeItem('backtide:result-id')
  if (!selectedId.value && experiments.value.length) {
    const selected = experiments.value.find(item => item.id === requested) || experiments.value[0]
    open(selected.id)
  }
}
function debouncedLoad() { clearTimeout(searchTimer); searchTimer = setTimeout(load, 250) }
async function open(id) {
  selectedId.value = id
  loading.value = true
  detail.value = null
  overviewFigure.value = null
  strategyFigure.value = null
  detailError.value = ''
  strategy.value = 0
  try {
    detail.value = await api(`/api/experiments/${id}`)
    strategyOptions.symbol = tradedSymbols.value[0] || ''
    loading.value = false
    void Promise.all([loadOverviewPlot(), loadStrategyPlot()])
  } catch (error) {
    detailError.value = error.message
    emit('toast', error.message, 'error')
  } finally {
    loading.value = false
  }
}
async function loadOverviewPlot() {
  if (!activeRun.value) return
  overviewLoading.value = true
  overviewError.value = ''
  try {
    overviewFigure.value = await post('/api/results/plot', { experiment_id: selectedId.value, strategy_id: activeRun.value.strategy_id, plot: overviewTab.value, options: overviewOptions })
  } catch (error) { overviewError.value = error.message }
  finally { overviewLoading.value = false }
}
async function loadStrategyPlot() {
  if (!isStrategyPlot.value || !activeRun.value) return
  strategyLoading.value = true
  strategyError.value = ''
  try {
    strategyFigure.value = await post('/api/results/plot', { experiment_id: selectedId.value, strategy_id: activeRun.value.strategy_id, plot: strategyTab.value, options: strategyOptions })
  } catch (error) { strategyError.value = error.message }
  finally { strategyLoading.value = false }
}
async function reuseSetup() {
  try {
    const config = await post('/api/config/parse', { suffix: '.toml', text: detail.value.config })
    sessionStorage.setItem('backtide:experiment-config', JSON.stringify(config))
    emit('navigate', 'experiment')
  } catch (error) { emit('toast', error.message, 'error') }
}
function requestDelete() {
  pendingDelete.value = { id: selectedId.value, name: detail.value.experiment.name }
}
async function destroy() {
  const target = pendingDelete.value
  if (!target) return
  deleting.value = true
  try {
    await remove(`/api/experiments/${target.id}`)
    pendingDelete.value = null
    detail.value = null
    selectedId.value = ''
    emit('toast', 'Experiment deleted.')
    await load()
  } catch (error) { emit('toast', error.message, 'error') }
  finally { deleting.value = false }
}
async function abort() { await post('/api/experiments/abort'); emit('toast', 'Abort requested.') }
async function pollJobs() { const previous = activeJobs.value.length; jobs.value = await api('/api/jobs'); if (previous && !activeJobs.value.length) await load(); pollTimer = setTimeout(pollJobs, 1500) }
watch(overviewTab, loadOverviewPlot)
watch([strategyTab, strategy], () => { strategyOptions.symbol = tradedSymbols.value[0] || ''; loadStrategyPlot() })
onMounted(() => { load(); pollJobs() })
onBeforeUnmount(() => { clearTimeout(pollTimer); clearTimeout(searchTimer) })
</script>
