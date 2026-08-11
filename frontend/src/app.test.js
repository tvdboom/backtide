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
vi.mock('./pages/experiment-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/library-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/live-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/results-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/storage-page.vue', () => ({ default: { name: 'StoragePage', template: '<div>Storage page</div>' } }))

describe('App theme control', () => {
  beforeEach(() => {
    api.mockClear()
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
})
