<template>
  <div class="page">
    <section class="page-intro live-intro">
      <div><span class="eyebrow live-label"><span /> WebSocket market data</span><h2>Paper trading</h2><p>Apply a saved strategy to live bars with simulated fills and no capital at risk.</p></div>
      <div v-if="state.status === 'running'" class="session-actions"><span class="status-pill running"><span/> Session live</span><button class="danger secondary" @click="stop"><Square :size="15"/> Stop</button></div>
    </section>

    <section v-if="state.status !== 'running'" class="live-setup">
      <form class="panel form-section" @submit.prevent="start">
        <div class="panel-header"><div><span class="eyebrow">Session setup</span><h3>Configure paper trading</h3></div><Radio :size="22"/></div>
        <div class="form-grid two">
          <label>Provider<select v-model="form.provider"><option v-for="provider in providers" :key="provider" :value="provider" :disabled="!available(provider)">{{ title(provider) }}{{ available(provider) ? '' : ' — unavailable' }}</option></select><small>{{ support(form.provider) }}</small></label>
          <label>Interval<select v-model="form.interval"><option v-for="interval in intervals" :key="interval">{{ interval }}</option></select></label>
          <label class="wide">Symbols<SearchSelect v-model="form.symbols" :options="liveSymbols" allow-custom placeholder="Enter a provider symbol, e.g. BTC-USD…" /></label>
          <label>Strategy<select v-model="form.strategy"><option value="">No strategy · monitor only</option><option v-for="item in bootstrap.strategies.saved" :key="item.name" :value="item.name">{{ item.name }}</option></select></label>
          <label>Initial cash<input id="live-initial-cash" v-model.number="form.config.initial_cash" type="number" min="0" step="0.01" /></label>
          <label>Base currency<input v-model="form.config.base_currency" maxlength="4" /></label>
          <label>Commission (%)<input v-model.number="form.config.commission_pct" type="number" min="0" step="0.01" /></label>
          <label>Fixed commission<input v-model.number="form.config.commission_fixed" type="number" min="0" step="0.01" /></label>
          <label>Slippage (%)<input v-model.number="form.config.slippage" type="number" min="0" step="0.01" /></label>
          <label>History limit<input v-model.number="form.config.max_history" type="number" min="100" max="100000" step="100" /></label>
          <label class="toggle-label"><span>Allow short positions<small>Strategy sell orders may open negative positions.</small></span><input v-model="form.config.allow_short" type="checkbox" class="toggle"/></label>
          <label class="toggle-label"><span>Allow margin<small>Permit simulated leverage beyond available cash.</small></span><input v-model="form.config.allow_margin" type="checkbox" class="toggle"/></label>
          <label class="toggle-label"><span>Trade partial bars<small>Evaluate the strategy before the candle closes.</small></span><input v-model="form.config.trade_on_partial" type="checkbox" class="toggle"/></label>
        </div>
        <div class="provider-grid">
          <div v-for="provider in providers" :key="provider" :class="{ unavailable: !available(provider) }"><span :class="{ online: available(provider) }"/><strong>{{ title(provider) }}</strong><small>{{ support(provider) }}</small></div>
        </div>
        <div class="form-footer"><span class="form-spacer"/><button class="primary live-button" :disabled="starting || !available(form.provider)"><span v-if="starting" class="spinner small"/><Radio v-else :size="16"/> {{ starting ? 'Connecting…' : 'Start live session' }}</button></div>
      </form>
      <aside class="panel safety-panel"><ShieldCheck :size="28"/><h3>Paper mode only</h3><p>Backtide calculates hypothetical fills locally. It does not connect to a brokerage account or submit real orders.</p><ul><li>Real-time provider WebSockets</li><li>Simulated commission and slippage</li><li>Local-only portfolio state</li><li>Bounded event history</li></ul><div class="callout"><Info :size="17"/><span>Yahoo Finance has no supported WebSocket feed. Choose Kraken, Binance, or Coinbase.</span></div></aside>
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
          <div class="panel-header"><div><span class="eyebrow">Streaming performance</span><h3>Paper equity</h3></div><span class="status-pill running"><span/> Live updates</span></div>
          <ChartPanel :figure="equityFigure" />
        </article>
        <article class="panel quote-board">
          <div class="panel-header"><div><span class="eyebrow">Latest prices</span><h3>Watchlist</h3></div></div>
          <div v-for="(price, symbol) in snapshot.latest_prices" :key="symbol" class="quote-row"><span class="asset-avatar">{{ symbol.slice(0, 2) }}</span><span><strong>{{ symbol }}</strong><small>{{ state.config.provider }}</small></span><strong>{{ Number(price).toLocaleString(undefined, { maximumFractionDigits: 6 }) }}</strong></div>
        </article>
      </section>
      <section class="split-grid live-tables">
        <article class="panel table-panel"><div class="panel-header"><div><span class="eyebrow">Portfolio</span><h3>Positions &amp; cash</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Asset</th><th class="number">Amount</th></tr></thead><tbody><tr v-for="(quantity, symbol) in snapshot.portfolio?.positions" :key="symbol"><td><strong>{{ symbol }}</strong></td><td class="number">{{ quantity }}</td></tr><tr v-for="(amount, currency) in snapshot.portfolio?.cash" :key="currency"><td>{{ currency }} cash</td><td class="number">{{ money(amount) }}</td></tr></tbody></table></div></article>
        <article class="panel table-panel"><div class="panel-header"><div><span class="eyebrow">Execution</span><h3>Recent fills &amp; orders</h3></div></div><div class="data-table-wrap"><table class="data-table"><thead><tr><th>Symbol</th><th>Status</th><th class="number">Fill</th><th class="number">P&amp;L</th></tr></thead><tbody><tr v-for="(fill, index) in fills" :key="index"><td>{{ fill.order?.symbol || '—' }}</td><td><span class="badge neutral">{{ fill.status }}</span></td><td class="number">{{ fill.fill_price ?? '—' }}</td><td class="number" :class="tone(fill.realized_pnl)">{{ fill.realized_pnl ?? '—' }}</td></tr></tbody></table><div v-if="!fills.length" class="empty-state"><p>Waiting for strategy orders…</p></div></div></article>
      </section>
      <article class="panel event-feed"><div class="panel-header"><div><span class="eyebrow">Live diagnostics</span><h3>Market event feed</h3></div><span>{{ updates.length }} buffered</span></div><div class="event-log"><div v-for="(update, index) in [...updates].reverse().slice(0, 50)" :key="index"><time>{{ eventTime(update.market) }}</time><span class="badge" :class="update.market?.is_final ? 'success' : 'neutral'">{{ update.market?.is_final ? 'CLOSED' : 'PARTIAL' }}</span><strong>{{ update.market?.symbol }}</strong><span>close {{ update.market?.close }}</span><span>volume {{ update.market?.volume }}</span><span v-if="update.fills?.length" class="positive">{{ update.fills.length }} fill{{ update.fills.length === 1 ? '' : 's' }}</span></div></div></article>
    </template>
    <div v-if="state.error" class="callout error-state"><TriangleAlert/><span>{{ state.error }}</span></div>
  </div>
</template>

<script setup>
import { Info, Radio, ShieldCheck, Square, TriangleAlert } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { api, post } from '../api'
import ChartPanel from '../components/chart-panel.vue'
import SearchSelect from '../components/search-select.vue'
import { flattenFills, paperEquitySeries } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast'])
const providers = ['kraken', 'binance', 'coinbase', 'yahoo']
const intervals = props.bootstrap.enums.intervals
const state = reactive({ status: 'idle', config: {}, snapshot: {}, updates: [], error: null })
const form = reactive({ provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategy: '', config: { initial_cash: 100000, base_currency: 'USD', commission_pct: 0.05, commission_fixed: 0, slippage: 0.01, allow_short: false, allow_margin: false, trade_on_partial: false, max_history: 10000 } })
const starting = ref(false)
const liveSymbols = ['BTC-USD', 'ETH-USD', 'SOL-USD', 'BTC-USDT', 'ETH-USDT', 'SOL-USDT']
let timer
const snapshot = computed(() => state.snapshot || {})
const updates = computed(() => state.updates || [])
const fills = computed(() => flattenFills(updates.value))
const equityFigure = computed(() => {
  const series = paperEquitySeries(updates.value)
  return { data: [{ type: 'scatter', mode: 'lines', x: series.map(item => new Date(item.timestamp * 1000)), y: series.map(item => item.equity), line: { color: '#23c483', width: 2 }, fill: 'tozeroy', fillcolor: 'rgba(35,196,131,.12)' }], layout: { yaxis: { title: form.config.base_currency }, xaxis: { title: '' } } }
})
function title(value) { return value.charAt(0).toUpperCase() + value.slice(1) }
function capability(provider, interval = form.interval) {
  const value = props.bootstrap.live.providers?.[provider]
  if (Array.isArray(value)) return { supported: value[0], reason: value[1] }
  if (!value || typeof value !== 'object') return { supported: false, reason: String(value || '') }
  return value.intervals?.[interval] || value
}
function available(provider, interval = form.interval) { return Boolean(capability(provider, interval).supported) }
function support(provider, interval = form.interval) { const result = capability(provider, interval); return result.reason || (result.supported ? `${interval} WebSocket supported` : 'Unavailable') }
function money(value) { return new Intl.NumberFormat('en', { style: 'currency', currency: form.config.base_currency || 'USD', maximumFractionDigits: 2 }).format(Number(value) || 0) }
const tone = value => Number(value) > 0 ? 'positive' : Number(value) < 0 ? 'negative' : ''
function eventTime(market) { const value = market?.received_ts || market?.close_ts; return value ? new Date(value * 1000).toLocaleTimeString() : 'now' }
async function start() { starting.value = true; try { Object.assign(state, await post('/api/live', form)); emit('toast', 'Paper-trading session started.'); poll() } catch (error) { emit('toast', error.message, 'error') } finally { starting.value = false } }
async function stop() { try { Object.assign(state, await post('/api/live/stop')); clearTimeout(timer); emit('toast', 'Paper-trading session stopped.') } catch (error) { emit('toast', error.message, 'error') } }
async function poll() { try { Object.assign(state, await api('/api/live')) } catch (error) { state.error = error.message } if (state.status === 'running') timer = setTimeout(poll, 1000) }
onMounted(async () => { Object.assign(state, await api('/api/live')); if (state.status === 'running') poll() })
onBeforeUnmount(() => clearTimeout(timer))
</script>
