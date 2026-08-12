<template>
  <div class="page">
    <section class="page-intro live-intro">
      <div><h2>Paper trading</h2><p>Apply a saved strategy to live bars with simulated fills and no capital at risk.</p></div>
      <div v-if="state.status === 'running'" class="session-actions"><span class="status-pill running"><span/> Session live</span><button class="danger secondary" @click="stop"><Square :size="15"/> Stop</button></div>
    </section>

    <section v-if="state.status !== 'running'" class="live-setup">
      <form class="panel form-section" @submit.prevent="start">
        <div class="panel-header"><div><span class="eyebrow">Session setup</span><h3>Configure paper trading</h3></div><Radio :size="22"/></div>
        <div class="form-grid two">
          <fieldset class="provider-field wide">
            <legend>Provider</legend>
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
          <label class="wide">Interval<select v-model="form.interval"><option v-for="interval in providerIntervals" :key="interval">{{ interval }}</option></select></label>
          <label class="wide symbol-select-field">Symbols<SearchSelect v-model="form.symbols" :options="liveSymbols" :descriptions="liveSymbolNames" :logos="liveSymbolLogos" :loading="loadingLiveSymbols" allow-custom input-id="live-symbols" label="Live symbols" placeholder="Search the provider catalog, e.g. BTC-USD…" /><small v-if="liveSymbolError" class="negative">{{ liveSymbolError }}</small></label>
          <label class="live-cash-field">Initial cash<input id="live-initial-cash" v-model.number="form.config.initial_cash" type="number" min="0" step="100" /></label>
          <div class="field-label live-base-currency">
            <span>Base currency</span>
            <CurrencySelect
              :model-value="form.config.base_currency"
              :options="bootstrap.enums.currencies"
              input-id="live-base-currency"
              @update:model-value="setBaseCurrency"
            />
          </div>
          <label class="wide">Strategy<select v-model="form.strategy"><option value="">No strategy · monitor only</option><option v-for="item in bootstrap.strategies.saved" :key="item.name" :value="item.name">{{ item.name }}</option></select></label>
          <label>Commission (%)<input v-model.number="form.config.commission_pct" type="number" min="0" step="0.01" /></label>
          <label>Fixed commission<input v-model.number="form.config.commission_fixed" type="number" min="0" step="0.01" /></label>
          <label>History limit<input v-model.number="form.config.max_history" type="number" min="100" max="100000" step="100" /></label>
          <label>Slippage (%)<input v-model.number="form.config.slippage" type="number" min="0" step="0.01" /></label>
          <label class="toggle-label"><span>Allow short positions<small>Strategy sell orders may open negative positions.</small></span><input v-model="form.config.allow_short" type="checkbox" class="toggle"/></label>
          <label class="toggle-label"><span>Allow margin<small>Permit simulated leverage beyond available cash.</small></span><input v-model="form.config.allow_margin" type="checkbox" class="toggle"/></label>
          <label class="toggle-label"><span>Trade partial bars<small>Evaluate the strategy before the candle closes.</small></span><input v-model="form.config.trade_on_partial" type="checkbox" class="toggle"/></label>
        </div>
        <div class="form-footer"><span class="form-spacer"/><button class="primary live-button" :disabled="starting || !available(form.provider)"><span v-if="starting" class="spinner small"/><Radio v-else :size="16"/> {{ starting ? 'Connecting…' : 'Start live session' }}</button></div>
      </form>
      <aside class="panel safety-panel"><ShieldCheck :size="28"/><h3>Paper mode only</h3><p>Backtide calculates hypothetical fills locally. It does not connect to a brokerage account or submit real orders.</p><ul><li>Real-time provider WebSockets</li><li>Simulated commission and slippage</li><li>Local-only portfolio state</li><li>Bounded event history</li></ul></aside>
    </section>

    <template v-else>
      <section class="metric-grid live-metrics">
        <article class="metric-card"><span>Equity</span><strong>{{ money(snapshot.equity) }}</strong><small>{{ snapshot.processed_bars || 0 }} bars processed</small></article>
        <article class="metric-card"><span>Realized P&amp;L</span><strong :class="tone(snapshot.realized_pnl)">{{ money(snapshot.realized_pnl) }}</strong><small>closed positions</small></article>
        <article class="metric-card"><span>Unrealized P&amp;L</span><strong :class="tone(snapshot.unrealized_pnl)">{{ money(snapshot.unrealized_pnl) }}</strong><small>open positions</small></article>
        <article class="metric-card"><span>Open positions</span><strong>{{ Object.keys(snapshot.portfolio?.positions || {}).length }}</strong><small>{{ state.config.provider }} · {{ state.config.interval }}</small></article>
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
      <section class="split-grid live-tables">
        <article class="panel table-panel"><div class="panel-header"><div><span class="eyebrow">Portfolio</span><h3>Positions &amp; cash</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Asset</th><th class="number">Amount</th></tr></thead><tbody><tr v-for="(quantity, symbol) in snapshot.portfolio?.positions" :key="symbol"><td><span class="live-asset-cell"><img v-if="symbolLogo(symbol)" :src="symbolLogo(symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ symbol.slice(0, 1) }}</span><strong>{{ symbol }}</strong></span></td><td class="number">{{ quantity }}</td></tr><tr v-for="(amount, currency) in snapshot.portfolio?.cash" :key="currency"><td>{{ currency }} cash</td><td class="number">{{ money(amount) }}</td></tr></tbody></table></div></article>
        <article class="panel table-panel execution-panel"><div class="panel-header"><div><span class="eyebrow">Execution</span><h3>Recent order outcomes</h3></div><small>Up to 12 latest</small></div><div class="data-table-wrap live-execution-table"><table class="data-table"><thead><tr><th>Symbol</th><th>Status</th><th class="number">Fill</th><th class="number">P&amp;L</th></tr></thead><tbody><tr v-for="(fill, index) in fills" :key="fillKey(fill, index)"><td><span class="execution-symbol"><span class="live-asset-cell"><img v-if="symbolLogo(fill.order?.symbol)" :src="symbolLogo(fill.order?.symbol)" class="order-symbol-logo" alt="" @error="markSymbolLogoFailed(fill.order?.symbol)"/><span v-else class="order-symbol-logo" aria-hidden="true">{{ String(fill.order?.symbol || '?').slice(0, 1) }}</span><strong>{{ fill.order?.symbol || '—' }}</strong></span><small v-if="fill.reason" class="execution-reason">{{ fill.reason }}</small></span></td><td><span class="badge" :class="fillStatusClass(fill.status)">{{ fill.status }}</span></td><td class="number">{{ fill.fill_price == null ? '—' : price(fill.fill_price) }}</td><td class="number" :class="tone(fill.realized_pnl)">{{ fill.realized_pnl == null ? '—' : money(fill.realized_pnl) }}</td></tr></tbody></table><div v-if="!fills.length" class="empty-state compact"><p>{{ state.config.strategy ? 'Waiting for strategy orders…' : 'Monitoring only · no strategy selected.' }}</p></div></div></article>
      </section>
      <article class="panel event-feed"><div class="panel-header"><div><span class="eyebrow">Live diagnostics</span><h3>Market event feed</h3></div><span>{{ updates.length }} buffered</span></div><div class="event-log"><div v-for="(update, index) in [...updates].reverse().slice(0, 50)" :key="index"><time>{{ eventTime(update.market) }}</time><span class="badge" :class="update.market?.is_final ? 'success' : 'neutral'">{{ update.market?.is_final ? 'CLOSED' : 'PARTIAL' }}</span><strong>{{ update.market?.symbol }}</strong><span class="event-close">close {{ update.market?.close }}</span><span class="event-volume">volume {{ update.market?.volume }}</span><span v-if="update.fills?.length" class="positive event-fills">{{ update.fills.length }} fill{{ update.fills.length === 1 ? '' : 's' }}</span></div></div></article>
    </template>
    <div v-if="state.error" class="callout error-state"><TriangleAlert/><span>{{ state.error }}</span></div>
  </div>
</template>

<script setup>
import { Radio, ShieldCheck, Square, TriangleAlert } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { api, post, query } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import CurrencySelect from '../components/currency-select.vue'
import SearchSelect from '../components/search-select.vue'
import { configuredPlotlyDateTimeFormat, flattenFills, formatConfiguredTime, instrumentLogoUrl, paperEquitySeries } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'live-status'])
const providers = ['kraken', 'binance', 'coinbase']
const intervals = props.bootstrap.enums.intervals
const state = reactive({ status: 'idle', config: {}, snapshot: {}, updates: [], error: null })
const portfolioDefaults = props.bootstrap.defaults?.portfolio || {}
const form = reactive({ provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategy: '', config: { initial_cash: portfolioDefaults.initial_cash ?? 10000, base_currency: portfolioDefaults.base_currency || 'USD', commission_pct: 0.05, commission_fixed: 0, slippage: 0.01, allow_short: false, allow_margin: false, trade_on_partial: false, max_history: 10000 } })
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
const updates = computed(() => state.updates || [])
const fills = computed(() => flattenFills(updates.value, 12))
const initialEquity = computed(() => Number(
  state.config?.config?.initial_cash ?? form.config.initial_cash
) || 0)
const baseCurrency = computed(() => state.status === 'running'
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
const liveChartEyebrow = computed(() => state.config?.strategy
  ? 'Strategy performance'
  : 'WebSocket market data')
const liveChartTitle = computed(() => state.config?.strategy
  ? 'Net paper P&L'
  : 'Live market prices')
const liveChartMessage = computed(() => {
  if (!state.config?.strategy) {
    return marketSeries.value.size ? '' : 'Waiting for the first WebSocket OHLC update…'
  }
  if (!hasTradingActivity.value) return 'Waiting for the strategy’s first filled order…'
  if (!equitySeries.value.length) return 'Waiting for the first account update…'
  return ''
})
const liveFigure = computed(() => {
  if (liveChartMessage.value) return null
  if (!state.config?.strategy) {
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
  emit('live-status', state.status === 'running')
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
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
function eventTime(market) { const value = market?.received_ts || market?.close_ts; return formatConfiguredTime(value, props.bootstrap?.display, 'now') }
async function start() { starting.value = true; try { updateState(await post('/api/live', form)); emit('toast', 'Paper-trading session started.'); poll() } catch (error) { emit('toast', error.message, 'error') } finally { starting.value = false } }
async function stop() { try { updateState(await post('/api/live/stop')); clearTimeout(timer); emit('toast', 'Paper-trading session stopped.') } catch (error) { emit('toast', error.message, 'error') } }
async function poll() { try { updateState(await api('/api/live')) } catch (error) { state.error = error.message } if (state.status === 'running') timer = setTimeout(poll, 1000) }
async function initialize() {
  try {
    updateState(await api('/api/live'))
    if (state.status === 'running') poll()
  } catch (error) {
    state.error = error.message
    emit('toast', error.message, 'error')
  }
}
onMounted(() => {
  initialize()
  loadLiveInstruments()
})
onBeforeUnmount(() => clearTimeout(timer))
</script>
