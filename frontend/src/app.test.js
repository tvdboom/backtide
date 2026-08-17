// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './app.vue'
import { THEME_STORAGE_KEY } from './theme'

const api = vi.hoisted(() => vi.fn(() => new Promise(() => {})))

vi.mock('./api', () => ({ api }))
vi.mock('./pages/analysis-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/dashboard-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/download-page.vue', () => ({ default: { name: 'DownloadPage', template: '<input aria-label="Download state" />' } }))
vi.mock('./pages/experiment-page.vue', () => ({ default: {
  props: ['bootstrap'],
  template: '<div class="experiment-catalog">{{ bootstrap && bootstrap.strategies ? bootstrap.strategies.saved.map(item => item.name).join(",") : "" }}</div>'
} }))
vi.mock('./pages/library-page.vue', () => ({ default: {
  emits: ['catalog-updated'],
  template: '<button class="publish-catalog" @click="$emit(\'catalog-updated\', { key: \'strategies\', catalog: { builtin: [], saved: [{ name: \'Fresh strategy\' }] } })">Publish</button>'
} }))
vi.mock('./pages/live-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/live-history-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/results-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/storage-page.vue', () => ({ default: { name: 'StoragePage', template: '<div>Storage page</div>' } }))

describe('App theme control', () => {
  beforeEach(() => {
    api.mockReset().mockImplementation(() => new Promise(() => {}))
    localStorage.clear()
    location.hash = '#home'
    window.scrollTo = vi.fn()
    document.documentElement.dataset.theme = 'dark'
    document.documentElement.style.colorScheme = 'dark'
  })

  it('switches to light mode and saves the choice from the top bar', async () => {
    const wrapper = mount(App)
    const toggle = wrapper.get('[aria-label="Switch to light mode"]')

    await toggle.trigger('click')

    expect(document.documentElement.dataset.theme).toBe('light')
    expect(document.documentElement.style.colorScheme).toBe('light')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('light')
    expect(toggle.attributes('aria-label')).toBe('Switch to dark mode')
    expect(toggle.attributes('aria-pressed')).toBe('true')
    wrapper.unmount()
  })

  it('preserves a page widget value after navigating away and back', async () => {
    api.mockResolvedValueOnce({})
    const wrapper = mount(App)
    await flushPromises()

    const downloadButton = wrapper.findAll('nav button').find(button => button.text().includes('Download'))
    const storageButton = wrapper.findAll('nav button').find(button => button.text().includes('Storage'))
    await downloadButton.trigger('click')
    await wrapper.get('[aria-label="Download state"]').setValue('INGA.AS')
    await storageButton.trigger('click')
    await downloadButton.trigger('click')

    expect(wrapper.get('[aria-label="Download state"]').element.value).toBe('INGA.AS')
    wrapper.unmount()
  })

  it('groups workflows and reusable assets by product role', async () => {
    api.mockResolvedValueOnce({})
    const wrapper = mount(App)
    await flushPromises()

    const labels = wrapper.findAll('.nav-label').map(label => label.text())
    expect(labels).toEqual(['Overview', 'Research', 'Trading', 'Library', 'Data'])
    expect(wrapper.text()).toContain('New experiment')
    expect(wrapper.text()).toContain('Live trading')
    expect(wrapper.text()).toContain('Strategies')
    expect(wrapper.text()).toContain('Indicators')
    expect(wrapper.text()).toContain('Metrics')
    expect(wrapper.text()).toContain('Sizers')
    wrapper.unmount()
  })

  it('shares newly saved library assets with the experiment builder immediately', async () => {
    api.mockImplementation(path => Promise.resolve(path === '/api/live'
      ? { status: 'stopped' }
      : { strategies: { builtin: [], saved: [] } }))
    const wrapper = mount(App)
    await flushPromises()

    const strategies = wrapper.findAll('nav button')
      .find(button => button.text().includes('Strategies'))
    const experiment = wrapper.findAll('nav button')
      .find(button => button.text().includes('New experiment'))
    await strategies.trigger('click')
    await wrapper.get('.publish-catalog').trigger('click')
    await experiment.trigger('click')

    expect(wrapper.get('.experiment-catalog').text()).toContain('Fresh strategy')
    wrapper.unmount()
  })

  it('shows an active live session in the top bar and opens live trading', async () => {
    api.mockImplementation(path => Promise.resolve(path === '/api/live' ? { status: 'running' } : {}))
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.text()).not.toContain('Local engine')
    const liveSession = wrapper.get('[aria-label="Open active live trading session"]')
    expect(liveSession.text()).toContain('Session live')
    expect(liveSession.get('span').classes()).toContain('online')

    await liveSession.trigger('click')

    expect(location.hash).toBe('#live')
    wrapper.unmount()
  })

  it('keeps a replay accessible from the top bar', async () => {
    api.mockImplementation(path => Promise.resolve(path === '/api/live'
      ? { status: 'stopped', config: { mode: 'replay' } }
      : {}))
    const wrapper = mount(App)
    await flushPromises()

    const replay = wrapper.get('[aria-label="Open active live trading session"]')
    expect(replay.text()).toContain('Replay')
    expect(replay.text()).not.toContain('complete')
    expect(replay.get('span').classes()).not.toContain('online')
    wrapper.unmount()
  })
})
