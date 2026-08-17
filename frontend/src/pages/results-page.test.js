// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, markRaw, shallowRef } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ResultsPage from './results-page.vue'

const { api, post, query, remove } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  query: vi.fn(),
  remove: vi.fn()
}))

vi.mock('../api', () => ({ api, post, query, remove }))
vi.mock('../components/chart-panel.vue', () => ({
  default: { template: '<div class="chart-stub" />' }
}))

const summary = {
  id: 'experiment-1',
  name: 'Momentum study',
  icon: '🎯',
  status: 'Success',
  started_at: 1750000000,
  n_strategies: 1,
  best_sharpe: 1.42,
  runs: [{
    strategy_id: 'strategy-1',
    strategy_name: 'Trend engine',
    is_benchmark: false,
    base_currency: 'USD',
    metrics: {
      sharpe_ratio: 1.42, pnl: 1250, total_return: 0.12, cagr: 0.09,
      alpha: 0.03, max_drawdown: -0.08, n_trades: 14, win_rate: 0.57
    }
  }]
}

const detail = {
  experiment: {
    id: 'experiment-1',
    name: 'Momentum study',
    icon: '🎯',
    status: 'Success',
    tags: [],
    started_at: 1750000000,
    finished_at: 1750000090,
    best_sharpe: 1.42
  },
  config_metadata: {
    symbols: 2,
    instrument_type: 'stocks',
    interval: 'OneDay',
    full_history: false,
    start_date: '2024-01-01',
    end_date: '2024-03-01'
  },
  config: '[general]\nname = "Momentum study"',
  logs: '',
  runs: [{
    strategy_id: 'strategy-1',
    strategy_name: 'Trend engine',
    is_benchmark: false,
    base_currency: 'USD',
    metrics: { total_return: 0.12, sharpe_ratio: 1.42, pnl: 1250, max_drawdown: -0.08 },
    trades: [{ symbol: 'AAPL' }],
    orders: []
  }]
}

function mockOrderBatches(orders) {
  query.mockImplementation((path, params = {}) => {
    if (!path.endsWith('/orders')) return [summary]
    const offset = Number(params.offset || 0)
    const limit = Number(params.limit || 100)
    const page = orders.slice(offset, offset + limit)
    return {
      orders: page,
      offset,
      limit,
      total: orders.length,
      has_more: offset + page.length < orders.length
    }
  })
}

describe('results page', () => {
  let wrapper

  beforeEach(() => {
    sessionStorage.clear()
    query.mockReset().mockImplementation(path => path.endsWith('/orders')
      ? { orders: [], offset: 0, limit: 100, total: 0, has_more: false }
      : [summary])
    api.mockReset().mockImplementation(path => path === '/api/jobs' ? [] : detail)
    post.mockReset().mockImplementation((path) => {
      if (path === '/api/config/parse') return { general: { name: 'Momentum study' } }
      return { data: [], layout: {} }
    })
    remove.mockReset()
  })

  afterEach(() => {
    wrapper?.unmount()
    vi.unstubAllGlobals()
  })

  async function mountAndOpen(bootstrap = {}) {
    wrapper = mount(ResultsPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.get('.experiment-card-actions .secondary').trigger('click')
    await flushPromises()
    await flushPromises()
  }

  it('shows compact experiments and expands one strategy breakdown at a time', async () => {
    query.mockResolvedValue([
      summary,
      { ...summary, id: 'experiment-2', name: 'Second study' }
    ])
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()

    const [card, secondCard] = wrapper.findAll('.experiment-result-card')
    expect(card.text()).not.toContain('Trend engine')
    expect(card.get('.experiment-avatar').text()).toBe('🎯')
    expect(api).not.toHaveBeenCalledWith('/api/experiments/experiment-1')
    expect(card.get('.experiment-result-title-line h3').text()).toBe('Momentum study')
    expect(card.get('.experiment-result-title-line .result-status').text()).toContain('Success')
    expect(card.get('.experiment-result-meta').text()).not.toContain('Success')
    expect(card.get('.experiment-result-meta').text()).toContain('Sharpe 1.42')
    expect(card.get('.experiment-result-meta').text()).not.toContain('Best Sharpe')

    await card.get('.breakdown-toggle').trigger('click')
    expect(card.text()).toContain('Trend engine')
    expect(card.text()).toContain('1.42')
    expect(card.text()).toContain('1,250')
    expect(card.text()).toContain('12.00%')
    expect(card.text()).toContain('-8.00%')
    expect(card.findAll('.run-summary-metrics > div')).toHaveLength(7)
    expect(api).not.toHaveBeenCalledWith('/api/experiments/experiment-1')

    await secondCard.get('.breakdown-toggle').trigger('click')
    expect(card.find('.experiment-breakdown').exists()).toBe(false)
    expect(secondCard.find('.experiment-breakdown').exists()).toBe(true)
  })

  it('shows a loading state before the first experiment page resolves', async () => {
    let resolveExperiments
    query.mockImplementation(path => path.endsWith('/orders')
      ? { orders: [], offset: 0, limit: 100, total: 0, has_more: false }
      : new Promise(resolve => { resolveExperiments = resolve }))

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })

    expect(wrapper.get('.results-list-loading').text()).toContain('Loading experiments')
    expect(wrapper.find('.experiment-result-card').exists()).toBe(false)

    resolveExperiments([summary])
    await flushPromises()

    expect(wrapper.find('.results-list-loading').exists()).toBe(false)
    expect(wrapper.get('.experiment-result-card').text()).toContain('Momentum study')
  })

  it('loads experiment summaries in ten-item batches at the scroll sentinel', async () => {
    const observers = []
    vi.stubGlobal('IntersectionObserver', class {
      constructor(callback) {
        this.callback = callback
        this.observe = vi.fn()
        this.disconnect = vi.fn()
        observers.push(this)
      }
    })
    const summaries = Array.from({ length: 12 }, (_, index) => ({
      ...summary,
      id: `experiment-${index + 1}`,
      name: `Study ${index + 1}`
    }))
    query.mockImplementation((path, params = {}) => {
      if (path.endsWith('/orders')) {
        return { orders: [], offset: 0, limit: 100, total: 0, has_more: false }
      }
      const offset = Number(params.offset || 0)
      return summaries.slice(offset, offset + Number(params.limit || 10))
    })

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(query).toHaveBeenCalledWith('/api/experiments', { search: '', offset: 0, limit: 10 })
    expect(wrapper.findAll('.experiment-result-card')).toHaveLength(10)

    observers[0].callback([{ isIntersecting: true }])
    await flushPromises()

    expect(query).toHaveBeenCalledWith('/api/experiments', { search: '', offset: 10, limit: 10 })
    expect(wrapper.findAll('.experiment-result-card')).toHaveLength(12)
    expect(wrapper.text()).toContain('Study 12')
  })

  it('shows a benchmark symbol without a duplicated benchmark label', async () => {
    query.mockResolvedValue([{
      ...summary,
      runs: [{ ...summary.runs[0], strategy_name: 'SPY', is_benchmark: true }]
    }])
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()

    await wrapper.get('.breakdown-toggle').trigger('click')

    const heading = wrapper.get('.run-breakdown-card > header')
    expect(heading.get('strong').text()).toBe('SPY')
    expect(heading.find('small').exists()).toBe(false)
  })

  it('uses an icon instead of a duplicated benchmark pill in the strategy selector', async () => {
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      runs: [{ ...detail.runs[0], strategy_name: 'SPY', is_benchmark: true }]
    })
    await mountAndOpen({ display: { logokit_api_key: 'test-token' } })

    const benchmark = wrapper.get('.strategy-switcher button')
    expect(benchmark.text()).toBe('SPY')
    expect(benchmark.find('svg').exists()).toBe(true)
    expect(benchmark.find('.badge').exists()).toBe(false)
  })

  it('opens a separate detail view and returns to the experiment overview', async () => {
    await mountAndOpen()

    expect(wrapper.text()).toContain('Experiment overview')
    expect(wrapper.text()).toContain('Strategies')
    expect(wrapper.get('.result-workspace').text()).toContain('Rolling Sharpe')
    expect(wrapper.get('.result-workspace').text()).toContain('Dividends')
    expect(wrapper.get('.result-workspace').text()).toContain('Cumulative profit and loss over time for each strategy.')
    expect(wrapper.findAll('.result-workspace')[1].text()).toContain('Trades on price')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Sharpe1.42')
    expect(wrapper.get('.result-overview-metrics').text()).not.toContain('Best Sharpe')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('StatusSuccess')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Strategies1')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Symbols2')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Period2024-01-01 → 2024-03-01 (61d)')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Interval1d')
    expect(wrapper.findAll('.primary-metrics .result-overview-metric').map(item => item.get('span').text())).toEqual([
      'Sharpe', 'Period', 'Interval', 'Status'
    ])
    expect(wrapper.findAll('.context-metrics .result-overview-metric').map(item => item.get('span').text())).toEqual([
      'Strategies', 'Symbols', 'Started at', 'Duration'
    ])
    const overviewMetrics = wrapper.findAll('.result-overview-metric')
    const periodMetric = overviewMetrics.find(item => item.get('span').text() === 'Period')
    const startedAtMetric = overviewMetrics.find(item => item.get('span').text() === 'Started at')
    expect(periodMetric.get('svg').classes()).toContain('lucide-calendar-range-icon')
    expect(startedAtMetric.get('svg').classes()).toContain('lucide-calendar-days-icon')
    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'pnl' }))
    expect(post).not.toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'mae_mfe' }))

    const strategyTabs = wrapper.findAll('.strategy-plot-tabs button').map(button => button.text())
    expect(strategyTabs).toEqual(['Metrics', 'MAE / MFE', 'Position size', 'Trades on price', 'Orders'])
    expect(wrapper.findAll('.result-workspace')[1].get('.result-table').exists()).toBe(true)
    expect(wrapper.get('.result-metrics').text()).toContain('PnL')
    expect(wrapper.get('.result-metrics').text()).toContain('1,250.00')
    expect(wrapper.findAll('.result-metrics .metric-card small')).toHaveLength(0)

    await wrapper.get('.results-back').trigger('click')
    expect(wrapper.find('.result-detail-page').exists()).toBe(false)
    expect(wrapper.get('.experiment-result-card').exists()).toBe(true)
  })

  it('formats result, period, trade, and order dates from display configuration', async () => {
    const startedAt = Date.UTC(2026, 7, 11, 19, 5) / 1000
    mockOrderBatches([{
      timestamp: startedAt,
      status: 'Filled',
      fill_price: 100,
      commission: 0,
      pnl: 5,
      order: { symbol: 'AAPL', order_type: 'Market', quantity: 1 }
    }])
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      experiment: { ...detail.experiment, started_at: startedAt },
      runs: [{
        ...detail.runs[0],
        trades: [{ symbol: 'AAPL', entry_ts: startedAt, exit_ts: startedAt + 3600 }]
      }]
    })
    await mountAndOpen({
      display: {
        date_format: 'DD/MM/YYYY',
        datetime_format: 'DD/MM/YYYY HH:MM',
        timezone: 'UTC'
      }
    })

    expect(wrapper.get('.result-overview-metrics').text()).toContain('Started at11/08/2026 19:05')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('Period01/01/2024 → 01/03/2024')

    const tabs = wrapper.findAll('.strategy-plot-tabs button')
    await tabs.find(button => button.text() === 'Orders').trigger('click')
    await flushPromises()
    expect(wrapper.get('.result-orders-table').text()).toContain('11/08/2026 19:05')
  })

  it('renders orders as the legacy formatted table without nested JSON', async () => {
    mockOrderBatches([
      {
        timestamp: 1750000060,
        status: 'Filled',
        fill_price: 100,
        commission: 1.5,
        pnl: 50,
        order: { id: 'buy-1', symbol: 'AAPL', order_type: 'Market', quantity: 10, price: null }
      },
      {
        timestamp: 1750000000,
        status: 'Pending',
        fill_price: 95,
        commission: 0,
        pnl: -12,
        order: { id: 'sell-1', symbol: 'MSFT', order_type: 'Limit', quantity: -2, price: 95 }
      }
    ])
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      runs: [{
        ...detail.runs[0],
        base_currency: 'USD'
      }]
    })
    await mountAndOpen({ display: { logokit_api_key: 'test-token' } })

    const ordersTab = wrapper.findAll('.strategy-plot-tabs button').find(button => button.text() === 'Orders')
    await ordersTab.trigger('click')
    await flushPromises()

    const table = wrapper.get('.result-orders-table')
    expect(table.findAll('th').map(header => header.text())).toEqual([
      'Symbol', 'Datetime', 'Type', 'Side', 'Qty', 'Price', 'PnL', 'Commission', 'Status'
    ])
    const rows = table.findAll('tbody tr')
    expect(rows).toHaveLength(2)
    expect(rows[0].get('.order-symbol-cell').text()).toBe('AAPL')
    expect(rows[0].get('.order-symbol-logo').exists()).toBe(true)
    expect(rows[0].text()).toContain('AAPL')
    expect(rows[0].text()).toContain('Buy')
    expect(rows[0].findAll('td')[5].text()).toContain('1,000.00')
    expect(rows[0].findAll('td')[6].classes()).toContain('positive')
    expect(rows[0].get('.execution-status .badge').classes()).toContain('success')
    expect(rows[1].text()).toContain('MSFT')
    expect(rows[1].text()).toContain('Sell')
    expect(rows[1].findAll('td')[6].classes()).toContain('negative')
    expect(rows[1].findAll('td')[8].classes()).toContain('warning')
    expect(rows[1].get('.execution-status .badge').classes()).toContain('partial')
    expect(table.text()).not.toContain('order_type')
    expect(table.text()).not.toContain('{')
  })

  it('loads orders in batches of 100 as the table reaches the bottom', async () => {
    const orders = Array.from({ length: 250 }, (_, index) => ({
      timestamp: 1750001000 - index,
      status: 'Filled',
      fill_price: 100,
      commission: 0,
      pnl: index,
      order: { id: `order-${index}`, symbol: 'AAPL', order_type: 'Market', quantity: 1 }
    }))
    mockOrderBatches(orders)
    await mountAndOpen()

    const ordersTab = wrapper.findAll('.strategy-plot-tabs button').find(button => button.text() === 'Orders')
    await ordersTab.trigger('click')
    await flushPromises()

    const table = wrapper.get('.result-orders-table')
    expect(table.findAll('tbody tr')).toHaveLength(100)
    expect(query).toHaveBeenCalledWith('/api/experiments/experiment-1/orders', {
      strategy_id: 'strategy-1', offset: 0, limit: 100
    })

    Object.defineProperties(table.element, {
      scrollTop: { value: 500, writable: true },
      clientHeight: { value: 500 },
      scrollHeight: { value: 1000 }
    })
    await table.trigger('scroll')
    await flushPromises()

    expect(table.findAll('tbody tr')).toHaveLength(200)
    expect(query).toHaveBeenCalledWith('/api/experiments/experiment-1/orders', {
      strategy_id: 'strategy-1', offset: 100, limit: 100
    })
  })

  it('opens a dashboard-requested result directly', async () => {
    sessionStorage.setItem('backtide:result-id', 'experiment-1')
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    expect(wrapper.get('.result-detail-page').text()).toContain('Momentum study')
    expect(sessionStorage.getItem('backtide:result-id')).toBeNull()
  })

  it('returns a cached result page to the overview after a new experiment is queued', async () => {
    const away = markRaw({ template: '<div class="away" />' })
    const current = shallowRef(markRaw(ResultsPage))
    const Host = defineComponent({
      setup: () => ({ current }),
      template: '<KeepAlive><component :is="current" :bootstrap="{}" /></KeepAlive>'
    })
    wrapper = mount(Host)
    await flushPromises()
    await wrapper.get('.experiment-card-actions .secondary').trigger('click')
    await flushPromises()
    expect(wrapper.find('.result-detail-page').exists()).toBe(true)

    current.value = away
    await wrapper.vm.$nextTick()
    sessionStorage.setItem('backtide:results-overview', 'true')
    current.value = markRaw(ResultsPage)
    await wrapper.vm.$nextTick()
    await flushPromises()

    expect(wrapper.find('.result-detail-page').exists()).toBe(false)
    expect(wrapper.get('.experiment-result-card').text()).toContain('Momentum study')
  })

  it('opens a new experiment with the saved configuration', async () => {
    await mountAndOpen()

    const [reuseButton, configButton] = wrapper.findAll('.result-actions .secondary')
    expect(reuseButton.text()).toContain('Reuse setup')
    expect(reuseButton.classes()).toEqual(configButton.classes())
    await reuseButton.trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/config/parse', {
      suffix: '.toml',
      text: detail.config
    })
    expect(JSON.parse(sessionStorage.getItem('backtide:experiment-config'))).toEqual({
      general: { name: 'Momentum study' }
    })
    expect(wrapper.emitted('navigate')).toContainEqual(['experiment'])
  })

  it('keeps tags below the title, moves status to metrics, and omits an empty description', async () => {
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      experiment: { ...detail.experiment, description: '   ', tags: ['momentum', 'daily'] }
    })

    await mountAndOpen()

    const summaryBlock = wrapper.get('.result-heading-copy > div')
    expect(summaryBlock.element.children[0].tagName).toBe('H2')
    expect(summaryBlock.element.children[1].classList).toContain('result-title')
    expect(summaryBlock.get('.result-title').text()).not.toContain('Success')
    expect(summaryBlock.get('.result-title').text()).toContain('momentum')
    expect(wrapper.get('.result-overview-metrics').text()).toContain('StatusSuccess')
    expect(summaryBlock.find('p').exists()).toBe(false)
    expect(summaryBlock.text()).not.toContain('No description was provided.')
  })

  it('opens an existing empty log artifact and explains that it has no entries', async () => {
    await mountAndOpen()

    const logsButton = wrapper.findAll('.result-actions .secondary').find(button => button.text().includes('Logs'))
    expect(logsButton.attributes('disabled')).toBeUndefined()
    await logsButton.trigger('click')

    expect(wrapper.get('.document-modal').text()).toContain('Engine logs')
    expect(wrapper.get('.document-empty').text()).toContain('Log file is empty')
  })

  it('opens large logs with a bounded 1,000-line preview', async () => {
    const logs = Array.from({ length: 1_500 }, (_, index) => `log line ${index}`).join('\n')
    api.mockImplementation(path => path === '/api/jobs' ? [] : { ...detail, logs })
    await mountAndOpen()

    const logsButton = wrapper.findAll('.result-actions .secondary').find(button => button.text().includes('Logs'))
    await logsButton.trigger('click')

    const preview = wrapper.get('.document-modal pre').text()
    expect(preview.split('\n')).toHaveLength(1_000)
    expect(preview).toMatch(/^log line 500/)
    expect(preview).toMatch(/log line 1499$/)
    expect(wrapper.get('.document-note').text()).toContain('limited to 1,000 lines')
    expect(wrapper.get('.document-note').text()).toContain('Download the full log')
    const download = wrapper.get('.document-modal-actions a')
    expect(download.text()).toContain('Download full log')
    expect(download.attributes('href')).toBe('/api/experiments/experiment-1/logs')
    expect(download.attributes()).toHaveProperty('download')
  })

  it('shows only plot names in tabs and the selected description below them', async () => {
    await mountAndOpen()

    const workspace = wrapper.findAll('.result-workspace')[0]
    const tabs = workspace.findAll('.result-plot-tabs button')
    expect(tabs[0].text()).toBe('PnL')
    expect(workspace.get('.result-plot-description').text()).toBe('Cumulative profit and loss over time for each strategy.')

    await tabs[1].trigger('click')

    expect(workspace.get('.result-plot-description').text()).toBe('Cash balance timeline by strategy and settlement currency.')
  })

  it('loads the dividends plot for the selected experiment', async () => {
    await mountAndOpen()
    post.mockClear()

    const dividends = wrapper.findAll('.result-workspace')[0]
      .findAll('.result-plot-tabs button')
      .find(button => button.text() === 'Dividends')
    await dividends.trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({
      experiment_id: 'experiment-1',
      plot: 'dividends'
    }))
  })

  it('places active plot controls in a right-side options region', async () => {
    await mountAndOpen()

    const workspace = wrapper.findAll('.result-workspace')[0]
    expect(workspace.get('.result-plot-stage').classes()).toContain('has-options')
    expect(workspace.get('.result-plot-stage').element.children[0].classList).toContain('chart-stub')
    expect(workspace.get('.result-plot-stage').element.children[1].classList).toContain('result-plot-options')
    expect(workspace.get('.result-plot-options').text()).toContain('Normalize')
    expect(workspace.findAll('.result-plot-tabs button svg').length).toBeGreaterThan(0)

    const normalize = workspace.findAll('.toggle-label')[0]
    expect(normalize.get('.toggle-description').text()).toContain('starting equity')
    expect(normalize.get('.field-info').exists()).toBe(true)
    post.mockClear()
    await normalize.get('.toggle').setValue(true)
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({
      options: expect.objectContaining({ normalize: true })
    }))
  })

  it('shows experiment details while plots are still loading', async () => {
    const resolvePlots = []
    post.mockImplementation(path => {
      if (path === '/api/config/parse') return { general: { name: 'Momentum study' } }
      return new Promise(resolve => { resolvePlots.push(resolve) })
    })

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await wrapper.get('.experiment-card-actions .secondary').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Momentum study')
    expect(wrapper.text()).not.toContain('Loading results')

    resolvePlots.forEach(resolve => resolve({ data: [], layout: {} }))
    await flushPromises()
  })

  it('asks in-page before deleting an experiment', async () => {
    await mountAndOpen()

    await wrapper.get('[aria-label="Delete experiment"]').trigger('click')

    expect(wrapper.get('[role="alertdialog"]').text()).toContain('Delete Momentum study?')
    expect(remove).not.toHaveBeenCalled()

    await wrapper.get('.confirm-submit').trigger('click')
    await flushPromises()

    expect(remove).toHaveBeenCalledWith('/api/experiments/experiment-1')
    expect(wrapper.get('.experiment-result-card').exists()).toBe(true)
  })

  it('opens Live trading with settings from the selected experiment', async () => {
    const draft = {
      provider: 'kraken',
      symbols: ['BTC-USD'],
      strategies: ['Trend engine'],
      config: { initial_cash: 100000 }
    }
    api.mockImplementation(path => {
      if (path === '/api/jobs') return []
      if (path.endsWith('/paper-config')) return draft
      return detail
    })
    await mountAndOpen()

    const liveButton = wrapper.findAll('.result-actions .secondary')
      .find(button => button.text().includes('Live trading'))
    await liveButton.trigger('click')
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/experiments/experiment-1/paper-config')
    expect(JSON.parse(sessionStorage.getItem('backtide:paper-config'))).toEqual(draft)
    expect(wrapper.emitted('navigate')[0]).toEqual(['live'])
  })
})
