<template>
  <Teleport to="body">
    <aside
      v-if="visible && value"
      class="instrument-preview"
      :style="position"
      role="note"
      :aria-label="`${value.symbol} instrument overview`"
    >
      <header>
        <span class="instrument-preview-logo">
          <img v-if="logo && !logoFailed" :src="logo" alt="" @error="logoFailed = true" />
          <ChartCandlestick v-else :size="24" aria-hidden="true" />
        </span>
        <span>
          <strong>{{ value.name || value.symbol }}</strong>
          <small>{{ value.symbol }}</small>
        </span>
        <span class="badge neutral">{{ typeLabel }}</span>
      </header>

      <section v-if="points.length > 1" class="instrument-preview-chart">
        <div>
          <span>Latest 30 stored closes</span>
          <strong :class="changeTone">{{ changeLabel }}</strong>
        </div>
        <svg viewBox="0 0 300 82" preserveAspectRatio="none" aria-label="Stored closing-price trend">
          <defs>
            <linearGradient :id="gradientId" x1="0" x2="0" y1="0" y2="1">
              <stop offset="0" stop-color="currentColor" stop-opacity=".24" />
              <stop offset="1" stop-color="currentColor" stop-opacity="0" />
            </linearGradient>
          </defs>
          <polygon :points="areaPoints" :fill="`url(#${gradientId})`" />
          <polyline :points="linePoints" fill="none" stroke="currentColor" stroke-width="2.3" vector-effect="non-scaling-stroke" />
        </svg>
        <div class="instrument-preview-range"><span>{{ price(points[0]) }}</span><span>{{ price(points.at(-1)) }}</span></div>
      </section>
      <div v-else class="instrument-preview-empty">
        <ChartNoAxesCombined :size="18" />
        <span>No stored price history yet. Download bars to add a trend preview.</span>
      </div>

      <dl>
        <div v-if="value.exchange" class="instrument-preview-meta-left"><dt>Market</dt><dd>{{ value.exchange }}</dd></div>
        <div v-if="pairLabel" class="instrument-preview-meta-left"><dt>Pair</dt><dd>{{ pairLabel }}</dd></div>
        <div v-if="value.quote" class="instrument-preview-meta-left">
          <dt>Currency</dt>
          <dd class="instrument-preview-detail">
            <img
              v-if="currencyFlagUrl && !currencyFlagFailed"
              class="instrument-preview-currency-flag"
              :src="currencyFlagUrl"
              alt=""
              @error="currencyFlagFailed = true"
            />
            <span v-else class="instrument-preview-currency-fallback" aria-hidden="true">{{ currencyCountryCode.toUpperCase() || currencyCode.slice(0, 2) }}</span>
            <span>{{ currencyCode }}</span>
          </dd>
        </div>
        <div v-if="providerLogo" class="instrument-preview-provider">
          <img :src="providerLogo" :alt="`${sourceLabel} provider`" />
        </div>
      </dl>
    </aside>
  </Teleport>
</template>

<script setup>
import { ChartCandlestick, ChartNoAxesCombined } from 'lucide-vue-next'
import { computed, ref, watch } from 'vue'
import { query } from '../api'

const props = defineProps({
  anchor: { type: Object, default: null },
  details: { type: Object, default: () => ({}) },
  logo: { type: String, default: '' },
  symbol: { type: String, default: '' },
  visible: Boolean
})

const overviewCache = new Map()
const remote = ref({})
const position = ref({})
const logoFailed = ref(false)
const currencyFlagFailed = ref(false)
const value = computed(() => ({ symbol: props.symbol, ...props.details, ...remote.value }))
const points = computed(() => (value.value.sparkline || [])
  .map(Number)
  .filter(Number.isFinite)
  .slice(-30))
const typeLabel = computed(() => instrumentTypeLabel(value.value.instrument_type))
const sourceLabel = computed(() => title(value.value.provider || ''))
const providerLogo = computed(() => ({
  binance: '/providers/binance.png',
  coinbase: '/providers/coinbase.png',
  kraken: '/providers/kraken.png',
  yahoo: '/providers/yahoo.png'
}[String(value.value.provider || '').toLowerCase()] || ''))
const currencyCode = computed(() => String(value.value.quote || '').toUpperCase())
const currencyCountryCode = computed(() => currencyCountryCodes[currencyCode.value] || '')
const currencyFlagUrl = computed(() => currencyCountryCode.value
  ? `https://flagcdn.com/${currencyCountryCode.value}.svg`
  : '')
const pairLabel = computed(() => value.value.base && value.value.quote
  ? `${value.value.base} / ${value.value.quote}`
  : '')
const currencyCountryCodes = {
  AED: 'ae', AUD: 'au', BRL: 'br', CAD: 'ca', CHF: 'ch', CNY: 'cn',
  CZK: 'cz', DKK: 'dk', EUR: 'eu', EURT: 'eu', GBP: 'gb', HKD: 'hk', HUF: 'hu',
  IDR: 'id', ILS: 'il', INR: 'in', JPY: 'jp', KRW: 'kr', MXN: 'mx',
  MYR: 'my', NOK: 'no', NZD: 'nz', PLN: 'pl', SAR: 'sa', SEK: 'se',
  SGD: 'sg', THB: 'th', TRY: 'tr', TWD: 'tw', USD: 'us', USDC: 'us',
  USDT: 'us', ZAR: 'za'
}
const change = computed(() => points.value.length > 1 && points.value[0]
  ? (points.value.at(-1) / points.value[0] - 1) * 100
  : null)
const changeLabel = computed(() => Number.isFinite(change.value)
  ? `${change.value > 0 ? '+' : ''}${change.value.toFixed(2)}%`
  : '—')
const changeTone = computed(() => change.value > 0 ? 'positive' : change.value < 0 ? 'negative' : '')
const gradientId = computed(() => `instrument-preview-${props.symbol.replaceAll(/[^a-z0-9]/gi, '-')}`)
const linePoints = computed(() => chartPoints(points.value).join(' '))
const areaPoints = computed(() => points.value.length > 1
  ? `0,82 ${linePoints.value} 300,82`
  : '')

function title(text) {
  return String(text || '').replaceAll('_', ' ').replace(/^./, letter => letter.toUpperCase())
}
function instrumentTypeLabel(raw) {
  const value = String(raw || '').toLowerCase()
  if (value.includes('stock')) return 'Stock'
  if (value.includes('etf')) return 'ETF'
  if (value.includes('forex')) return 'Forex'
  if (value.includes('crypto')) return 'Crypto'
  return title(raw || 'Instrument')
}
function chartPoints(values) {
  if (values.length < 2) return []
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  const span = maximum - minimum || 1
  return values.map((item, index) => {
    const x = index / (values.length - 1) * 300
    const y = 72 - (item - minimum) / span * 62
    return `${x.toFixed(2)},${y.toFixed(2)}`
  })
}
function price(raw) {
  const numeric = Number(raw)
  return Number.isFinite(numeric)
    ? numeric.toLocaleString('en', { maximumFractionDigits: Math.abs(numeric) < 1 ? 6 : 2 })
    : '—'
}
function place() {
  const bounds = props.anchor?.getBoundingClientRect?.()
  if (!bounds) return
  const width = Math.min(350, window.innerWidth - 24)
  const spaceRight = window.innerWidth - bounds.right
  const left = spaceRight >= width + 14
    ? bounds.right + 10
    : Math.max(12, bounds.left - width - 10)
  const top = Math.max(12, Math.min(bounds.top, window.innerHeight - 440))
  position.value = { left: `${left}px`, top: `${top}px`, width: `${width}px` }
}
async function loadOverview() {
  if (!props.visible || !props.symbol) return
  place()
  logoFailed.value = false
  currencyFlagFailed.value = false
  const key = `${props.symbol}|${props.details.instrument_type || ''}|${props.details.provider || ''}`
  if (!overviewCache.has(key)) {
    overviewCache.set(key, query('/api/instrument-overview', {
      symbol: props.symbol,
      instrument_type: props.details.instrument_type,
      provider: props.details.provider
    }).catch(() => ({})))
  }
  remote.value = await overviewCache.get(key)
}

watch(() => [props.visible, props.symbol, props.anchor], loadOverview, { immediate: true })
watch(() => props.visible, shown => {
  if (!shown) return
  if (window.requestAnimationFrame) window.requestAnimationFrame(place)
  else place()
})
</script>
