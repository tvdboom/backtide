// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './app.vue'
import { THEME_STORAGE_KEY } from './theme'

const api = vi.hoisted(() => vi.fn(() => new Promise(() => {})))

vi.mock('./api', () => ({ api }))
vi.mock('./pages/analysis-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/dashboard-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/download-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/experiment-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/library-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/live-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/results-page.vue', () => ({ default: { template: '<div />' } }))
vi.mock('./pages/storage-page.vue', () => ({ default: { template: '<div />' } }))

describe('App theme control', () => {
  beforeEach(() => {
    api.mockClear()
    localStorage.clear()
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
})
