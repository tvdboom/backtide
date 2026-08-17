<template>
  <div class="page">
    <section class="page-intro"><div><h2>Session history</h2><p>Inspect and replay locally persisted paper-trading sessions.</p></div><div class="session-history-toolbar"><label>Replay speed<select v-model="replaySpeed" aria-label="Replay playback speed"><option value="1">1× real time</option><option value="2">2×</option><option value="5">5×</option><option value="10">10×</option><option value="max">Maximum</option></select></label><button class="secondary" @click="load"><RefreshCw :size="16"/> Refresh</button></div></section>
    <section v-if="loading" class="panel empty-state large"><span class="spinner"/><p>Loading paper sessions...</p></section>
    <section v-else-if="!sessions.length" class="panel empty-state large"><History :size="30"/><h3>No saved sessions</h3><p>Completed paper sessions will appear here automatically.</p></section>
    <section v-else class="panel table-panel">
      <div class="data-table-wrap">
        <table class="data-table session-history-table">
          <thead><tr><th>Started</th><th>Finished</th><th>Strategies</th><th>Status</th><th class="number">Starting equity</th><th class="number">Final P&amp;L</th><th class="number">Actions</th></tr></thead>
          <tbody v-for="group in sessionGroups" :key="group.session.id">
            <tr class="session-history-row">
              <td>
                <div class="session-history-start">
                  <span>{{ dateTime(group.session.started_at) }}</span>
                  <button
                    v-if="group.replays.length"
                    type="button"
                    class="session-replay-toggle"
                    :aria-expanded="isExpanded(group.session.id)"
                    :aria-label="`${isExpanded(group.session.id) ? 'Hide' : 'Show'} ${replayCount(group.replays.length)}`"
                    @click="toggleReplays(group.session.id)"
                  >
                    <ChevronRight :size="14" aria-hidden="true"/>
                    {{ replayCount(group.replays.length) }}
                  </button>
                </div>
              </td>
              <td>{{ group.session.finished_at ? dateTime(group.session.finished_at) : '—' }}</td>
              <td><StrategySummary :names="strategyNames(group.session)" /></td>
              <td><span class="badge" :class="statusTone(group.session.status)">{{ group.session.status }}</span></td>
              <td class="number">{{ startingEquity(group.session) }}</td>
              <td class="number" :class="finalPnlTone(group.session)">{{ finalPnl(group.session) }}</td>
              <td><div class="session-history-actions"><button v-if="isActive(group.session)" class="primary compact-button" type="button" @click="openSession"><ExternalLink :size="14"/> Open</button><template v-else><button class="secondary compact-button" type="button" :disabled="isStarting(group.session)" @click="replay(group.session)"><RotateCcw :size="14"/> Replay</button><button class="primary compact-button" type="button" :disabled="isStarting(group.session)" @click="goLive(group.session)"><Radio :size="14"/> Go live</button><button type="button" class="icon-button danger" :aria-label="`Delete session from ${dateTime(group.session.started_at)}`" :disabled="isStarting(group.session)" @click="requestDelete(group.session)"><Trash2 :size="16"/></button></template></div></td>
            </tr>
            <template v-if="isExpanded(group.session.id)">
              <tr class="session-comparison-row">
                <td colspan="7">
                  <div class="session-comparison" aria-label="Original and replay comparison">
                    <div class="session-comparison-item original"><small>Original session</small><span>Final P&amp;L</span><strong>{{ finalPnl(group.session) }}</strong><em>Comparison baseline</em></div>
                    <div v-for="session in group.replays" :key="`comparison-${session.id}`" class="session-comparison-item"><div class="session-comparison-heading"><small>Replay · {{ dateTime(session.started_at) }}</small><span class="session-replay-speed-badge" :title="replaySpeedDescription(session)" :aria-label="replaySpeedDescription(session)">{{ replaySpeedBadge(session) }}</span></div><span>Final P&amp;L</span><strong :class="finalPnlTone(session)">{{ finalPnl(session) }}</strong><span>P&amp;L difference: <b :class="comparisonTone(session, group.session)">{{ comparisonDelta(session, group.session) }}</b></span><em :title="warmupExplanation">{{ replayWarmupDescription(session) }}</em></div>
                  </div>
                </td>
              </tr>
              <tr v-for="session in group.replays" :key="session.id" class="session-replay-row">
                <td>
                  <div class="session-replay-start">
                    <RotateCcw :size="14" aria-hidden="true"/>
                    <span><small>Replay</small>{{ dateTime(session.started_at) }}</span>
                  </div>
                </td>
                <td>{{ session.finished_at ? dateTime(session.finished_at) : '—' }}</td>
                <td><StrategySummary :names="strategyNames(session)" /></td>
                <td><span class="badge" :class="statusTone(session.status)">{{ session.status }}</span></td>
                <td class="number">{{ startingEquity(session) }}</td>
                <td class="number" :class="finalPnlTone(session)">{{ finalPnl(session) }}</td>
                <td><div class="session-history-actions"><button v-if="isActive(session)" class="primary compact-button" type="button" @click="openSession"><ExternalLink :size="14"/> Open</button><template v-else><button class="secondary compact-button" type="button" :disabled="isStarting(session)" @click="replay(session)"><RotateCcw :size="14"/> Replay</button><button class="primary compact-button" type="button" :disabled="isStarting(session)" @click="goLive(session)"><Radio :size="14"/> Go live</button><button type="button" class="icon-button danger" :aria-label="`Delete session from ${dateTime(session.started_at)}`" :disabled="isStarting(session)" @click="requestDelete(session)"><Trash2 :size="16"/></button></template></div></td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </section>
    <ConfirmationModal
      :open="Boolean(pendingDelete)"
      :title="pendingDelete ? `Delete session from ${dateTime(pendingDelete.started_at)}?` : ''"
      message="Are you sure you want to delete this paper-trading session and its recorded events? This action cannot be undone."
      :busy="deleting"
      @cancel="pendingDelete = null"
      @confirm="destroy"
    />
  </div>
</template>

<script setup>
import { ChevronRight, ExternalLink, History, Radio, RefreshCw, RotateCcw, Trash2 } from 'lucide-vue-next'
import { computed, onActivated, onMounted, ref } from 'vue'
import { api, post, remove } from '../api'
import ConfirmationModal from '../components/confirmation-modal.vue'
import StrategySummary from '../components/strategy-summary.vue'
import { formatConfiguredCurrency, formatConfiguredDateTime } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'navigate'])
const sessions = ref([])
const loading = ref(false)
const replaying = ref('')
const startingLive = ref('')
const deleting = ref(false)
const pendingDelete = ref(null)
const replaySpeed = ref('max')
const expandedSessions = ref(new Set())

const sessionGroups = computed(() => {
  const sessionsById = new Map(sessions.value.map(session => [session.id, session]))
  const groups = new Map()
  const rootSession = session => {
    let root = session
    const visited = new Set([root.id])
    while (root.config?.mode === 'replay') {
      const sourceId = root.config?.source_session_id
      const source = sourceId && !visited.has(sourceId) ? sessionsById.get(sourceId) : null
      if (!source) break
      root = source
      visited.add(root.id)
    }
    return root
  }

  for (const session of sessions.value) {
    const root = rootSession(session)
    if (!groups.has(root.id)) groups.set(root.id, { session: root, replays: [] })
    if (session.id !== root.id) groups.get(root.id).replays.push(session)
  }

  return [...groups.values()]
    .map(group => ({
      ...group,
      replays: group.replays.sort((left, right) =>
        String(right.started_at || '').localeCompare(String(left.started_at || '')))
    }))
    .sort((left, right) => String(right.session.started_at || '')
      .localeCompare(String(left.session.started_at || '')))
})

function dateTime(value) { return formatConfiguredDateTime(value, props.bootstrap?.display) }
function money(value, currency = 'USD') {
  return formatConfiguredCurrency(value, currency, props.bootstrap?.display)
}
function strategyNames(session) {
  const config = session.config || {}
  return config.strategies?.length ? config.strategies : config.strategy ? [config.strategy] : []
}
function startingEquityAmount(session) {
  const strategyCount = strategyNames(session).length
  const initialCash = Number(session.config?.config?.initial_cash)
  return strategyCount && Number.isFinite(initialCash) ? initialCash * strategyCount : null
}
function finalEquityAmount(session) {
  const value = Number(session.snapshot?.equity)
  return strategyNames(session).length && Number.isFinite(value) ? value : null
}
function startingEquity(session) {
  const value = startingEquityAmount(session)
  return value === null ? '—' : money(value, session.config?.config?.base_currency)
}
function finalPnlAmount(session) {
  const starting = startingEquityAmount(session)
  const final = finalEquityAmount(session)
  return starting === null || final === null ? null : final - starting
}
function finalPnl(session) {
  const value = finalPnlAmount(session)
  return value === null ? '—' : money(value, session.config?.config?.base_currency)
}
function finalPnlTone(session) {
  const value = finalPnlAmount(session)
  return value === null || value === 0 ? '' : value > 0 ? 'positive' : 'negative'
}
function comparisonDeltaAmount(session, original) {
  const replayValue = finalPnlAmount(session)
  const originalValue = finalPnlAmount(original)
  return replayValue === null || originalValue === null ? null : replayValue - originalValue
}
function comparisonDelta(session, original) {
  const value = comparisonDeltaAmount(session, original)
  return value === null
    ? '—'
    : `${value > 0 ? '+' : ''}${money(value, original.config?.config?.base_currency)}`
}
function comparisonTone(session, original) {
  const value = comparisonDeltaAmount(session, original)
  return value === null || value === 0 ? '' : value > 0 ? 'positive' : 'negative'
}
function replaySpeedDescription(session) {
  const speed = Number(session.config?.playback_speed)
  return speed > 0 ? `Playback speed: ${speed}×` : 'Playback speed: Maximum'
}
function replaySpeedBadge(session) {
  const speed = Number(session.config?.playback_speed)
  return speed > 0 ? `${speed}×` : 'Maximum'
}
const warmupExplanation = 'Starting price history is loaded before replayed events so strategies and indicators begin with the context they need.'
function replayWarmupDescription(session) {
  const replay = session.health?.replay
  if (!replay) return 'Starting price history was not saved for this older session'
  return replay?.warmup_source === 'recorded'
    ? `Starting price history: ${replay.warmup_bars_loaded || 0} saved bars restored`
    : replay?.warmup_source === 'storage'
      ? `Starting price history: ${replay.warmup_bars_loaded || 0} current stored bars used`
      : replay?.warmup_source === 'unavailable'
        ? 'Starting price history: no matching stored bars were available'
        : 'Starting price history: none requested'
}
function replayCount(count) { return `${count} ${count === 1 ? 'replay' : 'replays'}` }
function isActive(session) { return ['running', 'paused'].includes(session.status) }
function statusTone(status) {
  if (status === 'error') return 'error'
  return ['running', 'paused'].includes(status) ? 'running' : 'neutral'
}
function isExpanded(id) { return expandedSessions.value.has(id) }
function toggleReplays(id) {
  const expanded = new Set(expandedSessions.value)
  if (expanded.has(id)) expanded.delete(id)
  else expanded.add(id)
  expandedSessions.value = expanded
}
async function load() { loading.value = true; try { sessions.value = await api('/api/live/sessions') } catch (error) { emit('toast', error.message, 'error') } finally { loading.value = false } }
function openSession() { emit('navigate', 'live') }
function isStarting(session) {
  return replaying.value === session.id || startingLive.value === session.id
}
async function replay(session) {
  replaying.value = session.id
  try {
    await post('/api/live/replay', { session_id: session.id, speed: replaySpeed.value })
    emit('toast', 'Session replay started.')
    emit('navigate', 'live')
  } catch (error) {
    emit('toast', error.message, 'error')
  } finally {
    replaying.value = ''
  }
}
function livePayload(session) {
  const config = session.config || {}
  return {
    provider: config.provider,
    interval: config.interval,
    symbols: config.symbols || [],
    strategies: config.strategies?.length
      ? config.strategies
      : config.strategy ? [config.strategy] : [],
    indicators: config.indicators || [],
    warmup_bars: config.warmup_bars,
    config: config.config || {}
  }
}
async function goLive(session) {
  startingLive.value = session.id
  try {
    await post('/api/live', livePayload(session))
    emit('toast', 'Live trading session started.')
    emit('navigate', 'live')
  } catch (error) {
    emit('toast', error.message, 'error')
  } finally {
    startingLive.value = ''
  }
}
function requestDelete(session) {
  pendingDelete.value = { id: session.id, started_at: session.started_at }
}
async function destroy() {
  const target = pendingDelete.value
  if (!target) return
  deleting.value = true
  try {
    await remove(`/api/live/sessions/${encodeURIComponent(target.id)}`)
    pendingDelete.value = null
    emit('toast', 'Session deleted.')
    await load()
  } catch (error) {
    emit('toast', error.message, 'error')
  } finally {
    deleting.value = false
  }
}
onMounted(load)
let activationCount = 0
onActivated(() => {
  if (activationCount++) load()
})
</script>
