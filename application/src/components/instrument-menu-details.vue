<template>
  <aside class="instrument-menu-details" :aria-label="`${symbol} instrument details`">
    <header class="instrument-menu-heading">
      <span class="instrument-menu-logo" aria-hidden="true">
        <img v-if="logo && !instrumentLogoFailed" :src="logo" alt="" @error="instrumentLogoFailed = true" />
        <ChartCandlestick v-else :size="34" />
      </span>
      <span class="instrument-menu-title">
        <strong>{{ value.name || symbol }}</strong>
        <small v-if="value.name">{{ symbol }}</small>
      </span>
      <span
        v-if="provider"
        class="instrument-menu-provider"
        :aria-label="`Provider: ${providerLabel}`"
      >
        <img
          v-if="providerLogo && !providerLogoFailed"
          :src="providerLogo"
          :alt="`${providerLabel} logo`"
          @error="providerLogoFailed = true"
        />
        <strong v-else>{{ providerLabel }}</strong>
      </span>
    </header>

    <section v-if="loadGraph" class="instrument-menu-chart" aria-live="polite">
      <div v-if="loading" class="instrument-menu-chart-state" role="status">
        <span class="spinner small" /> Loading recent prices…
      </div>
      <template v-else-if="chartSeries.length > 1">
        <div class="instrument-menu-chart-heading">
          <span>Recent daily closes</span>
          <strong :class="changeTone">{{ changeLabel }}</strong>
        </div>
        <div class="instrument-menu-chart-body">
          <div class="instrument-menu-chart-y-axis" aria-hidden="true">
            <span
              v-for="axis in priceAxes"
              :key="axis.y"
              :style="{ top: `${axis.y}px` }"
            >{{ axis.label }}</span>
          </div>
          <div class="instrument-menu-chart-plot">
            <svg
              class="instrument-menu-sparkline"
              viewBox="0 0 360 88"
              role="img"
              :aria-label="`${symbol} recent daily closing-price trend`"
            >
              <defs>
                <linearGradient :id="gradientId" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="currentColor" stop-opacity=".22" />
                  <stop offset="100%" stop-color="currentColor" stop-opacity="0" />
                </linearGradient>
              </defs>
              <g class="instrument-menu-chart-grid" aria-hidden="true">
                <line
                  v-for="axis in priceAxes"
                  :key="axis.y"
                  x1="1"
                  x2="359"
                  :y1="axis.y"
                  :y2="axis.y"
                />
              </g>
              <polygon :points="areaPoints" :fill="`url(#${gradientId})`" aria-hidden="true" />
              <polyline :points="linePoints" vector-effect="non-scaling-stroke" aria-hidden="true" />
            </svg>
            <div class="instrument-menu-chart-x-axis" aria-hidden="true">
              <span>{{ startDate }}</span>
              <span>{{ endDate }}</span>
            </div>
          </div>
        </div>
      </template>
      <p v-else class="instrument-menu-chart-state">
        {{ failed ? 'Recent prices unavailable.' : 'No recent price data.' }}
      </p>
    </section>

    <dl v-if="showMarket || showCurrency">
      <div v-if="showMarket" class="instrument-market-fact">
        <dt>Market</dt>
        <dd class="instrument-market-fact-body">
          <img
            v-if="marketFlagUrl && !marketFlagFailed"
            class="currency-flag market-flag"
            :src="marketFlagUrl"
            :alt="`${marketCountryCode.toUpperCase()} flag`"
            @error="marketFlagFailed = true"
          />
          <span
            v-else-if="marketCountryCode"
            class="currency-flag currency-flag-fallback market-flag"
            aria-hidden="true"
          >{{ marketCountryCode.toUpperCase() }}</span>
          <span class="instrument-market-identity">{{ marketIdentity }}</span>
        </dd>
      </div>
      <div v-if="showCurrency">
        <dt>Currency</dt>
        <dd class="instrument-currency-fact">
          <img
            v-if="currencyFlagUrl && !currencyFlagFailed"
            class="currency-flag"
            :src="currencyFlagUrl"
            alt=""
            @error="currencyFlagFailed = true"
          />
          <span v-else class="currency-flag currency-flag-fallback" aria-hidden="true">
            {{ currencyCountryCode.toUpperCase() || currency.slice(0, 2) }}
          </span>
          <strong>{{ currency }}</strong>
        </dd>
      </div>
    </dl>
  </aside>
</template>

<script setup>
import { ChartCandlestick } from 'lucide-vue-next'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { query } from '../api'
import { formatConfiguredDate } from '../state'

const props = defineProps({
  details: { type: Object, default: () => ({}) },
  display: { type: Object, default: () => ({}) },
  loadGraph: Boolean,
  logo: { type: String, default: '' },
  symbol: { type: String, default: '' }
})

const providerLogos = {
  binance: '/providers/binance.png',
  coinbase: '/providers/coinbase.png',
  kraken: '/providers/kraken.png',
  yahoo: '/providers/yahoo.png'
}
const providerLabels = {
  binance: 'Binance',
  coinbase: 'Coinbase',
  kraken: 'Kraken',
  yahoo: 'Yahoo Finance'
}
const currencyCountryCodes = {
  AED: 'ae', AUD: 'au', BRL: 'br', CAD: 'ca', CHF: 'ch', CNY: 'cn', DKK: 'dk',
  EUR: 'eu', GBP: 'gb', HKD: 'hk', INR: 'in', JPY: 'jp', KRW: 'kr', MXN: 'mx',
  NOK: 'no', NZD: 'nz', PLN: 'pl', SAR: 'sa', SEK: 'se', SGD: 'sg', TRY: 'tr',
  USDC: 'us', USDT: 'us', USD: 'us', ZAR: 'za'
}
const overviewCache = new Map()
const remote = ref({})
const loading = ref(false)
const failed = ref(false)
const instrumentLogoFailed = ref(false)
const marketFlagFailed = ref(false)
const currencyFlagFailed = ref(false)
const providerLogoFailed = ref(false)
let requestVersion = 0

const value = computed(() => ({ ...props.details, ...remote.value }))
const instrumentType = computed(() => String(value.value.instrument_type || '')
  .replace(/[^a-z]/gi, '')
  .toLowerCase())
const isForex = computed(() => instrumentType.value === 'forex')
const isCrypto = computed(() => instrumentType.value === 'crypto')
const marketMic = computed(() => String(value.value.exchange_mic || value.value.exchange || ''))
const marketName = computed(() => String(value.value.exchange_name || ''))
const showMarket = computed(() => !isForex.value && !isCrypto.value &&
  Boolean(marketMic.value || marketName.value))
const marketIdentity = computed(() => marketName.value && marketMic.value
  ? `${marketName.value} (${marketMic.value})`
  : marketName.value || marketMic.value)
const marketCountryCode = computed(() => String(value.value.market_country_code || '').toLowerCase())
const marketFlagUrl = computed(() => /^[a-z]{2}$/.test(marketCountryCode.value)
  ? `https://flagcdn.com/${marketCountryCode.value}.svg`
  : '')
const currency = computed(() => String(value.value.quote || '').toUpperCase())
const showCurrency = computed(() => !isCrypto.value && Boolean(currency.value))
const currencyCountryCode = computed(() => String(value.value.currency_country_code ||
  currencyCountryCodes[currency.value] || '').toLowerCase())
const currencyFlagUrl = computed(() => /^[a-z]{2}$/.test(currencyCountryCode.value)
  ? `https://flagcdn.com/${currencyCountryCode.value}.svg`
  : '')
const provider = computed(() => String(value.value.provider || '').toLowerCase())
const providerLabel = computed(() => providerLabels[provider.value] || title(provider.value))
const providerLogo = computed(() => providerLogos[provider.value] || '')
const chartSeries = computed(() => (Array.isArray(remote.value.sparkline)
  ? remote.value.sparkline
  : [])
  .map((price, index) => ({
    price: Number(price),
    timestamp: Number(remote.value.sparkline_ts?.[index])
  }))
  .filter(point => Number.isFinite(point.price))
  .slice(-30))
const chartGeometry = computed(() => createChartGeometry(chartSeries.value))
const linePoints = computed(() => chartGeometry.value.points.join(' '))
const areaPoints = computed(() => chartGeometry.value.points.length
  ? `1,84 ${linePoints.value} 359,84`
  : '')
const priceAxes = computed(() => chartGeometry.value.axes)
const startDate = computed(() => formatDate(chartSeries.value[0]?.timestamp))
const endDate = computed(() => formatDate(chartSeries.value.at(-1)?.timestamp))
const percentageChange = computed(() => {
  const first = chartSeries.value[0]?.price
  const last = chartSeries.value.at(-1)?.price
  if (!Number.isFinite(first) || !Number.isFinite(last) || first === 0) return null
  return (last - first) / Math.abs(first) * 100
})
const changeLabel = computed(() => percentageChange.value === null
  ? '—'
  : `${percentageChange.value >= 0 ? '+' : ''}${percentageChange.value.toFixed(2)}%`)
const changeTone = computed(() => percentageChange.value === null
  ? ''
  : percentageChange.value >= 0 ? 'positive' : 'negative')
const gradientId = computed(() => `instrument-menu-fill-${props.symbol.replace(/[^a-z0-9]/gi, '-')}`)

function title(value) {
  return value ? `${value.charAt(0).toUpperCase()}${value.slice(1)}` : ''
}

function priceLabel(value) {
  const magnitude = Math.abs(value)
  if (magnitude >= 1000) return value.toLocaleString(undefined, { maximumFractionDigits: 0 })
  if (magnitude >= 10) return value.toFixed(1)
  return value.toFixed(2)
}

function createChartGeometry(series) {
  if (series.length < 2) return { points: [], axes: [] }
  const values = series.map(point => point.price)
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  const span = maximum - minimum
  const points = values.map((item, index) => {
    const x = 1 + index / (values.length - 1) * 358
    const y = span === 0 ? 48 : 80 - (item - minimum) / span * 64
    return `${x.toFixed(2)},${y.toFixed(2)}`
  })
  const middle = minimum + span / 2
  return {
    points,
    axes: [
      { label: priceLabel(maximum), y: 16 },
      { label: priceLabel(middle), y: 48 },
      { label: priceLabel(minimum), y: 80 }
    ]
  }
}

function formatDate(timestamp) {
  if (!Number.isFinite(timestamp)) return ''
  return formatConfiguredDate(timestamp, props.display, '')
}

async function loadOverview() {
  const version = ++requestVersion
  remote.value = {}
  loading.value = false
  failed.value = false
  if (!props.loadGraph || !props.symbol) return

  const key = `${props.symbol}|${props.details.instrument_type || ''}|${props.details.provider || ''}`
  if (!overviewCache.has(key)) {
    overviewCache.set(key, query('/api/instrument-overview', {
      symbol: props.symbol,
      instrument_type: props.details.instrument_type,
      provider: props.details.provider
    }))
  }
  loading.value = true
  try {
    const overview = await overviewCache.get(key)
    if (version === requestVersion) {
      remote.value = overview && typeof overview === 'object' && !Array.isArray(overview)
        ? overview
        : {}
    }
  } catch {
    overviewCache.delete(key)
    if (version === requestVersion) failed.value = true
  } finally {
    if (version === requestVersion) loading.value = false
  }
}

watch(() => [
  props.symbol,
  props.details.instrument_type,
  props.details.provider,
  props.loadGraph
], loadOverview, { immediate: true })
watch(() => [props.symbol, props.logo], () => { instrumentLogoFailed.value = false })
watch(marketCountryCode, () => { marketFlagFailed.value = false })
watch(currencyCountryCode, () => { currencyFlagFailed.value = false })
watch(provider, () => { providerLogoFailed.value = false })
onBeforeUnmount(() => { requestVersion += 1 })
</script>
