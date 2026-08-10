// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
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
})
