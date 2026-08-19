<template>
  <div class="page">
    <section class="page-intro">
      <div><h2>Design an experiment</h2><p>Configure market data, portfolio rules, execution assumptions and strategy logic.</p></div>
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
          <label>Name<FieldInfo text="Give the experiment a recognizable name for results and history." /><input id="experiment-name" v-model="config.general.name" maxlength="80" placeholder="Enter a name..." /></label>
          <label>Icon<FieldInfo text="Choose the icon used to identify this experiment in the interface." /><select id="experiment-icon" v-model="config.general.icon"><option v-for="item in experimentIcons" :key="item.value" :value="item.value">{{ item.value }} {{ item.label }}</option></select></label>
          <div class="field-label wide"><span>Tags</span><FieldInfo text="Add searchable labels that help organize related experiments." /><SearchSelect v-model="config.general.tags" :options="[]" :uppercase-custom="false" allow-custom input-id="experiment-tags" label="Experiment tags" placeholder="Type a tag and press Enter…" /></div>
          <label class="wide">Description<FieldInfo text="Record the hypothesis, purpose, or assumptions behind this experiment." /><textarea v-model="config.general.description" rows="5" placeholder="Add a description..." /></label>
        </div>
      </div>

      <div v-if="tab === 1" class="form-section">
        <div class="section-copy"><h3>Market universe</h3><p>Choose a comparable asset class, time range, and bar resolution.</p></div>
        <div class="instrument-type-control">
          <span class="field-control-label">Instrument type<FieldInfo text="Choose the asset class used to build the experiment's market universe." /></span>
          <div class="segmented wide-control">
            <button v-for="type in enums.instrument_types" :key="type" type="button" :class="{ active: config.data.instrument_type === optionValue('instrument_type', type) }" @click="setInstrumentType(type)"><component :is="instrumentTypeIcon(type)" :size="16" />{{ type }}</button>
          </div>
        </div>
        <div class="form-grid two">
          <div class="field-label wide symbol-select-field"><span>Symbols</span><FieldInfo text="Choose the instruments whose historical bars will be used in the experiment." /><SearchSelect :key="config.data.instrument_type" v-model="config.data.symbols" :options="symbols" :descriptions="symbolNames" :logos="symbolLogos" :selected-logos="selectedSymbolLogos" :loading="loadingInstruments" allow-custom input-id="experiment-symbols" label="Experiment symbols" placeholder="Search symbols or company names…" /></div>
          <div class="field-label interval-picker-field">
            <span>Interval</span>
            <FieldInfo text="Set the duration represented by each historical market-data bar." />
            <IntervalPicker v-model="config.data.interval" :options="enums.intervals" :values="intervalValues" input-id="experiment-interval" label="Experiment interval" />
          </div>
          <ToggleField v-model="config.data.full_history" label="Full available history" description="Use the provider's full available range." help="Use every historical bar available from the selected provider." />
          <label v-if="!config.data.full_history">Start date<FieldInfo text="Set the first calendar date included in the experiment." /><input id="experiment-start-date" v-model="config.data.start_date" type="date" /></label>
          <label v-if="!config.data.full_history">End date<FieldInfo text="Set the last calendar date included in the experiment." /><input id="experiment-end-date" v-model="config.data.end_date" type="date" /></label>
        </div>
      </div>

      <div v-if="tab === 2" class="form-section">
        <div class="section-copy"><h3>Starting portfolio</h3><p>Set the capital base and any positions held before the first bar.</p></div>
        <div class="portfolio-basics">
          <label>Initial cash<FieldInfo text="Set the cash balance available to each strategy when the simulation starts." /><input id="experiment-initial-cash" v-model.number="config.portfolio.initial_cash" type="number" min="0" step="100" /></label>
          <div class="field-label currency-picker-field">
            <span>Base currency</span>
            <FieldInfo text="Choose the currency used to value the portfolio and report results." />
            <CurrencySelect
              :model-value="config.portfolio.base_currency"
              :options="enums.currencies"
              input-id="experiment-base-currency"
              @update:model-value="setBaseCurrency"
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
            <div class="position-field"><span>Symbol</span><FieldInfo text="Choose a market-universe symbol to hold before the first bar." /><InstrumentSelect v-model="position.symbol" :options="positionOptions(index)" :descriptions="symbolNames" :logos="symbolLogos" :label="`Starting position ${index + 1} symbol`" /></div>
            <label>Quantity<FieldInfo text="Set the number of units held in this starting position." /><input v-model.number="position.quantity" type="number" step="any" /></label>
            <button type="button" class="icon-button danger" :aria-label="position.symbol ? `Remove ${position.symbol} starting position` : 'Remove empty starting position'" @click="removePosition(index)"><Trash2 :size="16" /></button>
          </div>
        </div>
      </div>

      <div v-if="tab === 3" class="form-section">
        <div class="section-copy"><h3>Trading logic</h3><p>Select saved strategies, optional indicators and a benchmark.</p></div>
        <div class="form-grid two">
          <div class="field-label wide benchmark-field"><span>Benchmark</span><FieldInfo text="Choose a passive reference instrument used to compare experiment performance." /><small class="field-help">Compare performance against a passive benchmark for this asset class.</small><BenchmarkSelect :model-value="config.strategy.benchmark" :options="benchmarkSymbols" :descriptions="symbolNames" :logos="symbolLogos" label="Experiment benchmark" :placeholder="benchmarkPlaceholder" @update:model-value="setBenchmark" /></div>
          <div class="field-label wide"><span>Strategies</span><FieldInfo text="Select the saved trading strategies to run against the same market data." /><SearchSelect v-model="config.strategy.strategies" :options="savedStrategies" :descriptions="strategyOptionDetails" :option-icons="strategyOptionIcons" option-name-first input-id="experiment-strategies" label="Experiment strategies" placeholder="Search saved strategies…" /></div>
          <section v-if="selectedStrategies.length" class="selection-insights wide" aria-label="Selected strategy details">
            <article v-for="item in selectedStrategies" :key="item.name" class="asset-selection-card">
              <header><span class="metric-icon"><LibraryAssetIcon kind="strategy" :builtin="item.builtin" :size="18" /></span><span><strong>{{ item.name }}</strong><small>{{ catalogTypeLabel(item.type) }}</small></span></header>
              <p>{{ item.description }}</p>
              <div v-if="item.required_indicators?.length" class="required-indicators"><strong>Injected indicators</strong><div class="indicator-chip-list"><span v-for="indicator in item.required_indicators" :key="indicator.name" class="indicator-chip" :title="indicator.description"><Shapes :size="14" />{{ indicator.name }}</span></div></div>
            </article>
          </section>
          <div class="field-label wide"><span>Indicators</span><FieldInfo text="Add optional indicators that will be calculated and supplied during the simulation." /><SearchSelect v-model="config.indicators.indicators" :options="savedIndicators" :descriptions="indicatorOptionDetails" :option-icons="indicatorOptionIcons" option-name-first label="Experiment indicators" placeholder="Search saved indicators…" /></div>
          <section v-if="selectedIndicators.length" class="selection-insights wide" aria-label="Selected indicator details">
            <article v-for="item in selectedIndicators" :key="item.name" class="asset-selection-card compact-card">
              <header><span class="metric-icon"><LibraryAssetIcon kind="indicator" :builtin="item.builtin" :size="18" /></span><span><strong>{{ item.name }}</strong><small>{{ catalogTypeLabel(item.type) }}</small></span></header>
              <p>{{ item.description }}</p>
            </article>
          </section>
        </div>
      </div>

      <div v-if="tab === 4" class="form-section">
        <div class="section-copy"><h3>Performance metrics</h3><p>Choose which built-in and custom metrics to compute, drag them into order, then select the experiment headline.</p></div>
        <div class="form-grid two">
          <div class="field-label wide"><span>Metrics</span><FieldInfo text="Choose the performance measures calculated for every strategy result." /><SearchSelect v-model="config.metrics.metrics" :options="metricOptions" :descriptions="metricOptionDetails" :option-icons="metricOptionIcons" option-name-first input-id="experiment-metrics" label="Experiment metrics" placeholder="Search built-in and custom metrics..." /><button type="button" class="text-button metric-clear-button" aria-label="Clear all metrics" :disabled="config.metrics.metrics.length === 0" @click="clearMetrics"><X :size="14" /> Clear all metrics</button></div>
          <label>Main metric<FieldInfo text="Select the headline measure used to rank and summarize experiment results." /><select id="experiment-main-metric" v-model="config.metrics.main_metric"><option v-for="key in config.metrics.metrics" :key="key" :value="key">{{ metricLabel(key) }}</option></select><small>The best value appears in the results overview.</small></label>
          <section v-if="selectedMetrics.length" class="selection-insights metric-selection-list wide" aria-label="Selected metric details">
            <article v-for="(item, index) in selectedMetrics" :key="item.key" class="asset-selection-card compact-card metric-selection-card" :class="{ 'main-metric-card': item.key === config.metrics.main_metric, dragging: draggedMetricKey === item.key, 'drop-target': dragOverMetricKey === item.key }" :draggable="item.key !== config.metrics.main_metric" @dragstart="startMetricDrag($event, item.key)" @dragover.prevent="dragOverMetric(item.key)" @drop.prevent="dropMetric($event, item.key)" @dragend="finishMetricDrag">
              <header>
                <span class="metric-icon"><LibraryAssetIcon kind="metric" :builtin="item.builtin" :size="18" /></span>
                <span><strong>{{ item.name }}</strong><small>{{ item.key === config.metrics.main_metric ? 'Main metric' : item.builtin ? 'Built-in' : 'Custom' }}</small></span>
                <span class="metric-order-actions">
                  <span class="metric-drag-handle" :class="{ disabled: item.key === config.metrics.main_metric }" :title="item.key === config.metrics.main_metric ? 'The main metric stays first' : `Drag ${item.name} to reorder`"><GripVertical :size="16" aria-hidden="true" /></span>
                  <span class="metric-order-number" :aria-label="`Metric position ${index + 1}`">{{ index + 1 }}</span>
                  <button type="button" class="icon-button" :aria-label="`Move ${item.name} up`" :disabled="item.key === config.metrics.main_metric || index <= 1" @click="moveMetric(item.key, -1)"><ChevronUp :size="15" /></button>
                  <button type="button" class="icon-button" :aria-label="`Move ${item.name} down`" :disabled="item.key === config.metrics.main_metric || index === selectedMetrics.length - 1" @click="moveMetric(item.key, 1)"><ChevronDown :size="15" /></button>
                </span>
              </header>
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
              <label>Commission (%)<FieldInfo text="Apply this percentage fee to the value of each simulated fill." /><input id="experiment-commission-pct" v-model.number="config.exchange.commission_pct" type="number" min="0" step="0.01" /></label>
              <label>Fixed commission<FieldInfo text="Apply this fixed cash fee to each simulated fill." /><input id="experiment-commission-fixed" v-model.number="config.exchange.commission_fixed" type="number" min="0" step="0.01" /></label>
              <label>Slippage (%)<FieldInfo text="Move simulated fill prices against the order by this percentage." /><input id="experiment-slippage" v-model.number="config.exchange.slippage" type="number" min="0" step="0.01" /></label>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Order handling</legend>
            <div class="form-grid two">
              <ToggleField v-model="config.exchange.partial_fills" label="Partial fills" description="Limit fills to available bar volume." help="Permit an order to fill only the quantity supported by available market volume." />
              <div class="field-label"><span>Allowed order types</span><FieldInfo text="Choose which simulated order instructions strategies may submit." /><SearchSelect v-model="config.exchange.allowed_order_types" :options="enums.order_types" :descriptions="orderTypeDescriptions" plain-options input-id="experiment-order-types" label="Allowed order types" /></div>
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
              <ToggleField v-model="config.exchange.allow_margin" label="Margin trading" description="Allow positions to use borrowed funds." help="Allow simulated positions to use borrowed funds within the configured margin limits." />
              <template v-if="config.exchange.allow_margin">
                <label>Maximum leverage<FieldInfo text="Limit gross exposure to this multiple of portfolio equity." /><input id="experiment-max-leverage" v-model.number="config.exchange.max_leverage" type="number" min="1" step="0.1" /></label>
                <label>Initial margin (%)<FieldInfo text="Require this percentage of a new leveraged position as opening collateral." /><input v-model.number="config.exchange.initial_margin" type="number" min="0" max="100" step="1" /></label>
                <label>Maintenance margin (%)<FieldInfo text="Require this collateral percentage to keep a leveraged position open." /><input v-model.number="config.exchange.maintenance_margin" type="number" min="0" max="100" step="1" /></label>
                <label>Margin interest (% annual)<FieldInfo text="Charge this annual rate on simulated borrowed cash." /><input v-model.number="config.exchange.margin_interest" type="number" min="0" step="0.1" /></label>
                <ToggleField v-model="config.exchange.raise_on_margin_limit" label="Raise on margin limit" description="Stop the run when a margin limit is hit." help="Stop the experiment with an error when an order exceeds a margin limit." />
              </template>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Short selling</legend>
            <p>Choose whether short positions are allowed and how violations are handled.</p>
            <div class="form-grid three">
              <ToggleField v-model="config.exchange.allow_short_selling" label="Short selling" description="Allow selling assets not currently held." help="Allow strategies to sell assets they do not currently hold." />
              <template v-if="config.exchange.allow_short_selling">
                <label>Borrow rate (% annual)<FieldInfo text="Charge this annual rate on the value of simulated short positions." /><input v-model.number="config.exchange.borrow_rate" type="number" min="0" step="0.1" /></label>
                <ToggleField v-model="config.exchange.raise_on_short_violation" label="Raise on short violation" description="Stop the run on a disallowed short." help="Stop the experiment with an error when a strategy submits a disallowed short order." />
              </template>
            </div>
          </fieldset>
          <fieldset class="settings-group">
            <legend>Exposure and currency</legend>
            <p>Limit position concentration and decide when foreign cash is converted.</p>
            <div class="form-grid three">
              <label>Max position (%)<FieldInfo text="Cap a single position at this percentage of portfolio equity." /><input id="experiment-max-position" v-model.number="config.exchange.max_position_size" type="number" min="1" max="100" /></label>
              <label>FX conversion<FieldInfo text="Choose when non-base-currency cash balances are converted back to the portfolio currency." /><select v-model="config.exchange.conversion_mode"><option v-for="item in enums.conversion_modes" :key="item" :value="item">{{ enumLabel(item) }}</option></select></label>
              <label v-if="config.exchange.conversion_mode === 'HoldUntilThreshold'">Conversion threshold<FieldInfo text="Convert foreign cash after its value reaches this amount." /><input v-model.number="config.exchange.conversion_threshold" type="number" min="0" step="100" /></label>
              <label v-if="config.exchange.conversion_mode === 'EndOfPeriod'">Conversion period<FieldInfo text="Choose the calendar boundary used to convert accumulated foreign cash." /><select v-model="config.exchange.conversion_period"><option :value="null">Not set</option><option v-for="item in enums.conversion_periods" :key="item" :value="optionValue('conversion_period', item)">{{ item }}</option></select></label>
              <label v-if="config.exchange.conversion_mode === 'CustomInterval'">Custom interval (bars)<FieldInfo text="Convert accumulated foreign cash after this number of simulation bars." /><input id="experiment-conversion-interval" v-model.number="config.exchange.conversion_interval" type="number" min="1" /></label>
            </div>
          </fieldset>
        </div>
      </div>

      <div v-if="tab === 7" class="form-section">
        <div class="section-copy"><h3>Engine behavior</h3><p>Choose warmup and timing conventions used on every simulation bar.</p></div>
        <fieldset class="settings-group">
          <legend>Simulation timing</legend>
          <div class="form-grid two">
            <label>Warm-up bars<FieldInfo text="Process this many bars before strategy orders are allowed, giving indicators time to initialize." /><input id="experiment-warmup" v-model.number="config.engine.warmup_period" type="number" min="0" /></label>
            <label>Risk-free rate (%)<FieldInfo text="Set the annual reference return used by risk-adjusted metrics such as Sharpe ratio." /><input v-model.number="config.engine.risk_free_rate" type="number" step="0.1" /></label>
            <ToggleField v-model="config.engine.trade_on_close" label="Trade on close" description="Fill market orders at the current close." help="Fill market orders using the current bar's closing price instead of the next bar." />
            <ToggleField v-model="config.engine.exclusive_orders" label="Exclusive orders" description="Keep one active order per symbol." help="Allow only one active simulated order for each symbol at a time." />
            <label class="wide">Empty-bar policy<FieldInfo text="Choose how the engine handles a missing bar for a symbol at a simulation timestamp." /><select v-model="config.engine.empty_bar_policy"><option v-for="item in enums.empty_bar_policies" :key="item" :value="item">{{ enumLabel(item) }}</option></select></label>
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
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  GripVertical,
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
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { post, query } from '../api'
import BenchmarkSelect from '../components/benchmark-select.vue'
import CurrencySelect from '../components/currency-select.vue'
import FieldInfo from '../components/field-info.vue'
import InstrumentSelect from '../components/instrument-select.vue'
import IntervalPicker from '../components/interval-picker.vue'
import LibraryAssetIcon from '../components/library-asset-icon.vue'
import SearchSelect from '../components/search-select.vue'
import ToggleField from '../components/toggle-field.vue'
import {
  cloneApiState,
  consumeExperimentDraft,
  defaultExperimentBenchmark,
  experimentOptionValue,
  instrumentLogoUrl,
  requestResultsOverview
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
const defaultMetricImportance = [
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
const enums = props.bootstrap.enums
const savedDraft = consumeExperimentDraft(sessionStorage)
const config = reactive(normalizedExperimentConfig(
  savedDraft || props.bootstrap.defaults,
  { prioritizeDefaults: !savedDraft }
))
const optionValue = experimentOptionValue
const intervalValues = Object.fromEntries(
  enums.intervals.map(item => [item, optionValue('interval', item)])
)
const tab = ref(0)
const running = ref(false)
const issue = ref(null)
const draggedMetricKey = ref('')
const dragOverMetricKey = ref('')
const benchmarkIsAutomatic = ref(!savedDraft && !config.strategy.benchmark)
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
  for (const symbol of [...config.data.symbols, config.strategy.benchmark].filter(Boolean)) {
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
  instrumentLogoUrl(
    symbol,
    config.data.instrument_type,
    props.bootstrap.display.logokit_api_key
  )
])))
const savedStrategies = computed(() => props.bootstrap.strategies.saved.map(item => item.name))
const savedIndicators = computed(() => props.bootstrap.indicators.saved.map(item => item.name))
const metricCatalog = computed(() => [
  ...(props.bootstrap.metrics?.builtin || []),
  ...(props.bootstrap.metrics?.saved || [])
])
const metricOptions = computed(() => metricCatalog.value.map(item => item.key))
const metricOptionDetails = computed(() => Object.fromEntries(metricCatalog.value.map(item => [item.key, item.builtin ? 'Built-in' : 'Custom'])))
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
const automaticBenchmark = computed(() => defaultExperimentBenchmark(
  config.portfolio.base_currency,
  config.data.instrument_type,
  symbols.value
))
const benchmarkSymbols = computed(() => config.data.instrument_type === 'forex'
  ? []
  : [...new Set([
      ...symbols.value,
      automaticBenchmark.value,
      config.strategy.benchmark
    ].filter(Boolean))])
const benchmarkPlaceholder = computed(() => benchmarkSymbols.value.length
  ? 'Search or enter a benchmark ticker…'
  : 'Enter an optional benchmark ticker…')
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
function orderedMetricKeys(keys, mainMetric, prioritize = false) {
  const unique = [...new Set((keys || []).filter(Boolean))]
  const remaining = unique.filter(key => key !== mainMetric)
  if (prioritize) {
    const rank = new Map(defaultMetricImportance.map((key, index) => [key, index]))
    remaining.sort((left, right) =>
      (rank.get(left) ?? Number.MAX_SAFE_INTEGER) - (rank.get(right) ?? Number.MAX_SAFE_INTEGER)
    )
  }
  return [...(mainMetric && unique.includes(mainMetric) ? [mainMetric] : []), ...remaining]
}
function normalizedExperimentConfig(value, { prioritizeDefaults = false } = {}) {
  const defaults = cloneApiState(props.bootstrap.defaults)
  const incoming = cloneApiState(value || {})
  for (const [section, sectionValue] of Object.entries(incoming)) {
    defaults[section] = sectionValue && typeof sectionValue === 'object' && !Array.isArray(sectionValue)
      ? { ...(defaults[section] || {}), ...sectionValue }
      : sectionValue
  }
  if (!defaults.metrics) {
    defaults.metrics = { metrics: [...defaultMetricImportance], main_metric: 'sharpe' }
  }
  defaults.metrics.metrics = orderedMetricKeys(
    defaults.metrics.metrics,
    defaults.metrics.main_metric,
    prioritizeDefaults
  )
  if (!defaults.general.icon) defaults.general.icon = experimentIcons[0].value
  defaults.exchange.commission_type = 'PercentagePlusFixed'
  return defaults
}
function catalogTypeLabel(value) {
  return enumLabel(value).replace(/\b(Macd|Rsi|Roc|Rsrs|Sma|Ema|Vwap)\b/g, token => token.toUpperCase())
}
function metricLabel(key) { return metricCatalog.value.find(item => item.key === key)?.name || enumLabel(key) }
function moveMetric(key, direction) {
  const from = config.metrics.metrics.indexOf(key)
  const to = from + direction
  if (from < 1 || to < 1 || to >= config.metrics.metrics.length) return
  const reordered = [...config.metrics.metrics]
  reordered.splice(to, 0, reordered.splice(from, 1)[0])
  config.metrics.metrics = reordered
}
function clearMetrics() {
  finishMetricDrag()
  config.metrics.main_metric = ''
  config.metrics.metrics = []
}
function startMetricDrag(event, key) {
  if (key === config.metrics.main_metric) {
    event.preventDefault()
    return
  }
  draggedMetricKey.value = key
  dragOverMetricKey.value = ''
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', key)
  }
}
function dragOverMetric(key) {
  if (!draggedMetricKey.value || key === draggedMetricKey.value) return
  dragOverMetricKey.value = key
}
function dropMetric(event, targetKey) {
  const sourceKey = draggedMetricKey.value || event.dataTransfer?.getData('text/plain')
  const from = config.metrics.metrics.indexOf(sourceKey)
  const target = config.metrics.metrics.indexOf(targetKey)
  if (from > 0 && target >= 0 && sourceKey !== targetKey) {
    const reordered = [...config.metrics.metrics]
    const [metric] = reordered.splice(from, 1)
    reordered.splice(Math.max(1, target), 0, metric)
    config.metrics.metrics = reordered
  }
  finishMetricDrag()
}
function finishMetricDrag() {
  draggedMetricKey.value = ''
  dragOverMetricKey.value = ''
}
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
  } finally {
    applyAutomaticBenchmark()
  }
}
async function setInstrumentType(type) {
  config.data.instrument_type = optionValue('instrument_type', type)
  config.data.symbols = []
  instruments.value = []
  resetBenchmarkDefault()
  try {
    await loadInstruments()
  } catch (error) {
    await showInstrumentError(error)
  } finally {
    applyAutomaticBenchmark()
  }
}
function applyAutomaticBenchmark() {
  if (benchmarkIsAutomatic.value) config.strategy.benchmark = automaticBenchmark.value
}
function resetBenchmarkDefault() {
  benchmarkIsAutomatic.value = true
  applyAutomaticBenchmark()
}
function setBenchmark(value) {
  benchmarkIsAutomatic.value = false
  config.strategy.benchmark = value
}
function setBaseCurrency(value) {
  config.portfolio.base_currency = value
  resetBenchmarkDefault()
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
    return { tab: 7, selector: '#experiment-warmup', message: 'Warm-up bars must be a whole number of zero or greater.' }
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
  const defaults = normalizedExperimentConfig(props.bootstrap.defaults, { prioritizeDefaults: true })
  defaults.data.instrument_type = instrumentType
  Object.assign(config, defaults)
  benchmarkIsAutomatic.value = true
  applyAutomaticBenchmark()
  positions.value = []
  tab.value = 0
  dismissIssue()
}

async function applyPendingExperimentDraft() {
  const pendingDraft = consumeExperimentDraft(sessionStorage)
  if (!pendingDraft) return false
  const nextConfig = normalizedExperimentConfig(pendingDraft)
  for (const key of Object.keys(config)) {
    if (!(key in nextConfig)) delete config[key]
  }
  Object.assign(config, nextConfig)
  positions.value = Object.entries(config.portfolio.starting_positions || {})
    .filter(([symbol]) => config.data.symbols.includes(symbol))
    .map(([symbol, quantity]) => ({ symbol, quantity }))
  benchmarkIsAutomatic.value = false
  tab.value = 0
  dismissIssue()
  try {
    await loadInstruments()
  } catch (error) {
    await showInstrumentError(error)
  }
  return true
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
    payload.exchange.commission_type = 'PercentagePlusFixed'
    payload.portfolio.starting_positions = parsePositions()
    const job = await post('/api/experiments', payload)
    resetExperiment()
    requestResultsOverview(sessionStorage)
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
    benchmarkIsAutomatic.value = false
    Object.assign(config, await post('/api/config/parse', { suffix, text: await file.text() }))
    config.metrics.metrics = orderedMetricKeys(config.metrics.metrics, config.metrics.main_metric)
    config.exchange.commission_type = 'PercentagePlusFixed'
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
watch(() => config.metrics.main_metric, mainMetric => {
  config.metrics.metrics = orderedMetricKeys(config.metrics.metrics, mainMetric)
})
watch(config, () => {
  if (!issue.value?.correctable) return
  const invalid = validationIssue()
  if (invalid?.message === issue.value.message) return
  dismissIssue()
}, { deep: true })
onBeforeUnmount(() => window.clearTimeout(issueTimer))
onMounted(initializeInstruments)
onActivated(() => { void applyPendingExperimentDraft() })
</script>
