<template>
  <div class="page">
    <section class="page-intro live-intro">
      <div><h2>Paper trading</h2><p>Apply a saved strategy to live bars with simulated fills and no capital at risk.</p></div>
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
      <form class="panel form-section" @submit.prevent="start">
        <div class="panel-header"><div><span class="eyebrow">Session setup</span><h3>Configure paper trading</h3></div><Radio :size="22"/></div>
        <nav class="tabs live-form-tabs" aria-label="Paper trading setup steps">
          <button v-for="(item, index) in setupTabs" :key="item" type="button" :class="{ active: setupTab === index }" @click="setupTab = index"><span>{{ index + 1 }}</span>{{ item }}</button>
        </nav>
        <div class="form-grid two">
          <fieldset v-show="setupTab === 0" class="provider-field wide">
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
          <label v-show="setupTab === 0" class="wide symbol-select-field">Symbols<FieldInfo text="Choose the instruments to subscribe to and monitor during this session." /><SearchSelect v-model="form.symbols" :options="liveSymbols" :descriptions="liveSymbolNames" :logos="liveSymbolLogos" :loading="loadingLiveSymbols" allow-custom input-id="live-symbols" label="Live symbols" placeholder="Search the provider catalog, e.g. BTC-USD…" /><small v-if="liveSymbolError" class="negative">{{ liveSymbolError }}</small></label>
          <label v-show="setupTab === 0" class="live-interval-field">Interval<FieldInfo text="Set the candle duration used for strategy evaluation and monitoring." /><select v-model="form.interval"><option v-for="interval in providerIntervals" :key="interval">{{ interval }}</option></select></label>
          <label v-show="setupTab === 0">Warm-up bars<FieldInfo text="Load this many historical bars before live evaluation so strategies and indicators have enough context." /><input v-model.number="form.warmup_bars" type="number" min="0" max="100000" step="100"/><small>Seeds strategy and indicator history without placing orders.</small></label>
          <label v-show="setupTab === 0">History limit<FieldInfo text="Limit how many recent live bars and events are retained in memory." /><input v-model.number="form.config.max_history" type="number" min="100" max="100000" step="100" /></label>
          <label v-show="setupTab === 2" class="live-cash-field">Initial cash<FieldInfo text="Set the simulated cash balance available when the live session starts." /><input id="live-initial-cash" v-model.number="form.config.initial_cash" type="number" min="0" step="100" /></label>
          <div v-show="setupTab === 2" class="field-label live-base-currency">
            <span>Base currency</span>
            <FieldInfo text="Choose the currency used to value the simulated account and report profit and loss." />
            <CurrencySelect
              :model-value="form.config.base_currency"
              :options="bootstrap.enums.currencies"
              input-id="live-base-currency"
              @update:model-value="setBaseCurrency"
            />
          </div>
          <label v-show="setupTab === 1" class="wide">Strategies<FieldInfo text="Select saved strategies to evaluate; leave this empty for market monitoring only." /><SearchSelect v-model="form.strategies" :options="strategyOptions" input-id="live-strategies" label="Paper strategies" placeholder="Select one or more saved strategies..."/><small>Leave empty to monitor the feed without orders.</small></label>
          <label v-show="setupTab === 1" class="wide">Indicators<FieldInfo text="Add optional indicators to calculate and display during the session." /><SearchSelect v-model="form.indicators" :options="indicatorOptions" input-id="live-indicators" label="Live indicators" placeholder="Select optional dashboard indicators..."/><small>Strategy-required indicators are added automatically.</small></label>
          <label v-show="setupTab === 1" class="wide">Metrics<FieldInfo text="Choose the performance measures updated while the session runs." /><SearchSelect v-model="form.config.metrics" :options="liveMetricOptions" :descriptions="liveMetricDescriptions" input-id="live-metrics" label="Live metrics" placeholder="Select live-compatible metrics..."/></label>
          <label v-show="setupTab === 1">Risk-free rate (%)<FieldInfo text="Set the annual reference return used by risk-adjusted metrics such as Sharpe ratio." /><input v-model.number="form.config.risk_free_rate" type="number" step="0.01"/></label>
          <label v-show="setupTab === 1" class="toggle-label"><span>Trade partial bars</span><FieldInfo text="Evaluate strategies on in-progress candles instead of waiting for each candle to close." /><input v-model="form.config.trade_on_partial" type="checkbox" class="toggle"/></label>
          <label v-show="setupTab === 2">Commission (%)<FieldInfo text="Apply this percentage fee to the value of every simulated fill." /><input v-model.number="form.config.commission_pct" type="number" min="0" step="0.01" /></label>
          <label v-show="setupTab === 2">Fixed commission<FieldInfo text="Apply this fixed cash fee to every simulated fill." /><input v-model.number="form.config.commission_fixed" type="number" min="0" step="0.01" /></label>
          <label v-show="setupTab === 2">Slippage (%)<FieldInfo text="Move simulated fill prices against the order by this percentage." /><input v-model.number="form.config.slippage" type="number" min="0" step="0.01" /></label>
          <label v-show="setupTab === 2" class="toggle-label"><span>Volume-constrained fills</span><FieldInfo text="Restrict simulated fills to a share of the volume reported by each candle." /><input v-model="form.config.partial_fills" type="checkbox" class="toggle"/></label>
          <label v-show="setupTab === 2 && form.config.partial_fills">Max volume participation (%)<FieldInfo text="Set the largest percentage of a candle's volume that one simulated fill may consume." /><input v-model.number="form.config.max_volume_participation" type="number" min="0.01" max="100" step="0.01"/></label>
          <label v-show="setupTab === 2" class="wide">Allowed order types<FieldInfo text="Choose which simulated order instructions strategies may submit." /><SearchSelect v-model="form.config.allowed_order_types" :options="orderTypes" input-id="live-order-types" label="Allowed order types"/></label>
          <label v-show="setupTab === 3" class="toggle-label"><span>Short selling</span><FieldInfo text="Allow strategies to sell assets they do not currently hold." /><input v-model="form.config.allow_short" type="checkbox" class="toggle"/></label>
          <label v-show="setupTab === 3" class="toggle-label"><span>Margin trading</span><FieldInfo text="Allow the simulated account to borrow funds up to the leverage limit." /><input v-model="form.config.allow_margin" type="checkbox" class="toggle"/></label>
          <label v-show="setupTab === 3">Max position (%)<FieldInfo text="Cap a single position at this percentage of simulated account equity." /><input v-model.number="form.config.max_position_size" type="number" min="0.01" max="100" step="0.01"/></label>
          <label v-show="setupTab === 3">Maximum drawdown halt (%)<FieldInfo text="Stop opening new positions when account drawdown reaches this percentage; zero disables the halt." /><input v-model.number="form.config.max_drawdown" type="number" min="0" max="100" step="1"/><small>Zero disables the kill switch.</small></label>
          <label v-show="setupTab === 3 && form.config.allow_margin">Maximum leverage<FieldInfo text="Limit total simulated exposure to this multiple of account equity." /><input v-model.number="form.config.max_leverage" type="number" min="1" step="0.1"/></label>
          <label v-show="setupTab === 3 && form.config.allow_margin">Initial margin (%)<FieldInfo text="Require this percentage of a new leveraged position as opening collateral." /><input v-model.number="form.config.initial_margin" type="number" min="0" max="100" step="1"/></label>
          <label v-show="setupTab === 3 && form.config.allow_margin">Maintenance margin (%)<FieldInfo text="Require this collateral percentage to keep a leveraged position open." /><input v-model.number="form.config.maintenance_margin" type="number" min="0" max="100" step="1"/></label>
          <label v-show="setupTab === 3 && form.config.allow_margin">Margin interest (% annual)<FieldInfo text="Charge this annual rate on simulated borrowed cash." /><input v-model.number="form.config.margin_interest" type="number" min="0" step="0.1"/></label>
          <label v-show="setupTab === 3 && form.config.allow_margin">Short borrow (% annual)<FieldInfo text="Charge this annual rate on the value of simulated short positions." /><input v-model.number="form.config.borrow_rate" type="number" min="0" step="0.1"/></label>
        </div>
        <div class="form-footer"><button v-if="setupTab" type="button" class="secondary" @click="setupTab--"><ChevronLeft :size="16"/> Back</button><span class="form-spacer"/><button v-if="setupTab < setupTabs.length - 1" type="button" class="secondary" @click="setupTab++">Continue <ChevronRight :size="16"/></button><button class="primary live-button" :disabled="starting || !available(form.provider) || !form.symbols.length"><span v-if="starting" class="spinner small"/><Radio v-else :size="16"/> {{ starting ? 'Connecting…' : 'Start live session' }}</button></div>
      </form>
      <aside class="panel safety-panel"><ShieldCheck :size="28"/><h3>Paper mode only</h3><p>Backtide calculates hypothetical fills locally. It does not connect to a brokerage account or submit real orders.</p><ul><li>Real-time provider WebSockets</li><li>Simulated commission and slippage</li><li>Local-only portfolio state</li><li>Bounded event history</li></ul></aside>
    </section>

    <template v-else>
      <section v-if="hasStrategy" class="metric-grid live-metrics">
        <article class="metric-card"><span>Equity</span><strong>{{ money(snapshot.equity) }}</strong><small>{{ snapshot.processed_bars || 0 }} bars processed</small></article>
        <article class="metric-card"><span>Realized P&amp;L</span><strong :class="tone(snapshot.realized_pnl)">{{ money(snapshot.realized_pnl) }}</strong><small>closed positions</small></article>
        <article class="metric-card"><span>Unrealized P&amp;L</span><strong :class="tone(snapshot.unrealized_pnl)">{{ money(snapshot.unrealized_pnl) }}</strong><small>open positions</small></article>
        <article class="metric-card"><span>Open positions</span><strong>{{ Object.keys(snapshot.portfolio?.positions || {}).length }}</strong><small>{{ state.config.provider }} · {{ state.config.interval }}</small></article>
      </section>
      <section v-if="hasStrategy" class="split-grid live-observability">
        <article class="panel">
          <div class="panel-header"><div><span class="eyebrow">Risk telemetry</span><h3>Exposure &amp; controls</h3></div><span v-if="snapshot.trading_halted" class="badge error">HALTED</span></div>
          <dl class="config-summary">
            <div><dt>Gross exposure</dt><dd>{{ money(snapshot.gross_exposure) }}</dd></div>
            <div><dt>Net exposure</dt><dd>{{ money(snapshot.net_exposure) }}</dd></div>
            <div><dt>Leverage</dt><dd>{{ Number(snapshot.leverage || 0).toFixed(2) }}x</dd></div>
            <div><dt>Buying power</dt><dd>{{ money(snapshot.buying_power) }}</dd></div>
            <div><dt>Drawdown</dt><dd>{{ percent(snapshot.drawdown) }}</dd></div>
            <div><dt>Total costs</dt><dd>{{ money(snapshot.total_costs) }}</dd></div>
          </dl>
          <p v-if="snapshot.halt_reason" class="negative">{{ snapshot.halt_reason }}</p>
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
      <section class="live-dashboard">
        <article class="panel live-chart">
          <div class="panel-header"><div><span class="eyebrow">{{ liveChartEyebrow }}</span><h3>{{ liveChartTitle }}</h3></div></div>
          <ChartPanel :figure="liveFigure" :empty-message="liveChartMessage" />
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
      <section v-if="hasStrategy" class="split-grid live-tables">
        <article class="panel table-panel"><div class="panel-header"><div><span class="eyebrow">Portfolio</span><h3>Positions &amp; cash</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Asset</th><th class="number">Amount</th></tr></thead><tbody><tr v-for="(quantity, symbol) in snapshot.portfolio?.positions" :key="symbol"><td><span class="live-asset-cell"><img v-if="symbolLogo(symbol)" :src="symbolLogo(symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ symbol.slice(0, 1) }}</span><strong>{{ symbol }}</strong></span></td><td class="number">{{ quantity }}</td></tr><tr v-for="(amount, currency) in snapshot.portfolio?.cash" :key="currency"><td>{{ currency }} cash</td><td class="number">{{ money(amount) }}</td></tr></tbody></table></div></article>
        <article class="panel table-panel execution-panel"><div class="panel-header"><div><span class="eyebrow">Execution</span><h3>Recent order outcomes</h3></div><small>Up to 12 latest</small></div><div class="data-table-wrap live-execution-table"><table class="data-table"><thead><tr><th>Symbol</th><th>Status</th><th class="number">Fill</th><th class="number">P&amp;L</th></tr></thead><tbody><tr v-for="(fill, index) in fills" :key="fillKey(fill, index)"><td><span class="execution-symbol"><span class="live-asset-cell"><img v-if="symbolLogo(fill.order?.symbol)" :src="symbolLogo(fill.order?.symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(fill.order?.symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ String(fill.order?.symbol || '?').slice(0, 1) }}</span><strong>{{ fill.order?.symbol || '—' }}</strong></span><small v-if="fill.reason" class="execution-reason">{{ fill.reason }}</small></span></td><td><span class="badge" :class="fillStatusClass(fill.status)">{{ fill.status }}</span></td><td class="number">{{ fill.fill_price == null ? '—' : price(fill.fill_price) }}</td><td class="number" :class="tone(fill.realized_pnl)">{{ fill.realized_pnl == null ? '—' : money(fill.realized_pnl) }}</td></tr></tbody></table><div v-if="!fills.length" class="empty-state compact"><p>{{ hasStrategy ? 'Waiting for strategy orders…' : 'Monitoring only · no strategy selected.' }}</p></div></div></article>
      </section>
      <article class="panel event-feed"><div class="panel-header"><div><span class="eyebrow">Live diagnostics</span><h3>Market event feed</h3></div><span>{{ updates.length }} buffered · {{ state.health?.received_events || 0 }} received · {{ state.health?.warmup_bars_loaded || 0 }} warmed</span></div><div class="event-log"><div v-for="(update, index) in [...updates].reverse().slice(0, 50)" :key="index"><time>{{ eventTime(update.market) }}</time><span class="badge" :class="update.market?.is_final ? 'success' : 'neutral'">{{ update.market?.is_final ? 'CLOSED' : 'PARTIAL' }}</span><strong>{{ update.market?.symbol }}</strong><span class="event-close">close {{ update.market?.close }}</span><span class="event-volume">volume {{ update.market?.volume }}</span><span v-if="update.fills?.length" class="positive event-fills">{{ update.fills.length }} fill{{ update.fills.length === 1 ? '' : 's' }}</span></div></div></article>
    </template>
    <div v-if="state.error" class="callout error-state"><TriangleAlert/><span>{{ state.error }}</span></div>
  </div>
</template>

<script setup>
import { ChevronLeft, ChevronRight, Pause, Play, Radio, ShieldCheck, Square, TriangleAlert } from 'lucide-vue-next'
import { computed, onActivated, onBeforeUnmount, onDeactivated, onMounted, reactive, ref } from 'vue'
import { api, post, query } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import CurrencySelect from '../components/currency-select.vue'
import FieldInfo from '../components/field-info.vue'
import SearchSelect from '../components/search-select.vue'
import { configuredPlotlyDateTimeFormat, flattenFills, formatConfiguredTime, instrumentLogoUrl, paperEquitySeries } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'live-status'])
const providers = ['kraken', 'binance', 'coinbase']
const intervals = props.bootstrap.enums.intervals
const state = reactive({ status: 'idle', config: {}, snapshot: {}, updates: [], health: {}, error: null })
const portfolioDefaults = props.bootstrap.defaults?.portfolio || {}
let paperDraft = null
try {
  paperDraft = JSON.parse(sessionStorage.getItem('backtide:paper-config') || 'null')
} catch {
  paperDraft = null
}
sessionStorage.removeItem('backtide:paper-config')
const setupTabs = ['Market data', 'Strategy & metrics', 'Portfolio & execution', 'Risk']
const setupTab = ref(0)
const form = reactive({
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
})
if (paperDraft) {
  Object.assign(form, paperDraft, {
    config: { ...form.config, ...(paperDraft.config || {}) }
  })
}
const strategyOptions = computed(() => (props.bootstrap.strategies?.saved || []).map(item => item.name))
const indicatorOptions = computed(() => (props.bootstrap.indicators?.saved || []).map(item => item.name))
const liveMetricCatalog = computed(() => (props.bootstrap.metrics?.builtin || []).filter(item => !['alpha', 'excess_return'].includes(item.key)))
const liveMetricOptions = computed(() => liveMetricCatalog.value.map(item => item.key))
const liveMetricDescriptions = computed(() => Object.fromEntries(liveMetricCatalog.value.map(item => [item.key, item.description])))
const orderTypes = props.bootstrap.enums.order_types || form.config.allowed_order_types
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
    return activeSession.value ? 'Replay running' : state.status === 'error' ? 'Replay failed' : 'Replay complete'
  }
  return state.status === 'paused' ? 'Session paused' : 'Session live'
})
const updates = computed(() => state.updates || [])
const latestIndicators = computed(() => {
  const indicators = updates.value.at(-1)?.indicators || {}
  const values = []
  for (const [name, symbols] of Object.entries(indicators)) {
    for (const [symbol, outputs] of Object.entries(symbols || {})) {
      const latest = (outputs || []).map(output => Array.isArray(output) ? output.at(-1) : output)
      values.push({ key: `${name}:${symbol}`, name, symbol, value: latest.map(value => Number(value).toLocaleString(undefined, { maximumFractionDigits: 6 })).join(', ') })
    }
  }
  return values
})
const liveMetrics = computed(() => {
  const aggregate = Object.entries(snapshot.value.metrics || {})
  if (aggregate.length) {
    return aggregate.map(([metric, value]) => ({
      key: metric,
      metric,
      label: metricName(metric),
      value
    }))
  }
  return Object.entries(state.strategies || {}).flatMap(([strategy, strategySnapshot]) =>
    Object.entries(strategySnapshot.metrics || {}).map(([metric, value]) => ({
      key: `${strategy}:${metric}`,
      metric,
      label: `${strategy} · ${metricName(metric)}`,
      value
    }))
  )
})
const fills = computed(() => flattenFills(updates.value, 12))
const initialEquity = computed(() => {
  const perStrategy = Number(state.config?.config?.initial_cash ?? form.config.initial_cash) || 0
  const accounts = Math.max(Object.keys(state.strategies || {}).length, 1)
  return perStrategy * accounts
})
const baseCurrency = computed(() => activeSession.value
  ? state.config?.config?.base_currency || form.config.base_currency
  : form.config.base_currency)
const watchlist = computed(() => {
  const prices = snapshot.value.latest_prices || {}
  const symbols = state.config?.symbols?.length ? state.config.symbols : Object.keys(prices)
  return [...new Set(symbols)].map(symbol => ({ symbol, price: prices[symbol] }))
})
const providerIntervals = computed(() => intervals.filter(interval => available(form.provider, interval)))
const equitySeries = computed(() => paperEquitySeries(updates.value).filter(
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
const hasFilledOrder = computed(() => updates.value.some(update =>
  (update.fills || []).some(fill => String(fill.status || '').toLowerCase() === 'filled')
))
const hasTradingActivity = computed(() =>
  hasFilledOrder.value
  || Object.keys(snapshot.value.portfolio?.positions || {}).length > 0
  || Number(snapshot.value.realized_pnl) !== 0
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
    form.symbols = form.symbols.filter(symbol => liveSymbols.value.includes(symbol))
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
function price(value) { return Number(value).toLocaleString(undefined, { maximumFractionDigits: 6 }) }
function percent(value) { return `${(Number(value) * 100 || 0).toFixed(2)}%` }
function metricName(key) { return liveMetricCatalog.value.find(item => item.key === key)?.name || String(key).replaceAll('_', ' ') }
function metricValue(key, value) { return liveMetricCatalog.value.find(item => item.key === key)?.percentage ? percent(value) : Number(value).toLocaleString(undefined, { maximumFractionDigits: 4 }) }
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
function eventTime(market) { const value = market?.received_ts || market?.close_ts; return formatConfiguredTime(value, props.bootstrap?.display, 'now') }
async function start() { starting.value = true; try { updateState(await post('/api/live', form)); emit('toast', 'Paper-trading session started.'); poll() } catch (error) { emit('toast', error.message, 'error') } finally { starting.value = false } }
async function stop() { try { updateState(await post('/api/live/stop')); clearTimeout(timer); emit('toast', 'Paper-trading session stopped.') } catch (error) { emit('toast', error.message, 'error') } }
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
  if (activationCount++) initialize()
})
onDeactivated(() => clearTimeout(timer))
onBeforeUnmount(() => clearTimeout(timer))
</script>
