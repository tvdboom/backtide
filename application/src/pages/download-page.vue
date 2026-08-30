<template>
  <div class="page narrow-page download-page">
    <section class="page-intro"><div><h2>Download historical bars</h2><p>Resolve instruments and fill the local data store from your configured provider.</p></div></section>
    <form class="panel form-section" @submit.prevent="download">
      <div class="segmented wide-control"><button v-for="type in enums.instrument_types" :key="type" type="button" :class="{ active: form.instrument_type === optionValue('instrument_type', type) }" @click="setType(type)"><component :is="instrumentTypeIcon(type)" :size="16" />{{ type }}</button></div>
      <div class="form-grid two">
        <div class="field-label wide symbol-select-field"><span>Symbols</span><SearchSelect :key="form.instrument_type" v-model="form.symbols" :options="symbols" :descriptions="names" :logos="logos" :selected-logos="selectedLogos" :option-details="instrumentDetails" :loading="loadingInstruments" clearable clear-label="symbols" allow-custom input-id="download-symbols" label="Download symbols" placeholder="Search symbols or company names…" /></div>
        <div class="field-label wide">
          <span>Intervals</span>
          <IntervalPicker v-model="form.intervals" :options="enums.intervals" multiple input-id="download-intervals" label="Download intervals" />
        </div>
        <ToggleField v-model="form.full_history" label="Full available history" description="Provider limits still apply by interval." help="Download every historical bar available from the selected provider." />
        <span />
        <label v-if="!form.full_history">Start date<input v-model="form.start" type="date" :min="plan?.available_start" :max="plan?.available_end" /></label>
        <label v-if="!form.full_history">End date<input v-model="form.end" type="date" :min="form.start || plan?.available_start" :max="plan?.available_end" /></label>
      </div>
      <div v-if="catalogError" class="callout download-plan-error"><TriangleAlert :size="18" /><span>{{ catalogError }}</span></div>
      <div v-if="planning && !plan" class="download-plan-loading"><span class="spinner small" /> Checking provider availability…</div>
      <div v-else-if="planError && !plan" class="callout download-plan-error"><TriangleAlert :size="18" /><span>{{ planError }}</span></div>
      <section v-else-if="plan" class="download-plan">
        <div class="metric-grid download-metrics">
          <article class="metric-card"><div class="metric-icon"><Rows3 :size="19" /></div><span>Estimated bars</span><strong>{{ compact(plan.summary.estimated_bars) }}</strong><small>{{ plan.summary.series }} provider series</small></article>
          <article class="metric-card"><div class="metric-icon"><Clock3 :size="19" /></div><span>Estimated time</span><strong>{{ duration(plan.summary.estimated_seconds) }}</strong><small>at roughly 40k bars / second</small></article>
          <article class="metric-card"><div class="metric-icon"><HardDriveDownload :size="19" /></div><span>Estimated memory</span><strong>{{ bytes(plan.summary.estimated_bytes) }}</strong><small>at roughly 120 bytes / bar</small></article>
        </div>
        <details class="download-details" open>
          <summary>
            <span class="download-details-icon"><CloudDownload :size="18" /></span>
            <span class="download-details-copy"><strong>Download details</strong><small>Exact provider ranges and effective request</small></span>
            <ChevronDown class="download-details-chevron" :size="18" />
          </summary>
          <div class="download-profile-list">
            <article v-for="profile in plan.profiles" :key="`${profile.symbol}-${profile.provider}`" class="download-profile">
              <header>
                <div class="download-profile-identity">
                  <span class="download-profile-icon"><img v-if="profileLogo(profile)" :src="profileLogo(profile)" alt="" @error="profileLogoFailed(profile)" /><ChartCandlestick v-else :size="19" /></span>
                  <span><strong>{{ profile.symbol }}</strong><small v-if="profile.name && profile.name !== profile.symbol">{{ profile.name }}</small></span>
                </div>
                <div class="download-profile-meta">
                  <div class="download-provider"><img v-if="providerLogo(profile.provider)" :src="providerLogo(profile.provider)" :alt="`${profile.provider} provider`" @error="providerLogoFailed(profile.provider)" /></div>
                  <span v-if="profile.exchange"><small>Exchange</small><strong>{{ profile.exchange }}</strong></span>
                  <span><small>Currency</small><strong>{{ profile.quote }}</strong></span>
                </div>
              </header>
              <div v-if="profile.legs.length" class="download-legs"><span>Conversion via</span><strong v-for="leg in profile.legs" :key="leg">{{ leg }}</strong></div>
              <div class="download-intervals">
                <div v-for="interval in profile.intervals" :key="interval.interval" class="download-interval-row">
                  <span class="download-interval-badge">{{ interval.interval }}</span>
                  <span class="download-range"><small>Provider availability</small><span>{{ displayDate(interval.available_start) }} <ArrowRight :size="12" /> {{ displayDate(interval.available_end) }}</span></span>
                  <span class="download-range requested"><small>Download range</small><span>{{ displayDate(interval.download_start) }} <ArrowRight :size="12" /> {{ displayDate(interval.download_end) }}</span></span>
                  <span class="download-row-count"><span class="download-row-value"><strong>~{{ compact(interval.estimated_bars) }}</strong><span>bars</span></span><small>{{ formatDaySpan(interval.days) }}</small></span>
                </div>
              </div>
            </article>
          </div>
        </details>
      </section>
      <div class="form-footer"><span class="form-spacer"/><button class="primary" :disabled="loading || planning || Boolean(planError) || !plan || !form.symbols.length || !form.intervals.length"><span v-if="loading" class="spinner small" /><Download v-else :size="16" /> {{ loading ? 'Queuing…' : 'Download data' }}</button></div>
    </form>
    <article v-if="job" class="panel job-card" :class="job.status" aria-live="polite">
      <div class="job-card-status">
        <span v-if="jobActive" class="spinner" />
        <span v-else class="job-state-icon" :class="job.status"><CircleCheck v-if="job.status === 'success'" :size="20" /><TriangleAlert v-else :size="20" /></span>
        <span class="job-copy"><strong>{{ jobTitle }}</strong><small>{{ jobMessage }}</small></span>
        <button v-if="job.status === 'success'" type="button" class="secondary" @click="inspectData"><BarChart3 :size="16" /> Inspect data</button>
      </div>
      <div v-if="jobActive" class="progress"><span :style="{ width: job.status === 'running' ? '65%' : '24%' }" /></div>
    </article>
  </div>
</template>

<script setup>
import { ArrowLeftRight, ArrowRight, BarChart3, Bitcoin, ChartCandlestick, ChevronDown, CircleCheck, Clock3, CloudDownload, Download, HardDriveDownload, Landmark, Rows3, TriangleAlert } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { api, post, query } from '../api'
import IntervalPicker from '../components/interval-picker.vue'
import SearchSelect from '../components/search-select.vue'
import ToggleField from '../components/toggle-field.vue'
import { experimentOptionValue, formatConfiguredDate, formatDaySpan, instrumentLogoUrl } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const enums = props.bootstrap.enums
const optionValue = experimentOptionValue
const form = reactive({ instrument_type: optionValue('instrument_type', enums.instrument_types[0]), symbols: [], intervals: ['1d'], full_history: true, start: '', end: '' })
const instruments = ref([])
const loadingInstruments = ref(false)
const catalogError = ref('')
const loading = ref(false)
const job = ref(null)
const downloadedSymbols = ref([])
const plan = ref(null)
const planning = ref(false)
const planError = ref('')
const failedProfileLogos = reactive(new Set())
const failedProviderLogos = reactive(new Set())
let timer
let planTimer
let planController
const symbols = computed(() => instruments.value.map(item => item.symbol))
const names = computed(() => Object.fromEntries(instruments.value.map(item => [item.symbol, item.name])))
const instrumentDetails = computed(() => Object.fromEntries(instruments.value.map(item => [item.symbol, item])))
const logos = computed(() => {
  const values = Object.fromEntries(instruments.value.map(item => [
    item.symbol,
    instrumentLogoUrl(item.symbol, item.instrument_type, props.bootstrap.display.logokit_api_key)
  ]))
  for (const symbol of form.symbols) {
    if (!values[symbol]) {
      values[symbol] = instrumentLogoUrl(
        symbol,
        form.instrument_type,
        props.bootstrap.display.logokit_api_key
      )
    }
  }
  return values
})
const selectedLogos = computed(() => Object.fromEntries(form.symbols.map(symbol => [
  symbol,
  instrumentLogoUrl(symbol, form.instrument_type, props.bootstrap.display.logokit_api_key)
])))
const jobActive = computed(() => ['queued', 'running'].includes(job.value?.status))
const jobTitle = computed(() => {
  if (job.value?.status === 'queued') return 'Download queued'
  if (job.value?.status === 'running') return 'Downloading data'
  if (job.value?.status === 'success') return 'Download complete'
  return 'Download failed'
})
const jobMessage = computed(() => {
  if (job.value?.status === 'queued') return 'Preparing provider requests…'
  if (job.value?.status === 'running') return 'Fetching and storing missing market data…'
  if (job.value?.status === 'error') return job.value.error || 'The download could not be completed.'
  const succeeded = Number(job.value?.result?.n_succeeded)
  const failed = Number(job.value?.result?.n_failed)
  if (!Number.isFinite(succeeded)) return 'Downloaded data is stored locally and ready to inspect.'
  const total = succeeded + (Number.isFinite(failed) ? failed : 0)
  if (failed > 0) return `${succeeded} of ${total} series downloaded. Review the provider warnings before analysis.`
  return `${succeeded} series downloaded and stored locally.`
})
const instrumentTypeIcons = {
  Stocks: ChartCandlestick,
  ETF: Landmark,
  Forex: ArrowLeftRight,
  Crypto: Bitcoin
}
const providerLogos = {
  binance: '/providers/binance.png',
  coinbase: '/providers/coinbase.png',
  kraken: '/providers/kraken.png',
  yahoo: '/providers/yahoo.png'
}
function instrumentTypeIcon(type) { return instrumentTypeIcons[type] || ChartCandlestick }
function profileLogoKey(profile) { return `${profile.instrument_type}:${profile.symbol}` }
function profileLogo(profile) {
  if (failedProfileLogos.has(profileLogoKey(profile))) return ''
  return instrumentLogoUrl(
    profile.symbol,
    profile.instrument_type,
    props.bootstrap.display.logokit_api_key
  )
}
function profileLogoFailed(profile) { failedProfileLogos.add(profileLogoKey(profile)) }
function providerLogo(provider) {
  const value = String(provider || '').toLowerCase()
  const key = Object.keys(providerLogos).find(item => value.includes(item))
  return key && !failedProviderLogos.has(key) ? providerLogos[key] : ''
}
function providerLogoFailed(provider) {
  const value = String(provider || '').toLowerCase()
  const key = Object.keys(providerLogos).find(item => value.includes(item))
  if (key) failedProviderLogos.add(key)
}
async function setType(type) {
  form.instrument_type = optionValue('instrument_type', type)
  form.symbols = []
  instruments.value = []
  await loadInstruments()
}
async function loadInstruments() {
  loadingInstruments.value = true
  catalogError.value = ''
  try {
    const result = await query('/api/instruments', {
      instrument_type: form.instrument_type,
      source: 'catalog',
      limit: 1500
    })
    instruments.value = [...result].sort((left, right) => left.symbol.localeCompare(right.symbol))
  } catch (error) {
    instruments.value = []
    catalogError.value = `Could not load the symbol catalog. ${error.message}`
    emit('toast', catalogError.value, 'error')
  } finally {
    loadingInstruments.value = false
  }
}
function compact(value) { return new Intl.NumberFormat('en', { notation: Number(value) > 99_999 ? 'compact' : 'standard', maximumFractionDigits: 1 }).format(Number(value) || 0) }
function displayDate(value) { return formatConfiguredDate(value, props.bootstrap?.display) }
function duration(value) {
  const seconds = Math.floor(Number(value) || 0)
  if (seconds >= 3600) return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`
  if (seconds >= 60) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`
  return seconds ? `${seconds}s` : '<1s'
}
function bytes(value) {
  const amount = Number(value) || 0
  if (amount >= 1024 ** 3) return `${(amount / 1024 ** 3).toFixed(2)} GB`
  if (amount >= 1024 ** 2) return `${(amount / 1024 ** 2).toFixed(1)} MB`
  return '<0.1 MB'
}
function schedulePlan() {
  clearTimeout(planTimer)
  planController?.abort()
  planController = null
  if (!form.symbols.length || !form.intervals.length) {
    plan.value = null
    planError.value = ''
    planning.value = false
    return
  }
  planning.value = true
  planError.value = ''
  if (!form.full_history && plan.value) {
    if (!form.start) form.start = plan.value.available_start
    if (!form.end) form.end = plan.value.available_end
  }
  planTimer = setTimeout(loadPlan, 250)
}
async function loadPlan() {
  planController = new AbortController()
  const controller = planController
  planning.value = true
  planError.value = ''
  try {
    const value = await api('/api/downloads/plan', {
      method: 'POST',
      signal: controller.signal,
      body: {
        ...form,
        start: form.full_history ? null : form.start,
        end: form.full_history ? null : form.end
      }
    })
    if (controller === planController) plan.value = value
  } catch (error) {
    if (error.name !== 'AbortError' && controller === planController) {
      plan.value = null
      planError.value = error.message
      emit('toast', error.message, 'error')
    }
  } finally {
    if (controller === planController) planning.value = false
  }
}
async function download() {
  if (!form.symbols.length || !form.intervals.length) { emit('toast', 'Select symbols and intervals.', 'error'); return }
  loading.value = true
  try {
    clearTimeout(timer)
    const requestedSymbols = [...form.symbols]
    job.value = await post('/api/downloads', { ...form, start: form.full_history ? null : form.start, end: form.full_history ? null : form.end })
    downloadedSymbols.value = requestedSymbols
    resetDownloadForm()
    await poll()
  } catch (error) { emit('toast', error.message, 'error') }
  finally { loading.value = false }
}
function resetDownloadForm() {
  const instrumentType = form.instrument_type
  clearTimeout(planTimer)
  planController?.abort()
  planController = null
  Object.assign(form, {
    instrument_type: instrumentType,
    symbols: [],
    intervals: ['1d'],
    full_history: true,
    start: '',
    end: ''
  })
  plan.value = null
  planError.value = ''
  planning.value = false
}
function inspectData() {
  sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify(downloadedSymbols.value))
  emit('navigate', 'analysis')
}
async function poll() {
  const jobId = job.value?.id
  if (!jobId) return
  try {
    job.value = await api(`/api/jobs/${jobId}`)
  } catch (error) {
    job.value = { ...job.value, status: 'error', error: error.message }
  }
  if (jobActive.value) {
    timer = setTimeout(poll, 1000)
    return
  }
  emit(
    'toast',
    job.value.status === 'success' ? 'Download completed.' : job.value.error,
    job.value.status === 'success' ? 'success' : 'error'
  )
}
onMounted(loadInstruments)
watch(form, schedulePlan, { deep: true })
onBeforeUnmount(() => {
  clearTimeout(timer)
  clearTimeout(planTimer)
  planController?.abort()
})
</script>
