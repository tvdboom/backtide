<template>
  <div class="app-shell" :class="{ 'sidebar-open': sidebarOpen }">
    <aside class="sidebar">
      <button type="button" class="brand" aria-label="Go to Home" @click="navigate('home')">
        <img src="/backtide-logo.png" class="brand-logo" alt="Backtide" />
      </button>
      <nav aria-label="Main navigation">
        <template v-for="group in navigation" :key="group.label">
          <div class="nav-label">{{ group.label }}</div>
          <button
            v-for="item in group.items"
            :key="item.id"
            type="button"
            :class="{ active: page === item.id }"
            @click="navigate(item.id)"
          >
            <component :is="item.icon" :size="18" />
            <span>{{ item.label }}</span>
          </button>
        </template>
      </nav>
      <div class="sidebar-footer">
        <a href="https://tvdboom.github.io/backtide" target="_blank" rel="noreferrer">
          <BookOpen :size="16" /> Docs
        </a>
        <a href="https://github.com/tvdboom/backtide" target="_blank" rel="noreferrer">
          <Github :size="16" /> GitHub
        </a>
      </div>
    </aside>
    <button class="backdrop" aria-label="Close menu" @click="sidebarOpen = false" />

    <main>
      <header class="topbar">
        <button
          class="icon-button mobile-menu"
          aria-label="Open menu"
          @click="sidebarOpen = true"
        ><Menu /></button>
        <div>
          <span class="eyebrow">{{ current.group }}</span>
          <h1>{{ current.label }}</h1>
        </div>
        <div class="topbar-actions" :class="{ 'has-live-session': liveSessionRunning }">
          <button
            v-if="liveSessionVisible"
            type="button"
            class="connection live-session-link"
            aria-label="Open active live trading session"
            @click="navigate('live')"
          >
            <span :class="{ online: liveSessionRunning }" />{{ liveSessionLabel }}
          </button>
          <button
            type="button"
            class="icon-button theme-toggle"
            :aria-label="`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`"
            :title="`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`"
            :aria-pressed="theme === 'light'"
            @click="toggleTheme"
          >
            <Sun v-if="theme === 'dark'" :size="17" />
            <Moon v-else :size="17" />
          </button>
          <button class="icon-button" aria-label="Refresh page" @click="refreshKey++">
            <RefreshCw :size="17" />
          </button>
        </div>
      </header>

      <section v-if="fatalError" class="fatal-state">
        <TriangleAlert :size="30" />
        <h2>Backtide could not start</h2>
        <p>{{ fatalError }}</p>
        <button class="primary" @click="load">Try again</button>
      </section>
      <section v-else-if="!bootstrap" class="loading-screen">
        <span class="spinner" /> Loading your trading workspace…
      </section>
      <KeepAlive v-else :max="20">
        <component
          :is="current.component"
          :key="`${page}-${refreshKey}`"
          :bootstrap="bootstrap"
          @dismiss-toast="toast = null"
          @live-status="setLiveStatus"
          @navigate="navigate"
          @toast="showToast"
        />
      </KeepAlive>
    </main>
    <Transition name="toast">
      <div
        v-if="toast"
        class="toast"
        :class="toast.kind"
        :role="toast.kind === 'error' ? 'alert' : 'status'"
        :aria-live="toast.kind === 'error' ? 'assertive' : 'polite'"
      >
        <TriangleAlert v-if="toast.kind === 'error'" :size="19" />
        <CircleAlert v-else-if="toast.kind === 'warning'" :size="19" />
        <CircleCheck v-else :size="19" />
        <span class="toast-copy">
          <strong>{{ toast.kind === 'error' ? 'Error' : toast.kind === 'warning' ? 'Check this' : 'Done' }}</strong>
          <span>{{ toast.message }}</span>
        </span>
        <button type="button" aria-label="Dismiss notification" @click="toast = null"><X :size="15" /></button>
      </div>
    </Transition>
  </div>
</template>

<script setup>
import {
  Activity,
  BarChart3,
  BookOpen,
  Bot,
  CircleAlert,
  CircleCheck,
  CloudDownload,
  Database,
  FlaskConical,
  Gauge,
  Github,
  Home,
  Menu,
  Sigma,
  Moon,
  RefreshCw,
  Scale,
  Shapes,
  Sun,
  TriangleAlert,
  X
} from 'lucide-vue-next'
import { computed, markRaw, onBeforeUnmount, onMounted, ref } from 'vue'
import { api } from './api'
import AnalysisPage from './pages/analysis-page.vue'
import DashboardPage from './pages/dashboard-page.vue'
import DownloadPage from './pages/download-page.vue'
import ExperimentPage from './pages/experiment-page.vue'
import LibraryPage from './pages/library-page.vue'
import LivePage from './pages/live-page.vue'
import LiveHistoryPage from './pages/live-history-page.vue'
import ResultsPage from './pages/results-page.vue'
import StoragePage from './pages/storage-page.vue'
import { resolvePage } from './state'
import { applyTheme, persistTheme, resolveTheme } from './theme'

const navigation = [
  {
    label: 'Overview',
    items: [{ id: 'home', label: 'Home', icon: Home, component: markRaw(DashboardPage) }]
  },
  {
    label: 'Research',
    items: [
      { id: 'experiment', label: 'New experiment', icon: FlaskConical, component: markRaw(ExperimentPage) },
      { id: 'results', label: 'Results', icon: Gauge, component: markRaw(ResultsPage) },
      { id: 'analysis', label: 'Analysis', icon: BarChart3, component: markRaw(AnalysisPage) }
    ]
  },
  {
    label: 'Trading',
    items: [
      { id: 'live', label: 'Live trading', icon: Activity, component: markRaw(LivePage) },
      { id: 'live-history', label: 'Session history', icon: Gauge, component: markRaw(LiveHistoryPage) }
    ]
  },
  {
    label: 'Library',
    items: [
      { id: 'strategies', label: 'Strategies', icon: Bot, component: markRaw(LibraryPage) },
      { id: 'indicators', label: 'Indicators', icon: Shapes, component: markRaw(LibraryPage) },
      { id: 'metrics', label: 'Metrics', icon: Sigma, component: markRaw(LibraryPage) },
      { id: 'sizers', label: 'Sizers', icon: Scale, component: markRaw(LibraryPage) }
    ]
  },
  {
    label: 'Data',
    items: [
      { id: 'download', label: 'Download', icon: CloudDownload, component: markRaw(DownloadPage) },
      { id: 'storage', label: 'Storage', icon: Database, component: markRaw(StoragePage) }
    ]
  }
]
const flat = navigation.flatMap(group => group.items.map(item => ({ ...item, group: group.label })))
const page = ref(resolvePage(location.hash, flat.map(item => item.id)))
const current = computed(() => flat.find(item => item.id === page.value) || flat[0])
const bootstrap = ref(null)
const fatalError = ref('')
const liveSessionRunning = ref(false)
const liveSessionVisible = ref(false)
const liveSessionLabel = ref('Session live')
const refreshKey = ref(0)
const sidebarOpen = ref(false)
const toast = ref(null)
const theme = ref(document.documentElement.dataset.theme || resolveTheme())
let toastTimer, liveStatusTimer

async function load() {
  fatalError.value = ''
  try {
    bootstrap.value = await api('/api/bootstrap')
    void pollLiveStatus()
  } catch (error) {
    fatalError.value = error.message
  }
}

function setLiveStatus(value) {
  if (typeof value === 'boolean') {
    liveSessionRunning.value = value
    liveSessionVisible.value = value
    liveSessionLabel.value = 'Session live'
    return
  }
  const running = ['running', 'paused'].includes(value?.status)
  const replay = value?.config?.mode === 'replay'
  liveSessionRunning.value = running
  liveSessionVisible.value = running || replay
  liveSessionLabel.value = replay
    ? running ? 'Replay running' : value?.status === 'error' ? 'Replay failed' : 'Replay'
    : value?.status === 'paused' ? 'Session paused' : 'Session live'
}

async function pollLiveStatus() {
  try {
    const state = await api('/api/live')
    setLiveStatus(state)
  } catch {
    setLiveStatus(false)
  } finally {
    liveStatusTimer = setTimeout(pollLiveStatus, 2000)
  }
}

function navigate(next) {
  page.value = flat.some(item => item.id === next) ? next : 'home'
  location.hash = page.value
  sidebarOpen.value = false
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

function showToast(message, kind = 'success') {
  toast.value = { message, kind }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => { toast.value = null }, kind === 'error' ? 8000 : 4500)
}

function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  applyTheme(theme.value)
  persistTheme(theme.value)
}

onMounted(() => {
  addEventListener('hashchange', () => { page.value = resolvePage(location.hash, flat.map(item => item.id)) })
  load()
})
onBeforeUnmount(() => { clearTimeout(toastTimer); clearTimeout(liveStatusTimer) })
</script>
