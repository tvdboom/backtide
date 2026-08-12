<template>
  <div class="page">
    <section class="page-intro"><div><h2>Session history</h2><p>Inspect and replay locally persisted paper-trading sessions.</p></div><button class="secondary" @click="load"><RefreshCw :size="16"/> Refresh</button></section>
    <section v-if="loading" class="panel empty-state large"><span class="spinner"/><p>Loading paper sessions...</p></section>
    <section v-else-if="!sessions.length" class="panel empty-state large"><History :size="30"/><h3>No saved sessions</h3><p>Completed paper sessions will appear here automatically.</p></section>
    <section v-else class="panel table-panel">
      <div class="data-table-wrap">
        <table class="data-table">
          <thead><tr><th>Started</th><th>Finished</th><th>Mode</th><th>Strategies</th><th>Status</th><th class="number">Final equity</th><th></th></tr></thead>
          <tbody><tr v-for="session in sessions" :key="session.id"><td>{{ dateTime(session.started_at) }}</td><td>{{ session.finished_at ? dateTime(session.finished_at) : '—' }}</td><td>{{ modeName(session.config?.mode) }}</td><td>{{ session.config?.strategies?.join(', ') || 'Monitor only' }}</td><td><span class="badge" :class="session.status === 'error' ? 'error' : 'neutral'">{{ session.status }}</span></td><td class="number">{{ money(session.snapshot?.equity, session.config?.config?.base_currency) }}</td><td><div class="session-history-actions"><button class="secondary compact-button" :disabled="Boolean(replaying || goingLive)" @click="replay(session.id)"><RotateCcw :size="14"/> {{ replaying === session.id ? 'Starting...' : 'Replay' }}</button><button class="primary compact-button" :disabled="Boolean(replaying || goingLive)" @click="goLive(session)"><Radio :size="14"/> {{ goingLive === session.id ? 'Connecting...' : 'Go live' }}</button></div></td></tr></tbody>
        </table>
      </div>
    </section>
  </div>
</template>

<script setup>
import { History, Radio, RefreshCw, RotateCcw } from 'lucide-vue-next'
import { onMounted, ref } from 'vue'
import { api, post } from '../api'
import { formatConfiguredDateTime } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast', 'navigate', 'live-status'])
const sessions = ref([])
const loading = ref(false)
const replaying = ref('')
const goingLive = ref('')

function dateTime(value) { return formatConfiguredDateTime(value, props.bootstrap?.display) }
function modeName(value) { return value === 'replay' ? 'Replay' : 'Live paper' }
function money(value, currency = 'USD') { return new Intl.NumberFormat('en', { style: 'currency', currency: currency || 'USD', maximumFractionDigits: 2 }).format(Number(value) || 0) }
async function load() { loading.value = true; try { sessions.value = await api('/api/live/sessions') } catch (error) { emit('toast', error.message, 'error') } finally { loading.value = false } }
async function replay(id) { replaying.value = id; try { const state = await post('/api/live/replay', { session_id: id }); emit('live-status', state); emit('toast', 'Replay started.'); emit('navigate', 'live') } catch (error) { emit('toast', error.message, 'error') } finally { replaying.value = '' } }
async function goLive(session) {
  goingLive.value = session.id
  const saved = session.config || {}
  const payload = {
    provider: saved.provider,
    interval: saved.interval,
    symbols: [...(saved.symbols || [])],
    strategies: saved.strategies?.length
      ? [...saved.strategies]
      : saved.strategy ? [saved.strategy] : [],
    indicators: [...(saved.indicators || [])],
    warmup_bars: saved.warmup_bars ?? 0,
    config: { ...(saved.config || {}) }
  }
  try {
    const state = await post('/api/live', payload)
    emit('live-status', state)
    emit('toast', 'Live session started from saved settings.')
    emit('navigate', 'live')
  } catch (error) {
    emit('toast', error.message, 'error')
  } finally {
    goingLive.value = ''
  }
}
onMounted(load)
</script>
