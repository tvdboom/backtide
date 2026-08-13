<template>
  <div class="page">
    <section class="page-intro live-intro">
      <div><h2>Live trading</h2><p>Apply a saved strategy to live bars with simulated fills and no capital at risk.</p></div>
      <div v-if="sessionVisible" class="session-actions">
        <span class="status-pill" :class="{ running: activeSession }"><span/> {{ sessionStatusLabel }}</span>
        <button v-if="state.status === 'paused'" class="secondary" @click="resume"><Play :size="15"/> Resume</button>
        <button v-else-if="activeSession" class="secondary" @click="pause"><Pause :size="15"/> Pause</button>
        <span v-if="activeSession && hasStrategy" class="action-help"><button class="secondary" @click="cancelAll">Cancel orders</button><span class="action-popover" role="tooltip">Cancel every open simulated order before the next market update.</span></span>
        <span v-if="activeSession && hasStrategy" class="action-help"><button class="secondary" @click="flatten">Flatten</button><span class="action-popover" role="tooltip">Close all simulated positions on the next market update.</span></span>
        <button v-if="activeSession" class="danger secondary" @click="stop"><Square :size="15"/> Stop</button>
      </div>
    </section>

    <section v-if="!sessionVisible" class="live-setup">
      <form class="panel experiment-builder live-builder" @submit.prevent="start">
        <div class="tabs live-form-tabs" role="tablist" aria-label="Live trading setup steps">
          <button v-for="(item, index) in setupTabs" :key="item" type="button" :class="{ active: setupTab === index }" @click="setupTab = index"><span>{{ index + 1 }}</span>{{ item }}</button>
        </div>

        <div v-if="setupTab === 0" class="form-section">
          <div class="section-copy"><h3>Market data</h3><p>Choose the live feed, instruments, and candle interval for this session.</p></div>
          <div class="form-grid two">
            <fieldset class="provider-field wide">
              <legend>Provider<FieldInfo text="Select the exchange WebSocket that supplies live market data." /></legend>
              <div class="provider-logo-select" role="radiogroup" aria-label="Live market data provider">
                <button
                  v-for="provider in providers"
                  :key="provider"
                  type="button"
                  role="radio"
                  :aria-checked="form.provider === provider"
                  :aria-label="`${title(provider)}${providerAvailable(provider) ? '' : ', unavailable'}`"
                  :class="{ selected: form.provider === provider, unavailable: !providerAvailable(provider) }"
                  :disabled="!providerAvailable(provider)"
                  @click="selectProvider(provider)"
                >
                  <img :src="providerLogos[provider]" alt="" />
                  <span v-if="!providerAvailable(provider)">Unavailable</span>
                </button>
              </div>
            </fieldset>
            <label class="wide symbol-select-field">Symbols<FieldInfo text="Choose the instruments to subscribe to and monitor during this session." /><SearchSelect v-model="form.symbols" :options="liveSymbols" :descriptions="liveSymbolNames" :logos="liveSymbolLogos" :loading="loadingLiveSymbols" allow-custom input-id="live-symbols" label="Live symbols" placeholder="Search the provider catalog, e.g. BTC-USD…" /><small v-if="liveSymbolError" class="negative">{{ liveSymbolError }}</small></label>
            <div class="field-label interval-picker-field live-interval-field wide">
              <span>Interval</span>
              <FieldInfo text="Set the candle duration used for strategy evaluation and monitoring." />
              <IntervalPicker v-model="form.interval" :options="providerIntervals" input-id="live-interval" label="Live trading interval" />
            </div>
          </div>
        </div>

        <div v-if="setupTab === 1" class="form-section">
          <div class="section-copy"><h3>Starting portfolio</h3><p>Set the simulated cash balance and reporting currency.</p></div>
          <div class="portfolio-basics live-portfolio-basics">
            <label class="live-cash-field">Initial cash<FieldInfo text="Set the simulated cash balance available when the live session starts." /><input id="live-initial-cash" v-model.number="form.config.initial_cash" type="number" min="0" step="100" /></label>
            <div class="field-label currency-picker-field live-base-currency">
              <span>Base currency</span>
              <FieldInfo text="Choose the currency used to value the simulated account and report profit and loss." />
              <CurrencySelect
                :model-value="form.config.base_currency"
                :options="bootstrap.enums.currencies"
                input-id="live-base-currency"
                @update:model-value="setBaseCurrency"
              />
            </div>
          </div>
        </div>

        <div v-if="setupTab === 2" class="form-section">
          <div class="section-copy"><h3>Trading logic</h3><p>Select saved strategies and optional indicators for the live feed.</p></div>
          <div class="form-grid two">
            <div class="field-label wide"><span>Strategies</span><FieldInfo text="Select saved strategies to evaluate; leave this empty for market monitoring only." /><SearchSelect v-model="form.strategies" :options="strategyOptions" :option-icons="strategyOptionIcons" input-id="live-strategies" label="Live strategies" placeholder="Select one or more saved strategies..."/><small>Leave empty to monitor the feed without orders.</small></div>
            <label class="wide">Indicators<FieldInfo text="Add optional indicators to calculate and display during the session." /><SearchSelect v-model="form.indicators" :options="indicatorOptions" :option-icons="indicatorOptionIcons" input-id="live-indicators" label="Live indicators" placeholder="Select optional indicators..."/><small>Strategy-required indicators are added automatically.</small></label>
          </div>
        </div>

        <div v-if="setupTab === 3" class="form-section">
          <div class="section-copy"><h3>Performance metrics</h3><p>Choose the measures updated while the live session runs.</p></div>
          <div class="form-grid two">
            <label class="wide">Metrics<FieldInfo text="Choose the performance measures updated while the session runs." /><SearchSelect v-model="form.config.metrics" :options="liveMetricOptions" :descriptions="liveMetricDescriptions" input-id="live-metrics" label="Live metrics" placeholder="Select live-compatible metrics..."/></label>
          </div>
        </div>

        <div v-if="setupTab === 4" class="form-section">
          <div class="section-copy"><h3>Execution model</h3><p>Model commissions, slippage, fills, and supported order types.</p></div>
          <div class="settings-stack">
            <fieldset class="settings-group">
              <legend>Fees and price impact</legend>
              <div class="form-grid three">
                <label>Commission (%)<FieldInfo text="Apply this percentage fee to the value of every simulated fill." /><input v-model.number="form.config.commission_pct" type="number" min="0" step="0.01" /></label>
                <label>Fixed commission<FieldInfo text="Apply this fixed cash fee to every simulated fill." /><input v-model.number="form.config.commission_fixed" type="number" min="0" step="0.01" /></label>
                <label>Slippage (%)<FieldInfo text="Move simulated fill prices against the order by this percentage." /><input v-model.number="form.config.slippage" type="number" min="0" step="0.01" /></label>
              </div>
            </fieldset>
            <fieldset class="settings-group">
              <legend>Order handling</legend>
              <div class="form-grid two">
                <ToggleField v-model="form.config.partial_fills" label="Volume-constrained fills" description="Limit fills to available candle volume." help="Restrict simulated fills to a share of the volume reported by each candle." />
                <label v-show="form.config.partial_fills">Max volume participation (%)<FieldInfo text="Set the largest percentage of a candle's volume that one simulated fill may consume." /><input v-model.number="form.config.max_volume_participation" type="number" min="0.01" max="100" step="0.01"/></label>
                <label class="wide">Allowed order types<FieldInfo text="Choose which simulated order instructions strategies may submit." /><SearchSelect v-model="form.config.allowed_order_types" :options="orderTypes" :descriptions="orderTypeDescriptions" plain-options input-id="live-order-types" label="Allowed order types"/></label>
              </div>
            </fieldset>
          </div>
        </div>

        <div v-if="setupTab === 5" class="form-section">
          <div class="section-copy"><h3>Risk controls</h3><p>Bound leverage, short exposure, concentration, and account drawdown.</p></div>
          <div class="settings-stack">
            <fieldset class="settings-group">
              <legend>Exposure</legend>
              <div class="form-grid two">
                <ToggleField v-model="form.config.allow_short" label="Short selling" description="Allow selling assets not currently held." help="Allow strategies to sell assets they do not currently hold." />
                <label>Max position (%)<FieldInfo text="Cap a single position at this percentage of simulated account equity." /><input v-model.number="form.config.max_position_size" type="number" min="0.01" max="100" step="0.01"/></label>
                <label>Maximum drawdown halt (%)<FieldInfo text="Stop opening new positions when account drawdown reaches this percentage; zero disables the halt." /><input v-model.number="form.config.max_drawdown" type="number" min="0" max="100" step="1"/><small>Zero disables the kill switch.</small></label>
              </div>
            </fieldset>
            <fieldset class="settings-group">
              <legend>Margin</legend>
              <div class="form-grid three">
                <ToggleField v-model="form.config.allow_margin" label="Margin trading" description="Allow positions to use borrowed funds." help="Allow the simulated account to borrow funds up to the leverage limit." />
                <template v-if="form.config.allow_margin">
                  <label>Maximum leverage<FieldInfo text="Limit total simulated exposure to this multiple of account equity." /><input v-model.number="form.config.max_leverage" type="number" min="1" step="0.1"/></label>
                  <label>Initial margin (%)<FieldInfo text="Require this percentage of a new leveraged position as opening collateral." /><input v-model.number="form.config.initial_margin" type="number" min="0" max="100" step="1"/></label>
                  <label>Maintenance margin (%)<FieldInfo text="Require this collateral percentage to keep a leveraged position open." /><input v-model.number="form.config.maintenance_margin" type="number" min="0" max="100" step="1"/></label>
                  <label>Margin interest (% annual)<FieldInfo text="Charge this annual rate on simulated borrowed cash." /><input v-model.number="form.config.margin_interest" type="number" min="0" step="0.1"/></label>
                  <label>Short borrow (% annual)<FieldInfo text="Charge this annual rate on the value of simulated short positions." /><input v-model.number="form.config.borrow_rate" type="number" min="0" step="0.1"/></label>
                </template>
              </div>
            </fieldset>
          </div>
        </div>

        <div v-if="setupTab === 6" class="form-section">
          <div class="section-copy"><h3>Engine behavior</h3><p>Choose warm-up, metric assumptions, retention, and candle timing for the live session.</p></div>
          <fieldset class="settings-group">
            <legend>Session processing</legend>
            <div class="form-grid two">
              <label>Warm-up bars<FieldInfo text="Load this many historical bars before live evaluation so strategies and indicators have enough context." /><input v-model.number="form.warmup_bars" type="number" min="0" max="100000" step="100"/><small>Seeds strategy and indicator history without placing orders.</small></label>
              <label>Risk-free rate (%)<FieldInfo text="Set the annual reference return used by risk-adjusted metrics such as Sharpe ratio." /><input v-model.number="form.config.risk_free_rate" type="number" step="0.01"/></label>
              <label>History limit<FieldInfo text="Limit how many recent live bars and events are retained in memory." /><input v-model.number="form.config.max_history" type="number" min="100" max="100000" step="100" /></label>
              <ToggleField v-model="form.config.trade_on_partial" label="Trade partial bars" description="Evaluate strategies before candles close." help="Evaluate strategies on in-progress candles instead of waiting for each candle to close." />
            </div>
          </fieldset>
        </div>

        <div class="form-footer"><button v-if="setupTab" type="button" class="secondary" @click="setupTab--"><ChevronLeft :size="16"/> Back</button><span class="form-spacer"/><button v-if="setupTab < setupTabs.length - 1" type="button" class="secondary" @click="setupTab++">Continue <ChevronRight :size="16"/></button><button type="submit" class="primary live-button" :disabled="starting || !available(form.provider) || !form.symbols.length"><span v-if="starting" class="spinner small"/><Radio v-else :size="16"/> {{ starting ? 'Connecting…' : 'Start live session' }}</button></div>
      </form>
      <aside class="panel safety-panel"><ShieldCheck :size="28"/><h3>Paper mode only</h3><p>Backtide calculates hypothetical fills locally. It does not connect to a brokerage account or submit real orders.</p><ul><li>Real-time provider WebSockets</li><li>Simulated commission and slippage</li><li>Local-only portfolio state</li><li>Bounded event history</li></ul></aside>
    </section>

    <template v-else>
      <section v-if="hasStrategy" class="metric-grid live-metrics">
        <article class="metric-card"><span>Equity</span><strong>{{ money(activeStrategySnapshot.equity) }}</strong><small>{{ activeStrategySnapshot.processed_bars || 0 }} bars processed</small></article>
        <article class="metric-card"><span>Realized P&amp;L</span><strong :class="tone(activeStrategySnapshot.realized_pnl)">{{ money(activeStrategySnapshot.realized_pnl) }}</strong><small>closed positions</small></article>
        <article class="metric-card"><span>Unrealized P&amp;L</span><strong :class="tone(activeStrategySnapshot.unrealized_pnl)">{{ money(activeStrategySnapshot.unrealized_pnl) }}</strong><small>open positions</small></article>
        <article class="metric-card"><span>Open positions</span><strong>{{ Object.keys(activeStrategySnapshot.portfolio?.positions || {}).length }}</strong><small>{{ state.config.provider }} · {{ state.config.interval }}</small></article>
      </section>
      <section class="live-dashboard">
        <article class="panel live-chart">
          <div class="panel-header"><div><span class="eyebrow">{{ liveChartEyebrow }}</span><h3>{{ liveChartTitle }}</h3></div></div>
          <ChartPanel :figure="liveFigure" :empty-message="liveChartMessage" />
          <div v-if="strategyNames.length" class="strategy-switcher live-strategy-switcher" role="tablist" aria-label="Live strategies">
            <button v-for="name in strategyNames" :key="name" type="button" role="tab" :aria-selected="activeStrategyName === name" :class="{ active: activeStrategyName === name }" @click="selectedStrategyName = name"><Bot :size="15" aria-hidden="true"/><span>{{ name }}</span></button>
          </div>
        </article>
        <article class="panel quote-board">
          <div class="panel-header"><div><span class="eyebrow">Latest prices</span><h3>Watchlist</h3></div></div>
          <div v-for="item in watchlist" :key="item.symbol" class="quote-row">
            <img v-if="symbolLogo(item.symbol)" :src="symbolLogo(item.symbol)" class="asset-avatar" alt="" @error="markSymbolLogoFailed(item.symbol)" />
            <span v-else class="asset-avatar" aria-hidden="true">{{ item.symbol.slice(0, 1) }}</span>
            <span><strong>{{ item.symbol }}</strong><small>{{ title(state.config.provider || '') }} · {{ state.config.interval }}</small></span>
            <strong v-if="item.price !== undefined">{{ price(item.price) }}</strong>
            <small v-else class="quote-waiting">Waiting for price</small>
          </div>
          <div v-if="!watchlist.length" class="empty-state compact"><p>Waiting for the first market update…</p></div>
        </article>
      </section>
      <section v-if="hasStrategy" class="split-grid live-observability">
        <article class="panel">
          <div class="panel-header"><div><span class="eyebrow">Risk telemetry</span><h3>Exposure &amp; controls</h3></div><span v-if="activeStrategySnapshot.trading_halted" class="badge error">HALTED</span></div>
          <dl class="config-summary">
            <div><dt>Gross exposure</dt><dd>{{ money(activeStrategySnapshot.gross_exposure) }}</dd></div>
            <div><dt>Net exposure</dt><dd>{{ money(activeStrategySnapshot.net_exposure) }}</dd></div>
            <div><dt>Leverage</dt><dd>{{ Number(activeStrategySnapshot.leverage || 0).toFixed(2) }}x</dd></div>
            <div><dt>Buying power</dt><dd>{{ money(activeStrategySnapshot.buying_power) }}</dd></div>
            <div><dt>Drawdown</dt><dd>{{ percent(activeStrategySnapshot.drawdown) }}</dd></div>
            <div><dt>Total costs</dt><dd>{{ money(activeStrategySnapshot.total_costs) }}</dd></div>
          </dl>
          <p v-if="activeStrategySnapshot.halt_reason" class="negative">{{ activeStrategySnapshot.halt_reason }}</p>
        </article>
        <article class="panel">
          <div class="panel-header"><div><span class="eyebrow">Live monitoring</span><h3>Metrics &amp; indicators</h3></div></div>
          <dl class="config-summary">
            <div v-for="item in liveMetrics" :key="item.key"><dt>{{ item.label }}</dt><dd>{{ metricValue(item.metric, item.value) }}</dd></div>
            <div v-for="item in latestIndicators" :key="item.key"><dt>{{ item.name }} · {{ item.symbol }}</dt><dd>{{ item.value }}</dd></div>
          </dl>
          <p v-if="!liveMetrics.length && !latestIndicators.length">Waiting for enough completed bars...</p>
        </article>
      </section>
      <section v-if="hasStrategy" class="split-grid live-tables">
        <article class="panel table-panel"><div class="panel-header"><div><span class="eyebrow">Portfolio</span><h3>Positions &amp; cash</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Asset</th><th class="number">Amount</th></tr></thead><tbody><tr v-for="(quantity, symbol) in activeStrategySnapshot.portfolio?.positions" :key="symbol"><td><span class="live-asset-cell"><img v-if="symbolLogo(symbol)" :src="symbolLogo(symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ symbol.slice(0, 1) }}</span><strong>{{ symbol }}</strong></span></td><td class="number">{{ quantity }}</td></tr><tr v-for="(amount, currency) in activeStrategySnapshot.portfolio?.cash" :key="currency"><td>{{ currency }} cash</td><td class="number">{{ money(amount) }}</td></tr></tbody></table></div></article>
        <article class="panel table-panel execution-panel"><div class="panel-header"><div><span class="eyebrow">Execution</span><h3>Recent order outcomes</h3></div><small>Up to 12 latest</small></div><div class="data-table-wrap live-execution-table"><table class="data-table"><thead><tr><th>Symbol</th><th>Status</th><th class="number">Fill</th><th class="number">P&amp;L</th></tr></thead><tbody><tr v-for="(fill, index) in fills" :key="fillKey(fill, index)"><td><span class="execution-symbol"><span class="live-asset-cell"><img v-if="symbolLogo(fill.order?.symbol)" :src="symbolLogo(fill.order?.symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(fill.order?.symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ String(fill.order?.symbol || '?').slice(0, 1) }}</span><strong>{{ fill.order?.symbol || '—' }}</strong></span><small v-if="fill.reason" class="execution-reason">{{ fill.reason }}</small></span></td><td><span class="badge" :class="fillStatusClass(fill.status)">{{ fill.status }}</span></td><td class="number">{{ fill.fill_price == null ? '—' : price(fill.fill_price) }}</td><td class="number" :class="tone(fill.realized_pnl)">{{ fill.realized_pnl == null ? '—' : money(fill.realized_pnl) }}</td></tr></tbody></table><div v-if="!fills.length" class="empty-state compact"><p>{{ hasStrategy ? 'Waiting for strategy orders…' : 'Monitoring only · no strategy selected.' }}</p></div></div></article>
      </section>
      <article class="panel event-feed">
        <div class="panel-header"><div><span class="eyebrow">Live diagnostics</span><h3>Market event feed</h3></div><span>{{ updates.length }} buffered · {{ state.health?.received_events || 0 }} received · {{ state.health?.warmup_bars_loaded || 0 }} warmed</span></div>
        <div class="event-log" role="table" aria-label="Latest live market events">
          <div class="event-log-header" role="row">
            <span role="columnheader">Time</span>
            <span role="columnheader">Status</span>
            <span role="columnheader">Symbol</span>
            <span class="event-open" role="columnheader">Open</span>
            <span class="event-high" role="columnheader">High</span>
            <span class="event-low" role="columnheader">Low</span>
            <span class="event-close" role="columnheader">Close</span>
            <span class="event-volume" role="columnheader">Volume</span>
            <span class="event-fills" role="columnheader">Fills</span>
          </div>
          <div v-for="(update, index) in [...updates].reverse().slice(0, 50)" :key="index" class="event-log-row" role="row">
            <time role="cell">{{ eventTime(update) }}</time>
            <span class="badge" :class="update.market?.is_final ? 'success' : 'neutral'" role="cell">{{ update.market?.is_final ? 'CLOSED' : 'PARTIAL' }}</span>
            <span class="event-symbol" role="cell">
              <img v-if="symbolLogo(update.market?.symbol)" :src="symbolLogo(update.market?.symbol)" class="event-symbol-logo" alt="" @error="markSymbolLogoFailed(update.market?.symbol)" />
              <span v-else class="event-symbol-logo" aria-hidden="true">{{ String(update.market?.symbol || '?').slice(0, 1) }}</span>
              <strong>{{ update.market?.symbol || '—' }}</strong>
            </span>
            <span class="event-open" role="cell">{{ update.market?.open ?? '—' }}</span>
            <span class="event-high" role="cell">{{ update.market?.high ?? '—' }}</span>
            <span class="event-low" role="cell">{{ update.market?.low ?? '—' }}</span>
            <span class="event-close" role="cell">{{ update.market?.close ?? '—' }}</span>
            <span class="event-volume" role="cell">{{ update.market?.volume ?? '—' }}</span>
            <span class="event-fills" :class="{ positive: update.fills?.length }" role="cell">
              <template v-if="update.fills?.length">{{ update.fills.length }} fill{{ update.fills.length === 1 ? '' : 's' }}</template>
              <template v-else>—</template>
            </span>
          </div>
        </div>
      </article>
    </template>
    <div v-if="state.error" class="callout error-state"><TriangleAlert/><span>{{ state.error }}</span></div>
  </div>
</template>

<script setup>
import { Bot, Braces, ChevronLeft, ChevronRight, Pause, Play, Radio, Shapes, ShieldCheck, Square, SquareCode, TriangleAlert } from 'lucide-vue-next'
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, reactive, ref } from 'vue'
import { api, post, query } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import CurrencySelect from '../components/currency-select.vue'
import FieldInfo from '../components/field-info.vue'
import IntervalPicker from '../components/interval-picker.vue'
import SearchSelect from '../components/search-select.vue'
import ToggleField from '../components/toggle-field.vue'
import { configuredPlotlyDateTimeFormat, flattenFills, formatConfiguredTimeWithSeconds, instrumentLogoUrl, paperEquitySeries } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'live-status'])
const providers = ['kraken', 'binance', 'coinbase']
const intervals = props.bootstrap.enums.intervals
const state = reactive({ status: 'idle', config: {}, snapshot: {}, updates: [], health: {}, error: null })
const portfolioDefaults = props.bootstrap.defaults?.portfolio || {}
const setupTabs = ['Market data', 'Portfolio', 'Strategy', 'Metrics', 'Execution', 'Risk', 'Engine']
const setupTab = ref(0)
const form = reactive(defaultLiveForm())
applyExperimentDraft()
const strategyOptions = computed(() => (props.bootstrap.strategies?.saved || []).map(item => item.name))
const indicatorOptions = computed(() => (props.bootstrap.indicators?.saved || []).map(item => item.name))
const strategyOptionIcons = computed(() => Object.fromEntries(
  (props.bootstrap.strategies?.saved || [])
    .map(item => [item.name, item.builtin ? Bot : SquareCode])))
const indicatorOptionIcons = computed(() => Object.fromEntries(
  (props.bootstrap.indicators?.saved || [])
    .map(item => [item.name, item.builtin ? Shapes : Braces])))
const liveMetricCatalog = computed(() => (props.bootstrap.metrics?.builtin || []).filter(item => !['alpha', 'excess_return'].includes(item.key)))
const liveMetricOptions = computed(() => liveMetricCatalog.value.map(item => item.key))
const liveMetricDescriptions = computed(() => Object.fromEntries(liveMetricCatalog.value.map(item => [item.key, item.description])))
const orderTypes = props.bootstrap.enums.order_types || form.config.allowed_order_types
const orderTypeDescriptions = computed(() => Object.fromEntries(orderTypes.map(item => [
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
const starting = ref(false)
const failedSymbolLogos = ref(new Set())
const liveInstruments = ref([])
const loadingLiveSymbols = ref(false)
const liveSymbolError = ref('')
const liveSymbols = computed(() => liveInstruments.value.map(item => item.symbol))
const liveSymbolNames = computed(() => Object.fromEntries(liveInstruments.value.map(item => [
  item.symbol,
  item.name || `${title(form.provider)} spot market`
])))
const providerLogos = {
  binance: '/providers/binance.png',
  coinbase: '/providers/coinbase.png',
  kraken: '/providers/kraken.png'
}
const liveSymbolLogos = computed(() => Object.fromEntries([
  ...liveSymbols.value,
  ...form.symbols
].map(symbol => [
  symbol,
  instrumentLogoUrl(symbol, 'Crypto', props.bootstrap.display.logokit_api_key)
])))
let timer
let liveCatalogRequest = 0
const snapshot = computed(() => state.snapshot || {})
const activeSession = computed(() => ['running', 'paused'].includes(state.status))
const sessionVisible = computed(() => activeSession.value || state.config?.mode === 'replay')
const sessionStatusLabel = computed(() => {
  if (state.config?.mode === 'replay') {
    return activeSession.value ? 'Replay running' : state.status === 'error' ? 'Replay failed' : 'Replay'
  }
  return state.status === 'paused' ? 'Session paused' : 'Session live'
})
const updates = computed(() => state.updates || [])
const selectedStrategyName = ref('')
const strategyNames = computed(() => {
  const configured = Array.isArray(state.config?.strategies) && state.config.strategies.length
    ? state.config.strategies
    : state.config?.strategy
      ? [state.config.strategy]
      : Object.keys(state.strategies || {})
  return [...new Set(configured.map(String).filter(name => name && name !== 'Monitor'))]
})
const activeStrategyName = computed(() =>
  strategyNames.value.includes(selectedStrategyName.value)
    ? selectedStrategyName.value
    : strategyNames.value[0] || '')
const activeStrategySnapshot = computed(() =>
  state.strategies?.[activeStrategyName.value] || snapshot.value)
const strategyUpdates = computed(() => updates.value.map(update => {
  const strategyUpdate = update.strategies?.[activeStrategyName.value]
  if (!strategyUpdate) return update
  return {
    ...update,
    fills: strategyUpdate.fills || [],
    indicators: strategyUpdate.indicators || {},
    snapshot: strategyUpdate.snapshot || {}
  }
}))
const latestIndicators = computed(() => {
  const indicators = strategyUpdates.value.at(-1)?.indicators || {}
  const values = []
  for (const [name, symbols] of Object.entries(indicators)) {
    for (const [symbol, outputs] of Object.entries(symbols || {})) {
      const latest = (outputs || []).map(output => Array.isArray(output) ? output.at(-1) : output)
      values.push({ key: `${name}:${symbol}`, name, symbol, value: latest.map(value => Number(value).toLocaleString('en', { maximumFractionDigits: 6 })).join(', ') })
    }
  }
  return values
})
const liveMetrics = computed(() => {
  const configured = Array.isArray(state.config?.config?.metrics)
    ? state.config.config.metrics
    : form.config.metrics
  const order = new Map(configured.map((metric, index) => [metric, index]))
  return Object.entries(activeStrategySnapshot.value.metrics || {})
    .sort(([left], [right]) => {
      const leftIndex = order.get(left) ?? Number.MAX_SAFE_INTEGER
      const rightIndex = order.get(right) ?? Number.MAX_SAFE_INTEGER
      return leftIndex - rightIndex || metricName(left).localeCompare(metricName(right))
    })
    .map(([metric, value]) => ({
      key: metric,
      metric,
      label: metricName(metric),
      value
    }))
})
const fills = computed(() => flattenFills(strategyUpdates.value, 12))
const initialEquity = computed(() =>
  Number(state.config?.config?.initial_cash ?? form.config.initial_cash) || 0)
const baseCurrency = computed(() => activeSession.value
  ? state.config?.config?.base_currency || form.config.base_currency
  : form.config.base_currency)
const watchlist = computed(() => {
  const prices = snapshot.value.latest_prices || {}
  const symbols = state.config?.symbols?.length ? state.config.symbols : Object.keys(prices)
  return [...new Set(symbols)].map(symbol => ({ symbol, price: prices[symbol] }))
})
const providerIntervals = computed(() => intervals.filter(interval => available(form.provider, interval)))
const equitySeries = computed(() => paperEquitySeries(strategyUpdates.value).filter(
  item => item.equity !== null && Number.isFinite(Number(item.equity))
))
const marketSeries = computed(() => {
  const series = new Map()
  for (const update of updates.value) {
    const market = update.market || {}
    const symbol = String(market.symbol || '')
    const timestamp = Number(market.received_ts || market.close_ts)
    if (!symbol || !Number.isFinite(timestamp) || !Number.isFinite(Number(market.close))) continue
    if (!series.has(symbol)) series.set(symbol, [])
    series.get(symbol).push({ ...market, timestamp })
  }
  return series
})
const hasFilledOrder = computed(() => strategyUpdates.value.some(update =>
  (update.fills || []).some(fill => String(fill.status || '').toLowerCase() === 'filled')
))
const hasTradingActivity = computed(() =>
  hasFilledOrder.value
  || Object.keys(activeStrategySnapshot.value.portfolio?.positions || {}).length > 0
  || Number(activeStrategySnapshot.value.realized_pnl) !== 0
  || equitySeries.value.some(item => Number(item.equity) !== initialEquity.value)
)
const hasStrategy = computed(() => Boolean(state.config?.strategy || state.config?.strategies?.length))
const liveChartEyebrow = computed(() => hasStrategy.value
  ? 'Strategy performance'
  : 'WebSocket market data')
const liveChartTitle = computed(() => hasStrategy.value
  ? 'Net paper P&L'
  : 'Live market prices')
const liveChartMessage = computed(() => {
  if (!hasStrategy.value) {
    return marketSeries.value.size ? '' : 'Waiting for the first WebSocket OHLC update…'
  }
  if (!hasTradingActivity.value) return 'Waiting for the strategy’s first filled order…'
  if (!equitySeries.value.length) return 'Waiting for the first account update…'
  return ''
})
const liveFigure = computed(() => {
  if (liveChartMessage.value) return null
  if (!hasStrategy.value) {
    const data = [...marketSeries.value.entries()].map(([symbol, points]) => ({
      type: 'scatter',
      mode: 'lines',
      name: symbol,
      x: points.map(item => new Date(item.timestamp * 1000)),
      y: points.map(item => Number(item.close)),
      customdata: points.map(item => [
        item.open,
        item.high,
        item.low,
        item.close,
        item.volume,
        item.is_final ? 'Closed candle' : 'Partial candle'
      ]),
      line: { width: 2 },
      hovertemplate: [
        '%{customdata[5]}',
        'Open %{customdata[0]:,.6f}',
        'High %{customdata[1]:,.6f}',
        'Low %{customdata[2]:,.6f}',
        'Close %{customdata[3]:,.6f}',
        'Volume %{customdata[4]:,.4f}',
        '<extra>%{fullData.name}</extra>'
      ].join('<br>')
    }))
    return {
      data,
      layout: {
        yaxis: { title: 'Price' },
        xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
        showlegend: data.length > 1
      }
    }
  }
  const currency = baseCurrency.value
  return {
    data: [{
      type: 'scatter',
      mode: 'lines',
      name: 'Net P&L',
      x: equitySeries.value.map(item => new Date(item.timestamp * 1000)),
      y: equitySeries.value.map(item => Number(item.equity) - initialEquity.value),
      line: { color: '#23c483', width: 2 },
      hovertemplate: `${currency} %{y:,.2f}<extra>Net P&L</extra>`
    }],
    layout: {
      yaxis: { title: `Net P&L (${currency})`, zeroline: true },
      xaxis: { title: '', tickformat: configuredPlotlyDateTimeFormat(props.bootstrap.display) },
      showlegend: false
    }
  }
})
function title(value) { return value.charAt(0).toUpperCase() + value.slice(1) }
function defaultLiveForm() {
  return {
    provider: 'kraken',
    interval: '1m',
    symbols: ['BTC-USD'],
    strategies: [],
    indicators: [],
    warmup_bars: 500,
    config: {
      initial_cash: portfolioDefaults.initial_cash ?? 10000,
      base_currency: portfolioDefaults.base_currency || 'USD',
      commission_pct: 0.05,
      commission_fixed: 0,
      slippage: 0.01,
      allow_short: false,
      allow_margin: false,
      trade_on_partial: false,
      max_history: 10000,
      max_leverage: 2,
      initial_margin: 50,
      maintenance_margin: 25,
      margin_interest: 0,
      borrow_rate: 0,
      max_position_size: 100,
      max_drawdown: 0,
      allowed_order_types: ['Market', 'Limit', 'StopLoss', 'TakeProfit', 'StopLossLimit', 'TakeProfitLimit', 'TrailingStop', 'TrailingStopLimit', 'SettlePosition', 'Cancel'],
      partial_fills: false,
      max_volume_participation: 100,
      metrics: ['total_return', 'pnl', 'final_equity', 'n_trades', 'win_rate', 'ann_volatility', 'sharpe', 'sortino', 'max_dd'],
      risk_free_rate: 0
    }
  }
}
function applyExperimentDraft() {
  let draft = null
  try {
    draft = JSON.parse(sessionStorage.getItem('backtide:paper-config') || 'null')
  } catch {
    draft = null
  }
  sessionStorage.removeItem('backtide:paper-config')
  if (!draft || typeof draft !== 'object' || Array.isArray(draft)) return false
  const defaults = defaultLiveForm()
  Object.assign(form, defaults, draft, {
    config: { ...defaults.config, ...(draft.config || {}) }
  })
  setupTab.value = 0
  return true
}
function capability(provider, interval = form.interval) {
  const value = props.bootstrap.live.providers?.[provider]
  if (Array.isArray(value)) return { supported: value[0], reason: value[1] }
  if (!value || typeof value !== 'object') return { supported: false, reason: String(value || '') }
  return value.intervals?.[interval] || value
}
function providerAvailable(provider) {
  const value = props.bootstrap.live.providers?.[provider]
  return Array.isArray(value) ? Boolean(value[0]) : Boolean(value?.supported)
}
function available(provider, interval = form.interval) { return Boolean(capability(provider, interval).supported) }
async function selectProvider(provider) {
  if (!providerAvailable(provider)) return
  form.provider = provider
  form.symbols = []
  if (!available(provider)) {
    form.interval = intervals.find(interval => available(provider, interval)) || form.interval
  }
  await loadLiveInstruments()
}
async function loadLiveInstruments() {
  const request = ++liveCatalogRequest
  const provider = form.provider
  loadingLiveSymbols.value = true
  liveSymbolError.value = ''
  try {
    const result = await query('/api/live/instruments', {
      provider,
      limit: 10000
    })
    if (request !== liveCatalogRequest) return
    liveInstruments.value = [...result].sort((left, right) =>
      left.symbol.localeCompare(right.symbol))
  } catch (error) {
    if (request !== liveCatalogRequest) return
    liveInstruments.value = []
    liveSymbolError.value = `Could not load ${title(provider)} symbols. ${error.message}`
    emit('toast', liveSymbolError.value, 'error')
  } finally {
    if (request === liveCatalogRequest) loadingLiveSymbols.value = false
  }
}
function setBaseCurrency(value) { form.config.base_currency = value }
function updateState(next) {
  Object.assign(state, next)
  emit('live-status', state)
}
function symbolLogo(symbol) {
  if (!symbol || failedSymbolLogos.value.has(symbol)) return ''
  return instrumentLogoUrl(symbol, 'Crypto', props.bootstrap.display.logokit_api_key)
}
function markSymbolLogoFailed(symbol) {
  if (symbol) failedSymbolLogos.value = new Set(failedSymbolLogos.value).add(symbol)
}
function fillStatusClass(status) {
  const value = String(status || '').toLowerCase()
  if (value === 'filled') return 'success'
  if (value === 'rejected') return 'error'
  if (value === 'canceled') return 'partial'
  return 'neutral'
}
function fillKey(fill, index) { return `${fill.order?.id || ''}:${fill.timestamp}:${fill.status}:${index}` }
function money(value) { return new Intl.NumberFormat('en', { style: 'currency', currency: baseCurrency.value || 'USD', maximumFractionDigits: 2 }).format(Number(value) || 0) }
function price(value) { return Number(value).toLocaleString('en', { maximumFractionDigits: 6 }) }
function percent(value) { return `${(Number(value) * 100 || 0).toFixed(2)}%` }
function metricName(key) { return liveMetricCatalog.value.find(item => item.key === key)?.name || String(key).replaceAll('_', ' ') }
function metricValue(key, value) { return liveMetricCatalog.value.find(item => item.key === key)?.percentage ? percent(value) : Number(value).toLocaleString('en', { maximumFractionDigits: 4 }) }
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
function eventTime(update) {
  const value = update?.received_at ?? update?.market?.received_ts ?? update?.market?.close_ts
  return formatConfiguredTimeWithSeconds(value, props.bootstrap?.display, 'now')
}
async function start() { starting.value = true; try { updateState(await post('/api/live', form)); emit('toast', 'Live trading session started.'); poll() } catch (error) { emit('toast', error.message, 'error') } finally { starting.value = false } }
async function stop() { try { updateState(await post('/api/live/stop')); clearTimeout(timer); emit('toast', 'Live trading session stopped.') } catch (error) { emit('toast', error.message, 'error') } }
async function pause() { try { updateState(await post('/api/live/pause')); emit('toast', 'Strategy evaluation paused.') } catch (error) { emit('toast', error.message, 'error') } }
async function resume() { try { updateState(await post('/api/live/resume')); emit('toast', 'Strategy evaluation resumed.'); poll() } catch (error) { emit('toast', error.message, 'error') } }
async function flatten() { try { updateState(await post('/api/live/flatten')); emit('toast', 'Positions will flatten on the next market update.', 'warning') } catch (error) { emit('toast', error.message, 'error') } }
async function cancelAll() { try { updateState(await post('/api/live/cancel-all')); emit('toast', 'Open orders will cancel on the next market update.', 'warning') } catch (error) { emit('toast', error.message, 'error') } }
async function poll() { try { updateState(await api('/api/live')) } catch (error) { state.error = error.message } if (activeSession.value) timer = setTimeout(poll, 1000) }
async function initialize() {
  try {
    updateState(await api('/api/live'))
    if (activeSession.value) poll()
  } catch (error) {
    state.error = error.message
    emit('toast', error.message, 'error')
  }
}
onMounted(() => {
  initialize()
  loadLiveInstruments()
})
let activationCount = 0
onActivated(() => {
  const promoted = applyExperimentDraft()
  if (activationCount++) {
    initialize()
    if (promoted) loadLiveInstruments()
  }
})
onDeactivated(() => clearTimeout(timer))
onBeforeUnmount(() => clearTimeout(timer))
</script>
