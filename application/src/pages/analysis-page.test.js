// @vitest-environment jsdom
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, ref } from 'vue'
import AnalysisPage from './analysis-page.vue'

const { api, post, query } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  query: vi.fn()
}))

vi.mock('../api', () => ({ api, post, query }))
vi.mock('../components/chart-panel.vue', () => ({
  default: { template: '<div class="chart-stub" />' }
}))
enableAutoUnmount(afterEach)

describe('analysis page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue([{
      symbol: 'AAPL',
      interval: '1d',
      provider: 'yahoo',
      name: 'Apple Inc.',
      instrument_type: 'stocks'
    }])
    query.mockReset().mockResolvedValue({})
    post.mockReset().mockImplementation((_endpoint, payload) => Promise.resolve(
      payload.plot === 'metrics'
        ? { rows: [{
            symbol: 'AAPL',
            sharpe: 1.24,
            cagr: 0.137,
            max_dd: -0.082,
            win_rate: 0.54,
            ann_volatility: 0.183,
            sortino: 1.62,
            total_bars: 252
          }] }
        : { data: [], layout: {} }
    ))
    sessionStorage.clear()
  })

  it('does not show a series count beside the plot title', async () => {
    const wrapper = mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.find('.chart-title .badge').exists()).toBe(false)
    expect(wrapper.get('.chart-title').text()).not.toContain('1 series')
    expect(wrapper.text()).not.toContain('Run analysis')
  })

  it('opens with the stock metrics table instead of a chart', async () => {
    const wrapper = mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.get('.chart-tabs button.active').text()).toContain('Metrics')
    expect(wrapper.find('.chart-stub').exists()).toBe(false)
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('AAPL')
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('1.24')
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('+13.70%')
    expect(wrapper.findAll('.analysis-metrics-table th').map(header => header.text())).toEqual([
      'Stock',
      'Sharpe',
      'CAGR',
      'Max drawdown',
      'Win rate',
      'Annualized volatility',
      'Sortino',
      'Total bars'
    ])
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('18.30%')
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('1.62')
    expect(wrapper.get('.analysis-metrics-table').text()).toContain('252')
    expect(post).toHaveBeenCalledWith('/api/analysis', expect.objectContaining({
      plot: 'metrics', symbols: ['AAPL']
    }))
  })

  it('adds a readable column for metrics introduced by the API', async () => {
    post.mockResolvedValue({ rows: [{ symbol: 'AAPL', tail_ratio: 1.15 }] })

    const wrapper = mount(AnalysisPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.findAll('.analysis-metrics-table th').map(header => header.text()))
      .toEqual(['Stock', 'Tail ratio'])
    expect(wrapper.get('.analysis-metrics-table tbody').text()).toContain('1.15')
  })

  it('shows instrument logos in the symbol menu and selected chips', async () => {
    const wrapper = mount(AnalysisPage, {
      props: { bootstrap: { display: { logokit_api_key: 'test-token' } } }
    })
    await flushPromises()

    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toContain('img.logokit.com/ticker/AAPL')

    await wrapper.get('.tag button').trigger('click')
    await wrapper.get('.tag-field input').trigger('focus')

    expect(wrapper.get('.search-option-logo img').attributes('src')).toContain('img.logokit.com/ticker/AAPL')
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

  it('selects the symbol requested after the cached page is reactivated', async () => {
    api.mockResolvedValue([
      { symbol: 'AAPL', interval: '1d', provider: 'yahoo', name: 'Apple Inc.' },
      { symbol: 'MSFT', interval: '15m', provider: 'yahoo', name: 'Microsoft Corp.' }
    ])
    const OtherPage = defineComponent({ template: '<div>Other page</div>' })
    const Host = defineComponent({
      components: { AnalysisPage, OtherPage },
      setup() {
        return { analysisActive: ref(true) }
      },
      template: `
        <KeepAlive>
          <AnalysisPage v-if="analysisActive" :bootstrap="{}" />
          <OtherPage v-else />
        </KeepAlive>
      `
    })
    const wrapper = mount(Host)
    await flushPromises()
    wrapper.vm.analysisActive = false
    await flushPromises()
    sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify(['MSFT']))
    sessionStorage.setItem('backtide:analysis-interval', '15m')

    wrapper.vm.analysisActive = true
    await flushPromises()

    expect(wrapper.get('.tag').text()).toContain('MSFT')
    expect(wrapper.get('.tag').text()).not.toContain('AAPL')
    expect(wrapper.findAll('select')[0].element.value).toBe('15m')
    expect(sessionStorage.getItem('backtide:analysis-symbols')).toBeNull()
    wrapper.unmount()
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
    await flushPromises()
    await vi.runAllTimersAsync()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith('/api/analysis', expect.objectContaining({
      interval: '15m'
    }))

    await wrapper.findAll('select')[1].setValue('open')
    await flushPromises()
    await vi.runAllTimersAsync()
    await flushPromises()
    expect(post).toHaveBeenLastCalledWith('/api/analysis', expect.objectContaining({
      interval: '15m', price_col: 'open'
    }))
  })
})
