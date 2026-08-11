// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import AnalysisPage from './analysis-page.vue'

const { api, post } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn()
}))

vi.mock('../api', () => ({ api, post }))
vi.mock('../components/chart-panel.vue', () => ({
  default: { template: '<div class="chart-stub" />' }
}))

describe('analysis page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue([{
      symbol: 'AAPL',
      interval: '1d',
      provider: 'yahoo',
      name: 'Apple Inc.'
    }])
    post.mockReset().mockResolvedValue({ data: [], layout: {} })
    sessionStorage.clear()
  })

  it('does not show a series count beside the plot title', async () => {
    const wrapper = mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.find('.chart-title .badge').exists()).toBe(false)
    expect(wrapper.get('.chart-title').text()).not.toContain('1 series')
    expect(wrapper.text()).not.toContain('Run analysis')
  })

  it('preselects requested download symbols without conversion legs', async () => {
    api.mockResolvedValue([
      { symbol: 'AAPL', interval: '1d', provider: 'yahoo', name: 'Apple Inc.' },
      { symbol: 'EUR-USD', interval: '1d', provider: 'yahoo', name: 'EUR/USD' }
    ])
    sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify(['AAPL']))

    mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/analysis', expect.objectContaining({
      symbols: ['AAPL']
    }))
  })

  afterEach(() => vi.useRealTimers())

  it('uses the interval requested by a storage row', async () => {
    api.mockResolvedValue([
      { symbol: 'AAPL', interval: '1d', provider: 'yahoo', name: 'Apple Inc.' },
      { symbol: 'AAPL', interval: '15m', provider: 'yahoo', name: 'Apple Inc.' }
    ])
    sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify(['AAPL']))
    sessionStorage.setItem('backtide:analysis-interval', '15m')

    mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/analysis', expect.objectContaining({
      symbols: ['AAPL'], interval: '15m'
    }))
    expect(sessionStorage.getItem('backtide:analysis-interval')).toBeNull()
  })

  it('refreshes automatically when interval or price changes', async () => {
    vi.useFakeTimers()
    api.mockResolvedValue([
      { symbol: 'AAPL', interval: '1d', provider: 'yahoo', name: 'Apple Inc.' },
      { symbol: 'AAPL', interval: '15m', provider: 'yahoo', name: 'Apple Inc.' }
    ])
    const wrapper = mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()
    post.mockClear()

    await wrapper.findAll('select')[0].setValue('15m')
    await vi.runAllTimersAsync()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith('/api/analysis', expect.objectContaining({
      interval: '15m'
    }))

    await wrapper.findAll('select')[1].setValue('open')
    await vi.runAllTimersAsync()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith('/api/analysis', expect.objectContaining({
      interval: '15m', price_col: 'open'
    }))
  })
})
