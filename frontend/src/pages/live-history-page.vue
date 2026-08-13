<template>
  <div class="page">
    <section class="page-intro"><div><h2>Session history</h2><p>Inspect and replay locally persisted paper-trading sessions.</p></div><button class="secondary" @click="load"><RefreshCw :size="16"/> Refresh</button></section>
    <section v-if="loading" class="panel empty-state large"><span class="spinner"/><p>Loading paper sessions...</p></section>
    <section v-else-if="!sessions.length" class="panel empty-state large"><History :size="30"/><h3>No saved sessions</h3><p>Completed paper sessions will appear here automatically.</p></section>
    <section v-else class="panel table-panel">
      <div class="data-table-wrap">
        <table class="data-table session-history-table">
          <thead><tr><th>Started</th><th>Finished</th><th>Strategies</th><th>Status</th><th class="number">Final equity</th><th></th></tr></thead>
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
              <td class="number">{{ finalEquity(group.session) }}</td>
              <td><div v-if="isActive(group.session)" class="session-history-actions"><button class="primary compact-button" type="button" @click="openSession"><ExternalLink :size="14"/> Open</button></div></td>
            </tr>
            <template v-if="isExpanded(group.session.id)">
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
                <td class="number">{{ finalEquity(session) }}</td>
                <td><div v-if="isActive(session)" class="session-history-actions"><button class="primary compact-button" type="button" @click="openSession"><ExternalLink :size="14"/> Open</button></div></td>
              </tr>
            </template>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>

<script setup>
import { ChevronRight, ExternalLink, History, RefreshCw, RotateCcw } from 'lucide-vue-next'
import { computed, onMounted, ref } from 'vue'
import { api } from '../api'
import StrategySummary from '../components/strategy-summary.vue'
import { formatConfiguredDateTime } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'navigate'])
const sessions = ref([])
const loading = ref(false)
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
function money(value, currency = 'USD') { return new Intl.NumberFormat('en', { style: 'currency', currency: currency || 'USD', maximumFractionDigits: 2 }).format(Number(value) || 0) }
function strategyNames(session) {
  const config = session.config || {}
  return config.strategies?.length ? config.strategies : config.strategy ? [config.strategy] : []
}
function finalEquity(session) {
  return strategyNames(session).length
    ? money(session.snapshot?.equity, session.config?.config?.base_currency)
    : '—'
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
onMounted(load)
</script>
