<template>
  <div class="page">
    <section class="page-intro results-page-intro">
      <div><h2>Experiment results</h2><p>Compare the complete experiment first, then inspect metrics, trades, and execution for each strategy.</p></div>
      <button class="primary" @click="$emit('navigate', 'experiment')"><Plus :size="16" /> New experiment</button>
    </section>
    <section v-if="activeJobs.length" class="panel running-banner">
      <span class="spinner"/><div><strong>{{ activeJobs.length }} experiment{{ activeJobs.length === 1 ? '' : 's' }} running</strong><small>Results appear here automatically when processing completes.</small></div>
      <button class="danger secondary" @click="abort"><Square :size="14"/> Abort</button>
    </section>
    <section v-if="!selectedId" class="results-overview">
      <div class="toolbar panel compact results-toolbar">
        <label class="search-box"><Search :size="16"/><input v-model="search" placeholder="Search results…" @input="debouncedLoad" /></label>
        <label class="results-status-filter"><span>Status</span><select v-model="statusFilter"><option>All</option><option>Success</option><option>Error</option></select></label>
        <span>{{ visibleExperiments.length }} experiment{{ visibleExperiments.length === 1 ? '' : 's' }}</span>
      </div>
      <div v-if="experimentsLoading" class="panel loading-screen results-list-loading" role="status"><span class="spinner"/> Loading experiments…</div>
      <div v-else-if="experimentsError && !experiments.length" class="panel empty-state large error-state"><TriangleAlert :size="34"/><h3>Could not load experiments</h3><p>{{ experimentsError }}</p><button class="secondary" @click="load()">Retry</button></div>
      <div v-else-if="visibleExperiments.length" class="experiment-result-list">
        <article v-for="item in visibleExperiments" :key="item.id" class="experiment-result-card panel" :class="{ expanded: expandedId === item.id }">
          <header class="experiment-result-summary">
            <div class="experiment-result-identity">
              <span class="experiment-avatar" aria-hidden="true">{{ item.icon || '🧪' }}</span>
              <div class="experiment-result-copy">
                <div class="experiment-result-title-line"><h3>{{ item.name }}</h3></div>
                <div v-if="tags(item.tags).length" class="experiment-result-tags"><span v-for="tag in tags(item.tags)" :key="tag" class="result-tag">{{ tag }}</span></div>
                <div class="experiment-result-meta">
                  <span><CalendarDays :size="14" />{{ dateTime(item.started_at) }}</span>
                  <span><Medal :size="14" />{{ item.primary_metric_name || 'Sharpe' }} <strong :class="tone(item.primary_metric_value ?? item.best_sharpe)">{{ formatResultMetric(item.primary_metric_value ?? item.best_sharpe, item.primary_metric_percentage) }}</strong></span>
                  <span><BrainCircuit :size="14" />{{ item.n_strategies || item.runs?.length || 0 }} {{ (item.n_strategies || item.runs?.length) === 1 ? 'strategy' : 'strategies' }}</span>
                  <span><ChartNoAxesCombined :size="14" />{{ item.n_symbols || 0 }} {{ item.n_symbols === 1 ? 'symbol' : 'symbols' }}</span>
                  <span class="experiment-result-status"><Activity :size="14" /><strong :class="statusTone(item.status)">{{ item.status || 'Unknown' }}</strong></span>
                </div>
              </div>
            </div>
            <div class="experiment-card-actions">
              <button type="button" class="breakdown-toggle" :aria-expanded="expandedId === item.id" @click="toggleBreakdown(item.id)">{{ expandedId === item.id ? 'Hide breakdown' : 'Show breakdown' }}<ChevronDown :size="16" /></button>
              <button type="button" class="secondary" @click="open(item.id)"><FileChartColumn :size="16" /> Full results</button>
              <button type="button" class="icon-button danger" :aria-label="`Delete ${item.name}`" @click="requestDelete(item)"><Trash2 :size="16" /></button>
            </div>
          </header>
          <div v-if="expandedId === item.id" class="experiment-breakdown">
            <article v-for="run in item.runs" :key="run.strategy_id" class="run-breakdown-card">
              <header><span class="run-kind-icon" :class="{ benchmark: run.is_benchmark }"><BarChart3 v-if="run.is_benchmark" :size="17" /><BrainCircuit v-else :size="17" /></span><span><strong>{{ run.strategy_name }}</strong></span></header>
              <div v-if="run.error" class="run-summary-error">{{ run.error }}</div>
              <div v-else class="run-summary-metrics">
                <div v-for="metricItem in runSummaryMetrics(item, run)" :key="metricItem.key"><span>{{ metricItem.label }}</span><strong :class="metricItem.tone">{{ metricItem.value }}</strong></div>
              </div>
            </article>
            <div v-if="!item.runs?.length" class="overview-no-runs">No strategy metrics were stored for this experiment.</div>
          </div>
        </article>
      </div>
      <div v-else class="panel empty-state large"><FlaskConical/><h3>No completed experiments</h3><p>Run an experiment to see strategy metrics here.</p></div>
      <div v-if="!experimentsLoading && experimentsLoadingMore" class="table-load-state" role="status"><span class="spinner small"/> Loading more experiments…</div>
      <div v-else-if="!experimentsLoading && experimentsError && experiments.length" class="table-load-state error-state"><span>{{ experimentsError }}</span><button class="text-button" type="button" @click="loadMoreExperiments">Retry</button></div>
      <div v-else-if="!experimentsLoading && experimentsHasMore" ref="loadMoreSentinel" class="results-load-sentinel" aria-hidden="true" />
    </section>
    <section v-else class="result-detail-page">
        <button type="button" class="text-button results-back" @click="backToOverview"><ArrowLeft :size="16" /> Back to experiments</button>
        <div v-if="loading" class="panel loading-screen"><span class="spinner"/> Loading results…</div>
        <div v-else-if="detailError" class="panel empty-state large"><TriangleAlert :size="34"/><h3>Could not load experiment</h3><p>{{ detailError }}</p><button class="secondary" @click="open(selectedId)">Retry</button></div>
        <div v-else-if="!detail" class="panel empty-state large"><Gauge :size="34"/><h3>Select an experiment</h3><p>Choose a run to inspect its performance.</p></div>
        <template v-else>
          <section class="panel result-summary-panel">
            <article class="result-heading">
              <div class="result-heading-copy">
                <span class="experiment-avatar result-heading-icon" aria-hidden="true">{{ detail.experiment.icon || '🧪' }}</span>
                <div><h2>{{ detail.experiment.name }}</h2>
                <div v-if="tags(detail.experiment.tags).length" class="result-title"><span v-for="tag in tags(detail.experiment.tags)" :key="tag" class="badge neutral">{{ tag }}</span></div>
                <p v-if="experimentDescription">{{ experimentDescription }}</p></div>
              </div>
              <div class="result-actions">
                <button class="secondary" :disabled="!detail.config" @click="reuseSetup"><CopyPlus :size="15"/> Reuse setup</button>
                <button class="secondary" :disabled="!detail.config" @click="openLiveTrading"><Activity :size="15"/> Live trading</button>
                <button class="secondary" :disabled="!detail.config" @click="openDocument('config')"><FileCode2 :size="15"/> Config</button>
                <button class="secondary" :disabled="detail.logs == null" @click="openDocument('logs')"><ScrollText :size="15"/> Logs</button>
                <button class="icon-button danger" aria-label="Delete experiment" @click="requestDelete()"><Trash2 :size="17"/></button>
              </div>
            </article>

            <section class="result-overview-metrics" aria-label="Experiment summary metrics">
              <div class="result-overview-row primary-metrics">
                <article v-for="metricItem in experimentPrimaryMetrics" :key="metricItem.label" class="result-overview-metric">
                  <component :is="metricItem.icon" :size="22" />
                  <span>{{ metricItem.label }}</span>
                  <strong :class="metricItem.tone">{{ metricItem.value }}</strong>
                </article>
              </div>
              <div class="result-overview-row context-metrics">
                <article v-for="metricItem in experimentContextMetrics" :key="metricItem.label" class="result-overview-metric">
                  <component :is="metricItem.icon" :size="22" />
                  <span>{{ metricItem.label }}</span>
                  <strong>{{ metricItem.value }}</strong>
                </article>
              </div>
            </section>
          </section>

          <div class="result-section-heading">
            <div><span class="eyebrow">Full result</span><h3>Experiment overview</h3></div>
            <p>Every strategy is shown together so performance and risk remain directly comparable.</p>
          </div>
          <article class="panel chart-workspace result-workspace">
            <div class="result-plot-tabs" role="tablist" aria-label="Experiment plots"><button v-for="tabItem in overviewTabs" :key="tabItem.id" role="tab" :aria-selected="overviewTab === tabItem.id" :class="{ active: overviewTab === tabItem.id }" @click="overviewTab = tabItem.id"><component :is="tabItem.icon" :size="18"/><strong>{{ tabItem.label }}</strong></button></div>
            <div class="result-plot-description"><p>{{ activeOverviewTab.description }}</p></div>
            <div class="result-plot-stage" :class="{ 'has-options': hasOverviewOptions }">
              <ChartPanel :figure="overviewFigure" :loading="overviewLoading" :error="overviewError" />
              <aside v-if="hasOverviewOptions" class="result-plot-options" aria-label="Plot options">
                <div class="result-options-heading"><SlidersHorizontal :size="16"/><span>Plot options</span></div>
                <ToggleField v-if="overviewTab === 'pnl'" v-model="overviewOptions.normalize" label="Normalize" description="Show P&amp;L relative to starting equity." help="Display cumulative profit and loss as a percentage of starting equity." @change="loadOverviewPlot" />
                <ToggleField v-if="overviewTab === 'pnl'" v-model="overviewOptions.drawdown" label="Show drawdown" description="Add portfolio drawdown to the chart." help="Overlay portfolio drawdown alongside cumulative profit and loss." @change="loadOverviewPlot" />
                <label v-if="['rolling_returns', 'rolling_sharpe'].includes(overviewTab)">Window<input v-model.number="overviewOptions.window" type="number" min="2" max="365" @change="loadOverviewPlot"/><small>Number of bars in the rolling window.</small></label>
                <label v-if="['pnl_histogram', 'trade_duration'].includes(overviewTab)">Bins<input v-model.number="overviewOptions.bins" type="number" min="5" max="100" @change="loadOverviewPlot"/><small>Number of histogram bins.</small></label>
                <label v-if="overviewTab === 'trade_duration'">Unit<select v-model="overviewOptions.unit" @change="loadOverviewPlot"><option>auto</option><option>minutes</option><option>hours</option><option>days</option></select><small>Time unit on the horizontal axis.</small></label>
              </aside>
            </div>
          </article>

          <div class="result-section-heading strategy-heading">
            <div><span class="eyebrow">Run details</span><h3>Strategies</h3></div>
            <p>Select a strategy to inspect its own metrics, execution history, and chart annotations.</p>
          </div>
          <div class="strategy-switcher"><button v-for="(run, index) in detail.runs" :key="run.strategy_id" :class="{ active: strategy === index, 'benchmark-run': run.is_benchmark }" @click="strategy = index"><BarChart3 v-if="run.is_benchmark" :size="15" aria-hidden="true"/><span>{{ run.strategy_name }}</span></button></div>
          <section v-if="activeRun" class="metric-grid result-metrics">
            <article v-for="metricItem in headlineMetrics" :key="metricItem.label" class="metric-card"><span>{{ metricItem.label }}</span><strong :class="tone(metricItem.raw)">{{ metricItem.value }}</strong></article>
          </section>
          <article class="panel chart-workspace result-workspace">
            <div class="result-plot-tabs strategy-plot-tabs" role="tablist" aria-label="Strategy results"><button v-for="tabItem in strategyTabs" :key="tabItem.id" role="tab" :aria-selected="strategyTab === tabItem.id" :class="{ active: strategyTab === tabItem.id }" @click="strategyTab = tabItem.id"><component :is="tabItem.icon" :size="18"/><strong>{{ tabItem.label }}</strong></button></div>
            <div class="result-plot-description"><p>{{ activeStrategyTab.description }}</p></div>
            <div class="result-plot-stage" :class="{ 'has-options': isStrategyPlot && strategyTab === 'price' }">
              <ChartPanel v-if="isStrategyPlot" :figure="strategyFigure" :loading="strategyLoading" :error="strategyError" />
              <div v-else class="data-table-wrap result-table" :class="{ 'result-record-table': isRecordTable, 'result-orders-table': strategyTab === 'orders' }" @scroll.passive="loadOrdersOnScroll">
                <table class="data-table"><thead><tr><th v-for="column in tableColumns" :key="column" :class="tableColumnClass(column)">{{ label(column) }}</th></tr></thead><tbody><tr v-for="(row, index) in tableRows" :key="index"><td v-for="column in tableColumns" :key="column" :class="tableCellClass(row, column)"><span v-if="isRecordTable && column === 'symbol'" class="order-symbol-cell"><span class="order-symbol-logo"><img v-if="symbolLogo(row) && !failedSymbolLogos.has(row.symbol)" :src="symbolLogo(row)" alt="" @error="markSymbolLogoFailed(row.symbol)"/><span v-else aria-hidden="true">{{ String(row.symbol || '?').slice(0, 1) }}</span></span><strong>{{ row.symbol }}</strong></span><ExecutionStatus v-else-if="isRecordTable && column === 'status'" :status="row.status"/><template v-else>{{ cell(row[column], column) }}</template></td></tr></tbody></table>
                <div v-if="!tableRows.length && !(strategyTab === 'orders' && (ordersLoading || ordersError))" class="empty-state"><p>No {{ strategyTabLabel.toLowerCase() }} for this run.</p></div>
                <div v-if="strategyTab === 'orders' && ordersLoading" class="table-load-state" role="status"><span class="spinner small"/> Loading more orders…</div>
                <div v-else-if="strategyTab === 'orders' && ordersError" class="table-load-state error-state"><span>{{ ordersError }}</span><button class="text-button" type="button" @click="loadMoreOrders">Retry</button></div>
              </div>
              <aside v-if="isStrategyPlot && strategyTab === 'price'" class="result-plot-options" aria-label="Plot options">
                <div class="result-options-heading"><SlidersHorizontal :size="16"/><span>Plot options</span></div>
                <label>Symbol<select v-model="strategyOptions.symbol" @change="loadStrategyPlot"><option v-for="symbol in tradedSymbols" :key="symbol">{{ symbol }}</option></select><small>Instrument shown on the price chart.</small></label>
              </aside>
            </div>
          </article>
          <article v-if="activeRun?.error" class="callout error-state"><TriangleAlert/><span>{{ activeRun.error }}</span></article>
        </template>
    </section>
    <div v-if="documentView" class="modal-layer" @mousedown.self="documentView = ''">
      <article class="modal panel document-modal"><div class="panel-header"><div><span class="eyebrow">Experiment artifact</span><h3>{{ documentView === 'config' ? 'Saved configuration' : 'Engine logs' }}</h3></div><div class="document-modal-actions"><a v-if="documentView === 'logs'" class="secondary" :href="fullLogUrl" download><Download :size="15"/> Download full log</a><button class="icon-button" aria-label="Close experiment artifact" @click="documentView = ''"><X/></button></div></div><div v-if="documentView === 'logs' && detail.logs === ''" class="document-empty"><ScrollText :size="26"/><strong>Log file is empty</strong><span>This experiment completed without writing any log entries.</span></div><template v-else-if="documentView === 'logs'"><div v-if="logPreview.truncated" class="document-note">Showing the most recent log output, limited to 1,000 lines. Download the full log for the complete output.</div><pre>{{ logPreview.text }}</pre></template><pre v-else>{{ detail.config }}</pre></article>
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
import { Activity, ArrowLeft, ArrowRightLeft, BarChart3, BrainCircuit, CalendarDays, CalendarRange, ChartLine, ChartNoAxesCombined, ChevronDown, CircleDollarSign, Clock3, Coins, CopyPlus, Download, FileChartColumn, FileCode2, FlaskConical, Gauge, Layers3, Medal, Plus, ReceiptText, Scale, ScrollText, Search, SlidersHorizontal, Square, TableProperties, Timer, Trash2, TriangleAlert, WalletCards, X } from 'lucide-vue-next'
import { computed, onActivated, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { api, post, query, remove } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import ConfirmationModal from '../components/confirmation-modal.vue'
import ExecutionStatus from '../components/execution-status.vue'
import ToggleField from '../components/toggle-field.vue'
import { consumeResultsOverviewRequest, formatConfiguredDate, formatConfiguredDateTime, formatIntervalLabel, formatResultMetric, instrumentLogoUrl } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const experiments = ref([]), jobs = ref([]), detail = ref(null), selectedId = ref(''), expandedId = ref(''), search = ref(''), statusFilter = ref('All'), loading = ref(false), strategy = ref(0), documentView = ref('')
const experimentsLoading = ref(true)
const experimentsLoadingMore = ref(false)
const experimentsHasMore = ref(true)
const experimentsError = ref('')
const experimentOffset = ref(0)
const loadMoreSentinel = ref(null)
const detailError = ref('')
const deleting = ref(false)
const pendingDelete = ref(null)
const overviewTab = ref('pnl'), overviewFigure = ref(null), overviewLoading = ref(false), overviewError = ref('')
const strategyTab = ref('metrics'), strategyFigure = ref(null), strategyLoading = ref(false), strategyError = ref('')
const overviewOptions = reactive({ normalize: false, drawdown: true, window: 30, bins: 40, unit: 'auto' })
const strategyOptions = reactive({ symbol: '' })
const failedSymbolLogos = ref(new Set())
const orderPages = ref({})
const orderLoadingKeys = reactive(new Set())
const orderErrors = ref({})
const orderBatchSize = 100
const experimentBatchSize = 10
const overviewTabs = [
  { id: 'pnl', label: 'PNL', icon: CircleDollarSign, description: 'Cumulative profit and loss over time for each strategy.' },
  { id: 'cash', label: 'Cash', icon: WalletCards, description: 'Cash balance timeline by strategy and settlement currency.' },
  { id: 'dividends', label: 'Dividends', icon: Coins, description: 'Dividend payments recorded for the experiment symbols.' },
  { id: 'pnl_histogram', label: 'PNL histogram', icon: BarChart3, description: 'Distribution of realized trade PNL across strategies.' },
  { id: 'rolling_returns', label: 'Rolling returns', icon: ChartNoAxesCombined, description: 'Rolling return trend to compare momentum over time.' },
  { id: 'rolling_sharpe', label: 'Rolling Sharpe', icon: Medal, description: 'Risk-adjusted performance through time.' },
  { id: 'trade_duration', label: 'Trade duration', icon: Timer, description: 'Distribution of trade holding periods.' },
  { id: 'trade_pnl', label: 'Trade PNL', icon: ArrowRightLeft, description: 'Per-trade PNL profile for each strategy.' }
]
const strategyTabs = [
  { id: 'metrics', label: 'Metrics', icon: TableProperties, description: 'Every metric stored for this strategy.' },
  { id: 'mae_mfe', label: 'MAE / MFE', icon: Scale, description: 'Maximum adverse and favorable excursion per trade.' },
  { id: 'position_size', label: 'Position size', icon: Layers3, description: 'Position size evolution through time.' },
  { id: 'price', label: 'Trades on price', icon: ChartLine, description: 'Price action with strategy context.' },
  { id: 'orders', label: 'Orders', icon: ReceiptText, description: 'Every submitted order, including fills, cancellations, and execution PNL.' }
]
const strategyPlotIds = new Set(['mae_mfe', 'position_size', 'price'])
let pollTimer, searchTimer, experimentObserver, experimentLoadVersion = 0
let activatedOnce = false
const activeJobs = computed(() => jobs.value.filter(job => job.kind === 'experiment' && ['queued', 'running'].includes(job.status)))
const visibleExperiments = computed(() => statusFilter.value === 'All'
  ? experiments.value
  : experiments.value.filter(item => String(item.status).toLowerCase() === statusFilter.value.toLowerCase()))
const activeRun = computed(() => detail.value?.runs?.[strategy.value])
const activeOrderKey = computed(() => activeRun.value ? `${selectedId.value}:${activeRun.value.strategy_id}` : '')
const activeOrderPage = computed(() => orderPages.value[activeOrderKey.value] || {
  orders: [], total: Number(activeRun.value?.order_count || 0), hasMore: true, initialized: false
})
const ordersLoading = computed(() => orderLoadingKeys.has(activeOrderKey.value))
const ordersError = computed(() => orderErrors.value[activeOrderKey.value] || '')
const experimentDescription = computed(() => String(detail.value?.experiment?.description || '').trim())
const fullLogUrl = computed(() => `/api/experiments/${encodeURIComponent(selectedId.value)}/logs`)
const isStrategyPlot = computed(() => strategyPlotIds.has(strategyTab.value))
const isRecordTable = computed(() => strategyTab.value === 'orders')
const hasOverviewOptions = computed(() => ['pnl', 'pnl_histogram', 'rolling_returns', 'rolling_sharpe', 'trade_duration'].includes(overviewTab.value))
const activeOverviewTab = computed(() => overviewTabs.find(item => item.id === overviewTab.value) || overviewTabs[0])
const activeStrategyTab = computed(() => strategyTabs.find(item => item.id === strategyTab.value) || strategyTabs[0])
const strategyTabLabel = computed(() => strategyTabs.find(item => item.id === strategyTab.value)?.label || '')
const tradedSymbols = computed(() => [...new Set((activeRun.value?.trades || []).map(item => item.symbol))])
const logPreview = computed(() => {
  const text = String(detail.value?.logs || '')
  const lines = text.split(/\r?\n/)
  let preview = lines.slice(-1_000).join('\n')
  const lineTruncated = lines.length > 1_000
  const maxCharacters = 200_000
  const characterTruncated = preview.length > maxCharacters
  if (characterTruncated) preview = preview.slice(-maxCharacters)
  return {
    text: preview,
    truncated: Boolean(detail.value?.logs_truncated) || lineTruncated || characterTruncated
  }
})
const experimentPrimaryMetrics = computed(() => {
  const experiment = detail.value?.experiment || {}
  const metadata = detail.value?.config_metadata || {}
  const primary = experiment.primary_metric_value ?? experiment.best_sharpe
  return [
    { label: experiment.primary_metric_name || 'Sharpe', value: formatResultMetric(primary, experiment.primary_metric_percentage), tone: tone(primary), icon: Medal },
    { label: 'Period', value: period(metadata), tone: '', icon: CalendarRange },
    { label: 'Interval', value: formatIntervalLabel(metadata.interval), tone: '', icon: Timer },
    { label: 'Status', value: experiment.status || 'Unknown', tone: statusTone(experiment.status), icon: Activity }
  ]
})
const experimentContextMetrics = computed(() => {
  const metadata = detail.value?.config_metadata || {}
  const experiment = detail.value?.experiment || {}
  return [
    { label: 'Strategies', value: Number(experiment.n_strategies || detail.value?.runs?.length || 0).toLocaleString(), icon: BrainCircuit },
    { label: 'Symbols', value: Number(metadata.symbols || 0).toLocaleString(), icon: ChartNoAxesCombined },
    { label: 'Started at', value: dateTime(experiment.started_at), icon: CalendarDays },
    { label: 'Duration', value: duration(experiment.started_at, experiment.finished_at), icon: Clock3 }
  ]
})
const headlineMetrics = computed(() => {
  const metrics = activeRun.value?.metrics || {}
  return orderedExperimentMetricKeys(detail.value?.experiment, detail.value?.runs, activeRun.value)
    .filter(key => hasRunMetric(metrics, key))
    .slice(0, 6)
    .map(key => {
      const definition = metricDefinition(key)
      const raw = runMetricValue(metrics, key)
      return metric(
        key === 'pnl' ? 'PNL' : definition?.name || enumLabel(key),
        raw,
        Boolean(definition?.percentage),
        key === 'pnl'
      )
    })
})
const orderColumns = ['symbol', 'datetime', 'type', 'side', 'qty', 'price', 'pnl', 'commission', 'status']
const orderRows = computed(() => activeOrderPage.value.orders
  .map((record) => {
    const order = record.order || {}
    const quantity = order.quantity == null ? Number.NaN : Number(order.quantity)
    const absoluteQuantity = Number.isFinite(quantity) ? Math.abs(quantity) : null
    const rawFillPrice = record.fill_price ?? order.price
    const fillPrice = rawFillPrice == null ? Number.NaN : Number(rawFillPrice)
    const total = Number.isFinite(fillPrice) && absoluteQuantity != null ? fillPrice * absoluteQuantity : null
    const currency = activeRun.value?.base_currency || 'USD'
    return {
      datetime: orderDate(record.timestamp),
      symbol: order.symbol || '—',
      type: enumLabel(order.order_type),
      side: quantity > 0 ? 'Buy' : quantity < 0 ? 'Sell' : '—',
      qty: absoluteQuantity == null ? '—' : absoluteQuantity.toLocaleString(undefined, { maximumFractionDigits: 8 }),
      price: total == null ? '—' : money(total, currency, false),
      pnl: record.pnl == null ? '—' : money(record.pnl, currency, false),
      pnlRaw: record.pnl == null ? null : Number(record.pnl),
      commission: money(record.commission || 0, currency, false),
      status: enumLabel(record.status)
    }
  }))
const tableRows = computed(() => {
  if (strategyTab.value === 'orders') return orderRows.value
  const metrics = activeRun.value?.metrics || {}
  return orderedExperimentMetricKeys(detail.value?.experiment, detail.value?.runs, activeRun.value)
    .filter(key => hasRunMetric(metrics, key))
    .map(key => {
      const definition = metricDefinition(key)
      return {
        metric: key === 'pnl' ? 'PNL' : definition?.name || enumLabel(key),
        value: formatResultMetric(runMetricValue(metrics, key), Boolean(definition?.percentage))
      }
    })
})
const tableColumns = computed(() => strategyTab.value === 'orders' ? orderColumns : Object.keys(tableRows.value[0] || {}))

function metric(labelText, raw, percent, currencyValue = false) {
  return {
    label: labelText,
    raw: Number(raw),
    value: currencyValue
      ? money(raw, activeRun.value?.base_currency || 'USD')
      : formatResultMetric(raw, percent)
  }
}
function metricDefinition(key) { return [...(props.bootstrap?.metrics?.builtin || []), ...(props.bootstrap?.metrics?.saved || [])].find(item => item.key === key) }
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
const metricAliases = {
  max_dd: ['max_drawdown'],
  sharpe: ['sharpe_ratio'],
  total_return: ['return']
}
const canonicalMetricKeys = {
  max_drawdown: 'max_dd',
  return: 'total_return',
  sharpe_ratio: 'sharpe'
}
const metricImportance = [
  'sharpe',
  'total_return',
  'pnl',
  'max_dd',
  'cagr',
  'n_trades',
  'win_rate',
  'sortino',
  'ann_volatility',
  'final_equity',
  'excess_return',
  'alpha'
]
const currencyMetrics = new Set(['avg_loss', 'avg_win', 'best_trade', 'expectancy', 'final_equity', 'pnl', 'worst_trade'])
function money(value, currency = 'USD', signed = true) {
  if (!Number.isFinite(Number(value))) return '—'
  try {
    return new Intl.NumberFormat(undefined, {
      style: 'currency', currency, notation: Math.abs(Number(value)) >= 100_000 ? 'compact' : 'standard',
      maximumFractionDigits: 2, signDisplay: signed ? 'exceptZero' : 'auto'
    }).format(Number(value))
  } catch {
    return Number(value).toLocaleString(undefined, { maximumFractionDigits: 2, signDisplay: signed ? 'exceptZero' : 'auto' })
  }
}
function runMetricValue(metrics, key) {
  if (Object.prototype.hasOwnProperty.call(metrics, key)) return metrics[key]
  const alias = metricAliases[key]?.find(candidate => Object.prototype.hasOwnProperty.call(metrics, candidate))
  return alias ? metrics[alias] : undefined
}
function hasRunMetric(metrics, key) {
  return Object.prototype.hasOwnProperty.call(metrics, key) ||
    Boolean(metricAliases[key]?.some(candidate => Object.prototype.hasOwnProperty.call(metrics, candidate)))
}
function orderedExperimentMetricKeys(experiment, runs = [], run = null) {
  const primary = canonicalMetricKeys[experiment?.primary_metric] || experiment?.primary_metric
  const configured = Array.isArray(experiment?.selected_metrics) && experiment.selected_metrics.length
    ? experiment.selected_metrics.map(key => canonicalMetricKeys[key] || key)
    : [...new Set((runs?.length ? runs : [run])
        .flatMap(experimentRun => Object.keys(experimentRun?.metrics || {}))
        .map(key => canonicalMetricKeys[key] || key))]
        .sort((left, right) => {
          const leftRank = metricImportance.indexOf(left)
          const rightRank = metricImportance.indexOf(right)
          if (leftRank < 0 && rightRank < 0) return left.localeCompare(right)
          if (leftRank < 0) return 1
          if (rightRank < 0) return -1
          return leftRank - rightRank
        })
  return [...new Set([primary, ...configured])].filter(Boolean)
}
function runSummaryMetric(key, run) {
  const metrics = run?.metrics || {}
  const definition = metricDefinition(key)
  const raw = runMetricValue(metrics, key)
  return {
    key,
    label: key === 'pnl' ? 'PNL' : definition?.name || enumLabel(key),
    value: currencyMetrics.has(key)
      ? money(raw, run.base_currency)
      : formatResultMetric(raw, Boolean(definition?.percentage)),
    tone: tone(raw)
  }
}
function runSummaryMetrics(experiment, run) {
  const metrics = run?.metrics || {}
  const ordered = orderedExperimentMetricKeys(experiment, experiment?.runs, run)
  const items = []
  for (const key of ordered) {
    if (key === 'n_trades' || key === 'win_rate') {
      if (items.some(item => item.key === 'trades_win_rate')) continue
      const trades = runMetricValue(metrics, 'n_trades')
      const winRate = runMetricValue(metrics, 'win_rate')
      const rawTone = Number(winRate)
      items.push({
        key: 'trades_win_rate',
        label: 'Trades / win rate',
        value: `${Number.isFinite(Number(trades)) ? Number(trades).toLocaleString() : '—'} / ${formatResultMetric(winRate, true)}`,
        tone: rawTone > 0.5 ? 'positive' : rawTone < 0.5 ? 'negative' : ''
      })
      continue
    }
    items.push(runSummaryMetric(key, run))
  }
  return items
}
const tags = value => Array.isArray(value) ? value : String(value || '').split(',').map(item => item.trim()).filter(Boolean)
function dateTime(value) { return formatConfiguredDateTime(value, props.bootstrap?.display, 'Unknown') }
function duration(start, finish) {
  const seconds = Math.max(0, Number(finish || 0) - Number(start || 0))
  if (!Number.isFinite(seconds)) return '—'
  if (seconds < 60) return `${Math.round(seconds)} s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)} m ${Math.round(seconds % 60)} s`
  return `${Math.floor(seconds / 3600)} h ${Math.round((seconds % 3600) / 60)} m`
}
function period(metadata = {}) {
  if (metadata.full_history) return 'Full history'
  const start = metadata.start_date
  const end = metadata.end_date
  if (start && end) {
    const days = Math.round((new Date(`${end}T00:00:00Z`) - new Date(`${start}T00:00:00Z`)) / 86_400_000) + 1
    return `${formatConfiguredDate(start, props.bootstrap?.display)} → ${formatConfiguredDate(end, props.bootstrap?.display)}${Number.isFinite(days) ? ` (${days}d)` : ''}`
  }
  if (start) return `From ${formatConfiguredDate(start, props.bootstrap?.display)}`
  if (end) return `Until ${formatConfiguredDate(end, props.bootstrap?.display)}`
  return 'Full history'
}
function statusTone(status) {
  const normalized = String(status || '').toLowerCase()
  return normalized === 'success' ? 'positive' : normalized === 'error' ? 'negative' : normalized === 'partial' ? 'warning' : ''
}
function label(value) { if (value === 'pnl') return 'PNL'; return value.replaceAll('_', ' ').replace(/^./, match => match.toUpperCase()) }
function cell(value, column = '') {
  if (/(?:^|_)(?:timestamp|ts|datetime|started_at|finished_at)$/.test(column)) return formatConfiguredDateTime(value, props.bootstrap?.display)
  if (/(?:^|_)(?:date|start_date|end_date)$/.test(column)) return formatConfiguredDate(value, props.bootstrap?.display)
  if (value && typeof value === 'object') return JSON.stringify(value)
  if (typeof value === 'number') return value.toLocaleString(undefined, { maximumFractionDigits: 4 })
  return value ?? '—'
}
function enumLabel(value) { return String(value || '—').replace(/([a-z])([A-Z])/g, '$1 $2') }
function orderDate(value) { return value || null }
function symbolLogo(row) { return row.symbol === '—' ? '' : instrumentLogoUrl(row.symbol, detail.value?.config_metadata?.instrument_type, props.bootstrap?.display?.logokit_api_key) }
function markSymbolLogoFailed(symbol) { failedSymbolLogos.value = new Set(failedSymbolLogos.value).add(symbol) }
function tableColumnClass(column) { return ['quantity', 'qty', 'price', 'pnl', 'commission'].includes(column) || column.endsWith('_price') ? 'number' : '' }
function tableCellClass(row, column) {
  return [
    tableColumnClass(column),
    column === 'side' ? (row.side === 'Buy' ? 'positive order-side' : row.side === 'Sell' ? 'negative order-side' : 'order-side') : '',
    column === 'pnl' ? tone(row.pnlRaw ?? row.pnl) : '',
    column === 'status' && String(row.status).toLowerCase().includes('pending') ? 'warning order-status' : column === 'status' ? 'order-status' : ''
  ]
}
async function load({ append = false } = {}) {
  if (append && (experimentsLoading.value || experimentsLoadingMore.value || !experimentsHasMore.value)) return
  const version = append ? experimentLoadVersion : ++experimentLoadVersion
  const offset = append ? experimentOffset.value : 0
  if (append) experimentsLoadingMore.value = true
  else {
    experimentsLoading.value = true
    experimentsLoadingMore.value = false
    experimentsHasMore.value = true
    experimentsError.value = ''
    experimentOffset.value = 0
  }
  try {
    const batch = await query('/api/experiments', {
      search: search.value,
      offset,
      limit: experimentBatchSize
    })
    if (version !== experimentLoadVersion) return
    experiments.value = append
      ? [...new Map([...experiments.value, ...batch].map(item => [item.id, item])).values()]
      : batch
    experimentOffset.value = offset + batch.length
    experimentsHasMore.value = batch.length === experimentBatchSize
    experimentsError.value = ''
    if (!append) {
      const requested = sessionStorage.getItem('backtide:result-id')
      sessionStorage.removeItem('backtide:result-id')
      if (!selectedId.value && requested) open(requested)
    }
  } catch (error) {
    if (version === experimentLoadVersion) {
      experimentsError.value = error.message
      emit('toast', error.message, 'error')
    }
  } finally {
    if (version === experimentLoadVersion) {
      experimentsLoading.value = false
      experimentsLoadingMore.value = false
    }
  }
}
function loadMoreExperiments() { return load({ append: true }) }
function debouncedLoad() { clearTimeout(searchTimer); searchTimer = setTimeout(load, 250) }
function toggleBreakdown(id) { expandedId.value = expandedId.value === id ? '' : id }
function openDocument(view) { documentView.value = view }
async function open(id) {
  selectedId.value = id
  loading.value = true
  detail.value = null
  overviewFigure.value = null
  strategyFigure.value = null
  detailError.value = ''
  strategy.value = 0
  orderPages.value = {}
  orderErrors.value = {}
  orderLoadingKeys.clear()
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
function backToOverview() {
  selectedId.value = ''
  detail.value = null
  detailError.value = ''
  overviewFigure.value = null
  strategyFigure.value = null
  documentView.value = ''
  orderPages.value = {}
  orderErrors.value = {}
  orderLoadingKeys.clear()
}
function resetToRequestedOverview() {
  if (!consumeResultsOverviewRequest(sessionStorage)) return false
  backToOverview()
  return true
}
async function loadMoreOrders() {
  const key = activeOrderKey.value
  const run = activeRun.value
  if (!key || !run || orderLoadingKeys.has(key)) return
  const current = orderPages.value[key] || { orders: [], hasMore: true, initialized: false }
  if (current.initialized && !current.hasMore) return
  orderLoadingKeys.add(key)
  orderErrors.value = { ...orderErrors.value, [key]: '' }
  try {
    const result = await query(`/api/experiments/${encodeURIComponent(selectedId.value)}/orders`, {
      strategy_id: run.strategy_id,
      offset: current.orders.length,
      limit: orderBatchSize
    })
    orderPages.value = {
      ...orderPages.value,
      [key]: {
        orders: [...current.orders, ...(result.orders || [])],
        total: Number(result.total || 0),
        hasMore: Boolean(result.has_more),
        initialized: true
      }
    }
  } catch (error) {
    orderErrors.value = { ...orderErrors.value, [key]: error.message }
  } finally {
    orderLoadingKeys.delete(key)
  }
}
function loadOrdersOnScroll(event) {
  if (strategyTab.value !== 'orders') return
  const element = event.currentTarget
  if (element.scrollTop + element.clientHeight >= element.scrollHeight - 48) {
    void loadMoreOrders()
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
async function openLiveTrading() {
  try {
    const config = await api(`/api/experiments/${encodeURIComponent(selectedId.value)}/paper-config`)
    sessionStorage.setItem('backtide:paper-config', JSON.stringify(config))
    emit('navigate', 'live')
  } catch (error) { emit('toast', error.message, 'error') }
}
function requestDelete(experiment = detail.value?.experiment) {
  if (!experiment) return
  pendingDelete.value = { id: experiment.id, name: experiment.name }
}
async function destroy() {
  const target = pendingDelete.value
  if (!target) return
  deleting.value = true
  try {
    await remove(`/api/experiments/${target.id}`)
    pendingDelete.value = null
    backToOverview()
    emit('toast', 'Experiment deleted.')
    await load()
  } catch (error) { emit('toast', error.message, 'error') }
  finally { deleting.value = false }
}
async function abort() { await post('/api/experiments/abort'); emit('toast', 'Abort requested.') }
async function pollJobs() { const previous = activeJobs.value.length; jobs.value = await api('/api/jobs'); if (previous && !activeJobs.value.length) await load(); pollTimer = setTimeout(pollJobs, 1500) }
watch(overviewTab, loadOverviewPlot)
watch([strategyTab, strategy], () => {
  strategyOptions.symbol = tradedSymbols.value[0] || ''
  loadStrategyPlot()
  if (strategyTab.value === 'orders') void loadMoreOrders()
})
watch(loadMoreSentinel, (element) => {
  experimentObserver?.disconnect()
  if (element) experimentObserver?.observe(element)
})
onMounted(() => {
  resetToRequestedOverview()
  if ('IntersectionObserver' in globalThis) {
    experimentObserver = new IntersectionObserver((entries) => {
      if (entries.some(entry => entry.isIntersecting)) void loadMoreExperiments()
    }, { rootMargin: '240px 0px' })
  }
  load()
  pollJobs()
})
onActivated(() => {
  resetToRequestedOverview()
  if (activatedOnce) load()
  activatedOnce = true
})
onBeforeUnmount(() => { clearTimeout(pollTimer); clearTimeout(searchTimer); experimentObserver?.disconnect() })
</script>
