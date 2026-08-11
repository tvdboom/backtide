<template>
  <div class="page">
    <section class="page-intro">
      <div><span class="eyebrow">Backtest builder</span><h2>Design an experiment</h2><p>Configure market data, portfolio rules, execution assumptions and strategy logic.</p></div>
      <label class="file-button import-config-button">
        <Upload :size="18" />
        <span><strong>Import config</strong><small>TOML, YAML or JSON</small></span>
        <input type="file" accept=".toml,.yaml,.yml,.json" @change="importConfig" />
      </label>
    </section>
    <form class="panel experiment-builder" novalidate @keydown.enter="preventImplicitSubmit" @submit.prevent="run">
      <div class="tabs" role="tablist">
        <button v-for="(item, index) in tabs" :key="item" type="button" :class="{ active: tab === index }" @click="tab = index"><span>{{ index + 1 }}</span>{{ item }}</button>
      </div>

      <Transition name="form-alert">
        <div v-if="issue" class="form-alert" :class="issue.kind" role="alert">
          <TriangleAlert :size="20" />
          <span>
            <strong>{{ issue.title }}</strong>
            <small>{{ issue.message }}</small>
          </span>
          <button type="button" aria-label="Dismiss experiment warning" @click="dismissIssue">
            <X :size="15" />
          </button>
        </div>
      </Transition>

      <div v-if="tab === 0" class="form-section">
        <div class="section-copy"><h3>Experiment identity</h3><p>Give this research run a recognizable name and context.</p></div>
        <div class="form-grid experiment-identity-grid">
          <label>Name<input id="experiment-name" v-model="config.general.name" maxlength="80" placeholder="Enter a name..." /></label>
          <label>Icon<select id="experiment-icon" v-model="config.general.icon"><option v-for="item in experimentIcons" :key="item.value" :value="item.value">{{ item.value }} {{ item.label }}</option></select></label>
          <label class="wide">Tags<SearchSelect v-model="config.general.tags" :options="[]" :uppercase-custom="false" allow-custom input-id="experiment-tags" label="Experiment tags" placeholder="Type a tag and press Enter…" /></label>
          <label class="wide">Description<textarea v-model="config.general.description" rows="5" placeholder="Add a description..." /></label>
        </div>
      </div>

      <div v-if="tab === 1" class="form-section">
        <div class="section-copy"><h3>Market universe</h3><p>Choose a comparable asset class, time range, and bar resolution.</p></div>
        <div class="segmented wide-control">
          <button v-for="type in enums.instrument_types" :key="type" type="button" :class="{ active: config.data.instrument_type === optionValue('instrument_type', type) }" @click="setInstrumentType(type)"><component :is="instrumentTypeIcon(type)" :size="16" />{{ type }}</button>
        </div>
        <div class="form-grid two">
          <label class="wide symbol-select-field">Symbols<SearchSelect :key="config.data.instrument_type" v-model="config.data.symbols" :options="symbols" :descriptions="symbolNames" :logos="symbolLogos" :selected-logos="selectedSymbolLogos" :loading="loadingInstruments" allow-custom input-id="experiment-symbols" label="Experiment symbols" placeholder="Search symbols or company names…" /><small class="logo-attribution">Selected logos provided by <a href="https://parqet.com/api" target="_blank" rel="noreferrer">Parqet</a>.</small></label>
          <label>Interval<select id="experiment-interval" v-model="config.data.interval"><option v-for="item in enums.intervals" :key="item" :value="optionValue('interval', item)">{{ item }}</option></select></label>
          <label class="toggle-label"><span>Full available history<small>Use the provider's maximum range.</small></span><input v-model="config.data.full_history" type="checkbox" class="toggle" /></label>
          <label v-if="!config.data.full_history">Start date<input id="experiment-start-date" v-model="config.data.start_date" type="date" /></label>
          <label v-if="!config.data.full_history">End date<input id="experiment-end-date" v-model="config.data.end_date" type="date" /></label>
        </div>
      </div>

      <div v-if="tab === 2" class="form-section">
        <div class="section-copy"><h3>Starting portfolio</h3><p>Set the capital base and any positions held before the first bar.</p></div>
        <div class="portfolio-basics">
          <label>Initial cash<input id="experiment-initial-cash" v-model.number="config.portfolio.initial_cash" type="number" min="1" step="1" /></label>
          <div class="field-label">
            <span>Base currency</span>
            <CurrencySelect
              v-model="config.portfolio.base_currency"
              :options="enums.currencies"
              input-id="experiment-base-currency"
            />
          </div>
        </div>
        <div class="starting-positions">
          <div class="starting-positions-heading">
            <span><strong>Starting positions</strong><small>Choose from the symbols in your market universe.</small></span>
            <button type="button" class="secondary" :disabled="!availablePositionSymbols.length" @click="addPosition"><Plus :size="16" /> Add position</button>
          </div>
          <div v-if="!positions.length" class="position-empty">No starting positions. The experiment will begin entirely in cash.</div>
          <div v-for="(position, index) in positions" :key="`${position.symbol}-${index}`" class="position-row">
            <div class="position-field"><span>Symbol</span><InstrumentSelect v-model="position.symbol" :options="positionOptions(index)" :descriptions="symbolNames" :logos="symbolLogos" :label="`Starting position ${index + 1} symbol`" /></div>
            <label>Quantity<input v-model.number="position.quantity" type="number" step="any" /></label>
            <button type="button" class="icon-button danger" :aria-label="position.symbol ? `Remove ${position.symbol} starting position` : 'Remove empty starting position'" @click="removePosition(index)"><Trash2 :size="16" /></button>
          </div>
        </div>
      </div>

      <div v-if="tab === 3" class="form-section">
        <div class="section-copy"><h3>Trading logic</h3><p>Select saved strategies, optional indicators and a benchmark.</p></div>
        <div class="form-grid two">
          <label class="wide">Strategies<SearchSelect v-model="config.strategy.strategies" :options="savedStrategies" :descriptions="strategyOptionDetails" :option-icons="strategyOptionIcons" option-name-first input-id="experiment-strategies" label="Experiment strategies" placeholder="Search saved strategies…" /></label>
          <section v-if="selectedStrategies.length" class="selection-insights wide" aria-label="Selected strategy details">
            <article v-for="item in selectedStrategies" :key="item.name" class="asset-selection-card">
              <header><span class="metric-icon"><LibraryAssetIcon kind="strategy" :builtin="item.builtin" :size="18" /></span><span><strong>{{ item.name }}</strong><small>{{ catalogTypeLabel(item.type) }}</small></span></header>
              <p>{{ item.description }}</p>
              <div v-if="item.required_indicators?.length" class="required-indicators"><strong>Injected indicators</strong><div class="indicator-chip-list"><span v-for="indicator in item.required_indicators" :key="indicator.name" class="indicator-chip" :title="indicator.description"><Shapes :size="14" />{{ indicator.name }}</span></div></div>
            </article>
          </section>
          <label class="wide">Indicators<SearchSelect v-model="config.indicators.indicators" :options="savedIndicators" :descriptions="indicatorOptionDetails" :option-icons="indicatorOptionIcons" option-name-first label="Experiment indicators" placeholder="Search saved indicators…" /></label>
          <section v-if="selectedIndicators.length" class="selection-insights wide" aria-label="Selected indicator details">
            <article v-for="item in selectedIndicators" :key="item.name" class="asset-selection-card compact-card">
              <header><span class="metric-icon"><LibraryAssetIcon kind="indicator" :builtin="item.builtin" :size="18" /></span><span><strong>{{ item.name }}</strong><small>{{ catalogTypeLabel(item.type) }}</small></span></header>
              <p>{{ item.description }}</p>
            </article>
          </section>
          <div class="field-label wide"><span>Benchmark</span><small class="field-help">Compare strategy performance against one stock or ETF over the same period.</small><BenchmarkSelect v-model="config.strategy.benchmark" :options="benchmarkSymbols" :descriptions="symbolNames" :logos="symbolLogos" label="Experiment benchmark" :placeholder="benchmarkPlaceholder" /></div>
        </div>
      </div>

      <div v-if="tab === 4" class="form-section">
        <div class="section-copy"><h3>Performance metrics</h3><p>Choose which built-in and custom metrics to compute, then select the experiment headline.</p></div>
        <div class="form-grid two">
          <label class="wide">Metrics<SearchSelect v-model="config.metrics.metrics" :options="metricOptions" :descriptions="metricOptionDetails" :option-icons="metricOptionIcons" option-name-first input-id="experiment-metrics" label="Experiment metrics" placeholder="Search built-in and custom metrics..." /></label>
          <label>Main metric<select id="experiment-main-metric" v-model="config.metrics.main_metric"><option v-for="key in config.metrics.metrics" :key="key" :value="key">{{ metricLabel(key) }}</option></select><small>The best value appears in the results overview.</small></label>
          <section v-if="selectedMetrics.length" class="selection-insights wide" aria-label="Selected metric details">
            <article v-for="item in selectedMetrics" :key="item.key" class="asset-selection-card compact-card">
              <header><span class="metric-icon"><LibraryAssetIcon kind="metric" :builtin="item.builtin" :size="18" /></span><span><strong>{{ item.name }}</strong><small>{{ item.builtin ? 'Built-in' : 'Custom' }}</small></span></header>
              <p>{{ item.description }}</p>
            </article>
          </section>
        </div>
      </div>

      <div v-if="tab === 5" class="form-section">
        <div class="section-copy"><h3>Execution model</h3><p>Model commissions, slippage, fills and supported order types.</p></div>
        <div class="settings-stack">
          <fieldset class="settings-group">
            <legend>Fees and price impact</legend>
            <div class="form-grid three">
              <label>Commission<select id="experiment-commission-type" v-model="config.exchange.commission_type"><option v-for="item in enums.commission_types" :key="item" :value="optionValue('commission_type', item)">{{ item }}</option></select></label>
              <label v-if="showsPercentageCommission">Commission (%)<input v-model.number="config.exchange.commission_pct" type="number" min="0" step="0.01" /></label>
              <label v-if="showsFixedCommission">Fixed commission<input v-model.number="config.exchange.commission_fixed" type="number" min="0" step="0.01" /></label>
              <label>Slippage (%)<input v-model.number="config.exchange.slippage" type="number" min="0" step="0.01" /></label>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Order handling</legend>
            <div class="form-grid two">
              <label class="toggle-label"><span>Partial fills<small>Allow available volume to fill part of an order.</small></span><input v-model="config.exchange.partial_fills" type="checkbox" class="toggle" /></label>
              <label>Allowed order types<SearchSelect v-model="config.exchange.allowed_order_types" :options="enums.order_types" :descriptions="orderTypeDescriptions" plain-options input-id="experiment-order-types" label="Allowed order types" /></label>
            </div>
          </fieldset>
        </div>
      </div>

      <div v-if="tab === 6" class="form-section">
        <div class="section-copy"><h3>Risk controls</h3><p>Bound leverage, short exposure, concentration and currency handling.</p></div>
        <div class="settings-stack">
          <fieldset class="settings-group">
            <legend>Margin</legend>
            <p>Control leverage, collateral requirements, and margin-limit behavior.</p>
            <div class="form-grid three">
              <label class="toggle-label"><span>Margin trading<small>Allow positions to use borrowed capital.</small></span><input v-model="config.exchange.allow_margin" type="checkbox" class="toggle" /></label>
              <template v-if="config.exchange.allow_margin">
                <label>Maximum leverage<input id="experiment-max-leverage" v-model.number="config.exchange.max_leverage" type="number" min="1" step="0.1" /></label>
                <label>Initial margin (%)<input v-model.number="config.exchange.initial_margin" type="number" min="0" max="100" step="1" /></label>
                <label>Maintenance margin (%)<input v-model.number="config.exchange.maintenance_margin" type="number" min="0" max="100" step="1" /></label>
                <label>Margin interest (% annual)<input v-model.number="config.exchange.margin_interest" type="number" min="0" step="0.1" /></label>
                <label class="toggle-label"><span>Raise on margin limit<small>Abort when an order breaches margin rules.</small></span><input v-model="config.exchange.raise_on_margin_limit" type="checkbox" class="toggle" /></label>
              </template>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Short selling</legend>
            <p>Choose whether short positions are allowed and how violations are handled.</p>
            <div class="form-grid three">
              <label class="toggle-label"><span>Short selling<small>Allow quantities below zero.</small></span><input v-model="config.exchange.allow_short_selling" type="checkbox" class="toggle" /></label>
              <template v-if="config.exchange.allow_short_selling">
                <label>Borrow rate (% annual)<input v-model.number="config.exchange.borrow_rate" type="number" min="0" step="0.1" /></label>
                <label class="toggle-label"><span>Raise on short violation<small>Abort when a disallowed short is submitted.</small></span><input v-model="config.exchange.raise_on_short_violation" type="checkbox" class="toggle" /></label>
              </template>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Exposure and currency</legend>
            <p>Limit position concentration and decide when foreign cash is converted.</p>
            <div class="form-grid three">
              <label>Max position (%)<input id="experiment-max-position" v-model.number="config.exchange.max_position_size" type="number" min="1" max="100" /></label>
              <label>FX conversion<select v-model="config.exchange.conversion_mode"><option v-for="item in enums.conversion_modes" :key="item" :value="item">{{ enumLabel(item) }}</option></select></label>
              <label v-if="config.exchange.conversion_mode === 'HoldUntilThreshold'">Conversion threshold<input v-model.number="config.exchange.conversion_threshold" type="number" min="0" step="100" /></label>
              <label v-if="config.exchange.conversion_mode === 'EndOfPeriod'">Conversion period<select v-model="config.exchange.conversion_period"><option :value="null">Not set</option><option v-for="item in enums.conversion_periods" :key="item" :value="optionValue('conversion_period', item)">{{ item }}</option></select></label>
              <label v-if="config.exchange.conversion_mode === 'CustomInterval'">Custom interval (bars)<input id="experiment-conversion-interval" v-model.number="config.exchange.conversion_interval" type="number" min="1" /></label>
            </div>
          </fieldset>
        </div>
      </div>

      <div v-if="tab === 7" class="form-section">
        <div class="section-copy"><h3>Engine behavior</h3><p>Choose warmup and timing conventions used on every simulation bar.</p></div>
        <fieldset class="settings-group">
          <legend>Simulation timing</legend>
          <div class="form-grid two">
            <label>Warmup bars<input id="experiment-warmup" v-model.number="config.engine.warmup_period" type="number" min="0" /></label>
            <label>Risk-free rate (%)<input v-model.number="config.engine.risk_free_rate" type="number" step="0.1" /></label>
            <label class="toggle-label"><span>Trade on close<small>Fill market orders on the current close.</small></span><input v-model="config.engine.trade_on_close" type="checkbox" class="toggle" /></label>
            <label class="toggle-label"><span>Exclusive orders<small>Keep one active order per symbol.</small></span><input v-model="config.engine.exclusive_orders" type="checkbox" class="toggle" /></label>
            <label class="wide">Empty-bar policy<select v-model="config.engine.empty_bar_policy"><option v-for="item in enums.empty_bar_policies" :key="item" :value="item">{{ enumLabel(item) }}</option></select></label>
          </div>
        </fieldset>
      </div>

      <div class="form-footer">
        <button v-if="tab" type="button" class="secondary" @click="tab--"><ChevronLeft :size="16" /> Back</button>
        <span class="form-spacer" />
        <button v-if="tab < tabs.length - 1" type="button" class="secondary" @click="tab++">Continue <ChevronRight :size="16" /></button>
        <button type="submit" class="primary" :disabled="running"><span v-if="running" class="spinner small" /><Play v-else :size="16" /> {{ running ? 'Launching…' : 'Run experiment' }}</button>
      </div>
    </form>
  </div>
</template>

<script setup>
import {
  ArrowLeftRight,
  Bitcoin,
  Bot,
  Braces,
  ChartCandlestick,
  ChevronLeft,
  ChevronRight,
  Landmark,
  Play,
  Plus,
  Shapes,
  Sigma,
  SquareCode,
  Trash2,
  TriangleAlert,
  Upload,
  X
} from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { post, query } from '../api'
import BenchmarkSelect from '../components/benchmark-select.vue'
import CurrencySelect from '../components/currency-select.vue'
import InstrumentSelect from '../components/instrument-select.vue'
import LibraryAssetIcon from '../components/library-asset-icon.vue'
import SearchSelect from '../components/search-select.vue'
import {
  cloneApiState,
  consumeExperimentDraft,
  experimentOptionValue,
  instrumentLogoUrl,
  selectedInstrumentLogoUrl
} from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const tabs = ['General', 'Market data', 'Portfolio', 'Strategy', 'Metrics', 'Execution', 'Risk', 'Engine']
const experimentIcons = [
  { value: '🧪', label: 'Research' },
  { value: '📈', label: 'Growth' },
  { value: '🎯', label: 'Target' },
  { value: '🚀', label: 'Launch' },
  { value: '💡', label: 'Idea' },
  { value: '🌊', label: 'Trend' },
  { value: '🧭', label: 'Explore' },
  { value: '⚛️', label: 'Model' },
  { value: '🏆', label: 'Champion' },
  { value: '🔬', label: 'Inspect' }
]
const enums = props.bootstrap.enums
const savedDraft = consumeExperimentDraft(sessionStorage)
const config = reactive(cloneApiState(savedDraft || props.bootstrap.defaults))
if (!config.metrics) config.metrics = { metrics: ['total_return', 'final_equity', 'pnl', 'n_trades', 'win_rate', 'cagr', 'ann_volatility', 'sharpe', 'sortino', 'max_dd', 'excess_return', 'alpha'], main_metric: 'sharpe' }
if (!config.general.icon) config.general.icon = experimentIcons[0].value
const optionValue = experimentOptionValue
const tab = ref(0)
const running = ref(false)
const issue = ref(null)
let issueTimer
const instruments = ref([])
const loadingInstruments = ref(false)
const positions = ref(Object.entries(config.portfolio.starting_positions || {}).map(([symbol, quantity]) => ({ symbol, quantity })))
const symbols = computed(() => instruments.value.map(item => item.symbol))
const symbolNames = computed(() => Object.fromEntries(instruments.value.map(item => [item.symbol, item.name])))
const symbolLogos = computed(() => {
  const values = Object.fromEntries(instruments.value.map(item => [
    item.symbol,
    instrumentLogoUrl(item.symbol, item.instrument_type, props.bootstrap.display.logokit_api_key)
  ]))
  for (const symbol of config.data.symbols) {
    if (!values[symbol]) {
      values[symbol] = instrumentLogoUrl(
        symbol,
        config.data.instrument_type,
        props.bootstrap.display.logokit_api_key
      )
    }
  }
  return values
})
const selectedSymbolLogos = computed(() => Object.fromEntries(config.data.symbols.map(symbol => [
  symbol,
  selectedInstrumentLogoUrl(symbol, config.data.instrument_type)
])))
const savedStrategies = computed(() => props.bootstrap.strategies.saved.map(item => item.name))
const savedIndicators = computed(() => props.bootstrap.indicators.saved.map(item => item.name))
const metricCatalog = computed(() => [
  ...(props.bootstrap.metrics?.builtin || []),
  ...(props.bootstrap.metrics?.saved || [])
])
const metricOptions = computed(() => metricCatalog.value.map(item => item.key))
const metricOptionDetails = computed(() => Object.fromEntries(metricCatalog.value.map(item => [item.key, item.builtin ? 'Rust built-in' : 'Custom Python'])))
const metricOptionIcons = computed(() => Object.fromEntries(metricCatalog.value.map(item => [item.key, item.builtin ? Sigma : Braces])))
const strategyOptionDetails = computed(() => Object.fromEntries(
  props.bootstrap.strategies.saved.map(item => [
    item.name, item.builtin ? catalogTypeLabel(item.type) : 'Custom'
  ])))
const indicatorOptionDetails = computed(() => Object.fromEntries(
  props.bootstrap.indicators.saved.map(item => [
    item.name, item.builtin ? catalogTypeLabel(item.type) : 'Custom'
  ])))
const strategyOptionIcons = computed(() => Object.fromEntries(
  props.bootstrap.strategies.saved.map(item => [item.name, item.builtin ? Bot : SquareCode])))
const indicatorOptionIcons = computed(() => Object.fromEntries(
  props.bootstrap.indicators.saved.map(item => [item.name, item.builtin ? Shapes : Braces])))
const selectedStrategies = computed(() => config.strategy.strategies
  .map(name => props.bootstrap.strategies.saved.find(item => item.name === name))
  .filter(Boolean))
const selectedIndicators = computed(() => config.indicators.indicators
  .map(name => props.bootstrap.indicators.saved.find(item => item.name === name))
  .filter(Boolean))
const selectedMetrics = computed(() => config.metrics.metrics
  .map(key => metricCatalog.value.find(item => item.key === key))
  .filter(Boolean))
const benchmarkSymbols = computed(() => ['stocks', 'etf'].includes(config.data.instrument_type) ? symbols.value : [])
const benchmarkPlaceholder = computed(() => benchmarkSymbols.value.length
  ? 'Search or enter a benchmark ticker…'
  : 'Enter an optional benchmark ticker…')
const showsPercentageCommission = computed(() => [
  'Percentage',
  'PercentagePlusFixed'
].includes(config.exchange.commission_type))
const showsFixedCommission = computed(() => [
  'Fixed',
  'PercentagePlusFixed'
].includes(config.exchange.commission_type))
const availablePositionSymbols = computed(() => config.data.symbols.filter(symbol =>
  !positions.value.some(position => position.symbol === symbol)))
const orderTypeDescriptions = computed(() => Object.fromEntries(enums.order_types.map(item => [
  item,
  {
    market: 'Execute at the best available market price.',
    limit: 'Execute only at the chosen price or better.',
    stoploss: 'Trigger a protective market order at the stop price.',
    takeprofit: 'Close a position at the chosen profit target.',
    stoplimit: 'Trigger a limit order when the stop price is reached.',
    stoplosslimit: 'Trigger a protective limit order when the stop price is reached.',
    takeprofitlimit: 'Trigger a closing limit order when the profit target is reached.',
    trailingstop: 'Follow favorable prices and trigger a market order after a reversal.',
    trailingstoplimit: 'Follow favorable prices and trigger a limit order after a reversal.',
    settleposition: 'Close or settle an existing position without opening a new one.',
    cancel: 'Cancel an existing pending order.'
  }[String(item).replaceAll(/[^a-z]/gi, '').toLowerCase()] || 'Control how and when an order may be filled.'
])))

function enumLabel(value) { return String(value).replace(/([a-z])([A-Z])/g, '$1 $2').replace('Na N', 'NaN') }
function catalogTypeLabel(value) {
  return enumLabel(value).replace(/\b(Macd|Rsi|Roc|Rsrs|Sma|Ema|Vwap)\b/g, token => token.toUpperCase())
}
function metricLabel(key) { return metricCatalog.value.find(item => item.key === key)?.name || enumLabel(key) }
const instrumentTypeIcons = {
  Stocks: ChartCandlestick,
  ETF: Landmark,
  Forex: ArrowLeftRight,
  Crypto: Bitcoin
}
function instrumentTypeIcon(type) { return instrumentTypeIcons[type] || ChartCandlestick }
async function loadInstruments() {
  loadingInstruments.value = true
  try {
    const result = await query('/api/instruments', {
      instrument_type: config.data.instrument_type,
      source: 'catalog',
      limit: 1500
    })
    instruments.value = [...result].sort((left, right) => left.symbol.localeCompare(right.symbol))
  } finally {
    loadingInstruments.value = false
  }
}
async function showInstrumentError(error) {
  instruments.value = []
  await showIssue(`Could not load the symbol catalog. ${error.message}`, {
    tab: 1,
    selector: '#experiment-symbols'
  })
}
async function initializeInstruments() {
  try {
    await loadInstruments()
  } catch (error) {
    await showInstrumentError(error)
  }
}
async function setInstrumentType(type) {
  config.data.instrument_type = optionValue('instrument_type', type)
  config.data.symbols = []
  instruments.value = []
  try {
    await loadInstruments()
  } catch (error) {
    await showInstrumentError(error)
  }
}
function addPosition() {
  const symbol = availablePositionSymbols.value[0]
  if (symbol) positions.value.push({ symbol, quantity: 1 })
}
function removePosition(index) { positions.value.splice(index, 1) }
function positionOptions(currentIndex) {
  const current = positions.value[currentIndex]?.symbol
  return config.data.symbols.filter(symbol => symbol === current ||
    !positions.value.some((position, index) => index !== currentIndex && position.symbol === symbol))
}
function parsePositions() {
  return Object.fromEntries(positions.value
    .map(position => [position.symbol, Number(position.quantity)])
    .filter(([symbol, quantity]) => config.data.symbols.includes(symbol) && Number.isFinite(quantity)))
}

function preventImplicitSubmit(event) {
  if (event.defaultPrevented) return
  if (event.target instanceof HTMLInputElement && event.target.type !== 'submit') {
    event.preventDefault()
  }
}

function validationIssue() {
  if (!config.general.name?.trim()) {
    return { tab: 0, selector: '#experiment-name', message: 'Enter a name for this experiment.' }
  }
  if (!config.data.symbols.length) {
    return { tab: 1, selector: '#experiment-symbols', message: 'Select at least one market symbol.' }
  }
  if (!config.data.full_history && !config.data.start_date) {
    return { tab: 1, selector: '#experiment-start-date', message: 'Choose a start date or enable full available history.' }
  }
  if (!config.data.full_history && config.data.end_date && config.data.end_date < config.data.start_date) {
    return { tab: 1, selector: '#experiment-end-date', message: 'The end date must be on or after the start date.' }
  }
  if (!Number.isFinite(config.portfolio.initial_cash) || config.portfolio.initial_cash <= 0) {
    return { tab: 2, selector: '#experiment-initial-cash', message: 'Initial cash must be greater than zero.' }
  }
  if (!config.strategy.strategies.length) {
    return { tab: 3, selector: '#experiment-strategies', message: 'Select at least one strategy.' }
  }
  if (!config.metrics.metrics.length) {
    return { tab: 4, selector: '#experiment-metrics', message: 'Select at least one metric.' }
  }
  if (!config.metrics.metrics.includes(config.metrics.main_metric)) {
    return { tab: 4, selector: '#experiment-main-metric', message: 'Choose a main metric from the selected metrics.' }
  }
  if (!config.exchange.allowed_order_types.length) {
    return { tab: 5, selector: '#experiment-order-types', message: 'Select at least one allowed order type.' }
  }
  if (config.exchange.allow_margin && (!Number.isFinite(config.exchange.max_leverage) || config.exchange.max_leverage < 1)) {
    return { tab: 6, selector: '#experiment-max-leverage', message: 'Maximum leverage must be at least 1.' }
  }
  if (!Number.isFinite(config.exchange.max_position_size) || config.exchange.max_position_size < 1 || config.exchange.max_position_size > 100) {
    return { tab: 6, selector: '#experiment-max-position', message: 'Maximum position size must be between 1% and 100%.' }
  }
  if (config.exchange.conversion_mode === 'CustomInterval' && (!Number.isInteger(config.exchange.conversion_interval) || config.exchange.conversion_interval < 1)) {
    return { tab: 6, selector: '#experiment-conversion-interval', message: 'Enter a custom conversion interval of at least one bar.' }
  }
  if (!Number.isInteger(config.engine.warmup_period) || config.engine.warmup_period < 0) {
    return { tab: 7, selector: '#experiment-warmup', message: 'Warmup bars must be a whole number of zero or greater.' }
  }
  return null
}

function locateIssue(message) {
  const value = String(message || '').toLowerCase()
  const locations = [
    { terms: ['name', 'description', 'tag', 'general'], tab: 0, selector: '#experiment-name' },
    { terms: ['symbol', 'instrument', 'interval', 'date', 'market data'], tab: 1, selector: '#experiment-symbols' },
    { terms: ['cash', 'currency', 'portfolio', 'position'], tab: 2, selector: '#experiment-initial-cash' },
    { terms: ['strategy', 'indicator', 'benchmark'], tab: 3, selector: '#experiment-strategies' },
    { terms: ['commission', 'slippage', 'order', 'partial fill'], tab: 4, selector: '#experiment-order-types' },
    { terms: ['margin', 'short', 'leverage', 'conversion', 'borrow', 'risk'], tab: 5, selector: '#experiment-max-position' },
    { terms: ['warmup', 'empty bar', 'risk-free', 'engine', 'trade on close'], tab: 6, selector: '#experiment-warmup' }
  ]
  return locations.find(location => location.terms.some(term => value.includes(term))) || {
    tab: tab.value,
    selector: '.form-section input, .form-section select, .form-section textarea'
  }
}

async function showIssue(message, location = locateIssue(message), correctable = false) {
  window.clearTimeout(issueTimer)
  tab.value = location.tab
  issue.value = {
    kind: 'error',
    title: `Check ${tabs[location.tab]}`,
    message,
    correctable
  }
  issueTimer = window.setTimeout(dismissIssue, 4500)
  await nextTick()
  const target = document.querySelector(location.selector)
  target?.focus()
  target?.scrollIntoView?.({ block: 'center', behavior: 'smooth' })
}

function dismissIssue() {
  window.clearTimeout(issueTimer)
  issue.value = null
}

function resetExperiment() {
  const instrumentType = config.data.instrument_type
  const defaults = cloneApiState(props.bootstrap.defaults)
  defaults.data.instrument_type = instrumentType
  if (!defaults.general.icon) defaults.general.icon = experimentIcons[0].value
  Object.assign(config, defaults)
  positions.value = []
  tab.value = 0
  dismissIssue()
}

async function run() {
  dismissIssue()
  const invalid = validationIssue()
  if (invalid) {
    await showIssue(invalid.message, invalid, true)
    return
  }
  running.value = true
  try {
    const payload = cloneApiState(config)
    payload.portfolio.starting_positions = parsePositions()
    const job = await post('/api/experiments', payload)
    resetExperiment()
    emit('toast', `Experiment queued · ${job.id}`)
    emit('navigate', 'results')
  } catch (error) { await showIssue(error.message) }
  finally { running.value = false }
}
async function importConfig(event) {
  const file = event.target.files?.[0]
  if (!file) return
  const suffix = file.name.slice(file.name.lastIndexOf('.')).toLowerCase()
  try {
    Object.assign(config, await post('/api/config/parse', { suffix, text: await file.text() }))
    positions.value = Object.entries(config.portfolio.starting_positions || {})
      .filter(([symbol]) => config.data.symbols.includes(symbol))
      .map(([symbol, quantity]) => ({ symbol, quantity }))
    await loadInstruments()
    dismissIssue()
    emit('toast', 'Configuration imported.')
  } catch (error) {
    await showIssue(error.message)
  }
}
watch(() => [...config.data.symbols], selected => {
  positions.value = positions.value.filter(position => selected.includes(position.symbol))
})
watch(() => [...config.metrics.metrics], selected => {
  if (!selected.includes(config.metrics.main_metric)) config.metrics.main_metric = selected[0] || ''
})
watch(config, () => {
  if (!issue.value?.correctable) return
  const invalid = validationIssue()
  if (invalid?.message === issue.value.message) return
  dismissIssue()
}, { deep: true })
onBeforeUnmount(() => window.clearTimeout(issueTimer))
onMounted(initializeInstruments)
</script>
