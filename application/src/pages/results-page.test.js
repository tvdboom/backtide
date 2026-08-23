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
  n_symbols: 2,
  best_sharpe: 1.42,
  primary_metric: 'sharpe',
  selected_metrics: ['total_return', 'pnl', 'cagr', 'alpha', 'max_dd', 'n_trades', 'win_rate', 'sharpe'],
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

const metricBootstrap = {
  metrics: {
    builtin: [
      { key: 'sharpe', name: 'Sharpe ratio', percentage: false },
      { key: 'total_return', name: 'Total return', percentage: true },
      { key: 'pnl', name: 'Profit and loss', percentage: false },
      { key: 'cagr', name: 'CAGR', percentage: true },
      { key: 'alpha', name: 'Alpha', percentage: true },
      { key: 'max_dd', name: 'Maximum drawdown', percentage: true },
      { key: 'n_trades', name: 'Trades', percentage: false },
      { key: 'win_rate', name: 'Win rate', percentage: true }
    ],
    saved: []
  }
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
    best_sharpe: 1.42,
    primary_metric: 'sharpe',
    selected_metrics: ['sharpe', 'max_dd', 'total_return', 'pnl']
  },
  config_metadata: {
    symbols: 2,
    symbol_values: ['AAPL', 'MSFT'],
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

  it('shows the running research name, elapsed time, progress, and remaining time', async () => {
    api.mockImplementation(path => path === '/api/jobs'
      ? [{
          id: 'job-1',
          kind: 'study',
          name: 'SMA parameter study',
          status: 'running',
          started_at: new Date(Date.now() - 65_000).toISOString(),
          progress_started_at: new Date(Date.now() - 30_000).toISOString(),
          progress_completed: 1,
          progress_total: 4,
          progress_unit: 'candidate runs'
        }]
      : detail)

    wrapper = mount(ResultsPage, { props: { bootstrap: metricBootstrap } })
    await flushPromises()

    const banner = wrapper.get('.running-banner')
    expect(banner.get('strong').text()).toBe('SMA parameter study')
    expect(banner.text()).toMatch(/Study running · 1m \d+s elapsed · ~1m \d+s remaining/)
    const progress = banner.get('[role="progressbar"]')
    expect(progress.attributes('aria-valuenow')).toBe('25')
    expect(progress.attributes('aria-valuetext')).toBe('1 of 4 candidate runs · 25.0%')
    expect(progress.get('span').attributes('style')).toContain('width: 25%')
  })

  it('shows compact experiments and expands one strategy breakdown at a time', async () => {
    query.mockResolvedValue([
      summary,
      { ...summary, id: 'experiment-2', name: 'Second study' }
    ])
    wrapper = mount(ResultsPage, { props: { bootstrap: metricBootstrap } })
    await flushPromises()

    const [card, secondCard] = wrapper.findAll('.experiment-result-card')
    expect(card.text()).not.toContain('Trend engine')
    expect(card.get('.experiment-avatar').text()).toBe('🎯')
    expect(api).not.toHaveBeenCalledWith('/api/experiments/experiment-1')
    expect(card.get('.experiment-result-title-line h3').text()).toBe('Momentum study')
    expect(card.get('.experiment-result-title-line').text()).not.toContain('Success')
    const status = card.get('.experiment-result-status')
    expect(status.text()).toBe('Success')
    expect(status.find('svg').exists()).toBe(true)
    expect(status.get('strong').classes()).toContain('positive')
    expect(card.get('.experiment-result-meta > span').text()).toMatch(/\d{1,2}:\d{2}/)
    expect(card.get('.experiment-result-meta').text()).toContain('Sharpe 1.42')
    expect(card.get('.experiment-result-meta').text()).toContain('2 symbols')
    expect(card.get('.experiment-result-meta').text()).not.toContain('Best Sharpe')

    await card.get('.breakdown-toggle').trigger('click')
    expect(card.text()).toContain('Trend engine')
    expect(card.text()).toContain('1.42')
    expect(card.text()).toContain('1,250')
    expect(card.text()).toContain('12.00%')
    expect(card.text()).toContain('-8.00%')
    const metrics = card.findAll('.run-summary-metrics > div')
    expect(metrics).toHaveLength(7)
    expect(metrics.map(item => item.get('span').text())).toEqual([
      'Sharpe ratio',
      'Total return',
      'PNL',
      'CAGR',
      'Alpha',
      'Maximum DD',
      'Trades / win rate'
    ])
    expect(metrics.at(-1).get('strong').text()).toBe('14 / 57.00%')
    expect(api).not.toHaveBeenCalledWith('/api/experiments/experiment-1')

    await secondCard.get('.breakdown-toggle').trigger('click')
    expect(card.find('.experiment-breakdown').exists()).toBe(false)
    expect(secondCard.find('.experiment-breakdown').exists()).toBe(true)
  })

  it('shows one study with four focused result tabs', async () => {
    const studySummary = {
      ...summary,
      kind: 'study',
      study: {
        candidate_count: 4,
        fold_count: 2,
        objective: 'sharpe',
        best_candidate_id: 'candidate-002'
      },
      runs: [
        {
          ...summary.runs[0],
          strategy_id: 'benchmark',
          strategy_name: 'Benchmark',
          is_benchmark: true
        },
        {
          ...summary.runs[0],
          strategy_id: 'strategy-2',
          strategy_name: 'C002',
          parameters: { fast: 20, slow: 100, threshold: 0.02, quantity: 100 }
        },
        {
          ...summary.runs[0],
          strategy_id: 'strategy-1',
          strategy_name: 'C001',
          parameters: { fast: 10, slow: 100, threshold: 0.02, quantity: 50 }
        },
        {
          ...summary.runs[0],
          strategy_id: 'strategy-4',
          strategy_name: 'C004',
          parameters: { fast: 20, slow: 200, threshold: 0.04, quantity: 100 }
        }
      ]
    }
    const studyDetail = structuredClone(detail)
    studyDetail.runs = [{
      ...detail.runs[0],
      strategy_id: 'strategy-2',
      strategy_name: 'C002',
      parameters: { fast: 20, slow: 100, threshold: 0.02, quantity: 100 }
    }]
    studyDetail.study = {
      schema_version: 1,
      study_id: 'experiment-1',
      name: 'Momentum study',
      strategy_name: 'Custom crossover',
      objective: 'sharpe',
      maximize: true,
      parameter_space: { fast: [10, 20], slow: [100, 200] },
      candidates: [
        { candidate_id: 'candidate-001', strategy_name: 'C001', strategy_id: 'strategy-1', parameters: { fast: 10, slow: 100, threshold: 0.02, quantity: 50 }, metrics: { sharpe: 0.8 }, trade_count: 20, eligible: true, rank: 2, error: null },
        { candidate_id: 'candidate-002', strategy_name: 'C002', strategy_id: 'strategy-2', parameters: { fast: 20, slow: 100, threshold: 0.02, quantity: 100 }, metrics: { sharpe: 1.4 }, trade_count: 25, eligible: true, rank: 1, error: null },
        { candidate_id: 'candidate-003', strategy_name: 'C003', strategy_id: 'strategy-3', parameters: { fast: 10, slow: 200, threshold: 0.04, quantity: 50 }, metrics: { sharpe: 0.2 }, trade_count: 4, eligible: false, rank: null, error: null },
        { candidate_id: 'candidate-004', strategy_name: 'C004', strategy_id: 'strategy-4', parameters: { fast: 20, slow: 200, threshold: 0.04, quantity: 100 }, metrics: { sharpe: 0.6 }, trade_count: 18, eligible: true, rank: 3, error: null }
      ],
      folds: [
        { fold: 1, training_start: '2020-01-01', training_end: '2021-12-31', test_start: '2022-01-01', test_end: '2022-12-31', candidate_id: 'candidate-002', parameters: { fast: 20, slow: 100, threshold: 0.02, quantity: 100 }, training_objective: 1.4, test_objective: 0.7, test_metrics: { sharpe: 0.7 }, trade_count: 12, error: null },
        { fold: 2, training_start: '2021-01-01', training_end: '2022-12-31', test_start: '2023-01-01', test_end: '2023-12-31', candidate_id: 'candidate-001', parameters: { fast: 10, slow: 100, threshold: 0.02, quantity: 50 }, training_objective: 1.1, test_objective: -0.1, test_metrics: { sharpe: -0.1 }, trade_count: 10, error: null }
      ],
      best_candidate_id: 'candidate-002',
      min_trades: 10,
      max_drawdown: 0.25,
      warnings: []
    }
    query.mockResolvedValue([studySummary])
    api.mockImplementation(path => path === '/api/jobs' ? [] : studyDetail)
    post.mockImplementation(path => {
      if (path === '/api/studies/reuse') {
        return { general: { name: 'Momentum study · C002' }, strategy: { strategies: ['Custom crossover · C002'] } }
      }
      if (path === '/api/studies/rerun') {
        return { general: { name: 'Momentum study' }, _study: { parameter_space: { fast: [10, 20] } } }
      }
      return { data: [], layout: {} }
    })

    wrapper = mount(ResultsPage, { props: { bootstrap: metricBootstrap } })
    await flushPromises()
    const card = wrapper.get('.experiment-result-card')
    expect(card.text()).toContain('Study')
    expect(card.get('.study-badge').exists()).toBe(true)
    expect(card.text()).toContain('4 candidates')
    expect(card.text()).not.toContain('top 3 in breakdown')
    await card.get('.breakdown-toggle').trigger('click')
    expect(card.findAll('.run-breakdown-card > header strong').map(item => item.text())).toEqual([
      'Benchmark', 'C2', 'C1', 'C4'
    ])
    expect(card.get('.candidate-parameters-info').attributes('aria-label')).toContain('fast=20 · slow=100')
    await card.get('.experiment-card-actions .secondary').trigger('click')
    await flushPromises()
    await flushPromises()

    const tabs = wrapper.findAll('.study-result-tabs button')
    expect(tabs.map(item => item.text())).toEqual(['Sweep', 'Candidates', 'Walk-forward', 'Report'])
    expect(wrapper.get('.study-sweep-heading h3').text()).toBe('All 4 candidate results')
    expect(wrapper.get('.study-sweep-heading p').text()).toBe(
      'Candidates ranked by Sharpe ratio; only eligible candidates receive a rank.'
    )
    const sweepSummary = wrapper.findAll('.study-summary-grid article')
    expect(sweepSummary).toHaveLength(3)
    expect(sweepSummary.map(item => item.get('span').text())).toEqual([
      'Best Sharpe ratio', 'Eligible / total candidates', 'Parameters swept'
    ])
    expect(sweepSummary.map(item => item.get('strong').text())).toEqual(['1.4', '3 / 4', '2'])
    expect(sweepSummary.every(item => !item.find('small').exists())).toBe(true)
    expect(wrapper.find('.study-heatmap').exists()).toBe(true)
    expect(wrapper.get('.study-heatmap-scroll').attributes()).toMatchObject({
      role: 'region',
      tabindex: '0',
      'aria-label': 'Sharpe ratio by parameter combination'
    })
    expect(wrapper.get('.study-heatmap-heading').text()).toContain('Sharpe ratio heatmap')
    expect(wrapper.get('.heatmap-scale').text()).toBe('LowHigh')
    const heatmapStyles = wrapper.findAll('.heatmap-cell').map(cell => cell.attributes('style'))
    expect(heatmapStyles.every(style => style.includes('--heatmap-score'))).toBe(true)
    expect(new Set(heatmapStyles).size).toBe(4)
    expect(wrapper.text()).toContain('Reuse best setup')
    expect(wrapper.text()).toContain('Rerun study')
    expect(wrapper.text()).not.toContain('Live session')
    const candidateHeading = wrapper.findAll('.result-section-heading')
      .find(item => item.text().includes('Top candidate runs'))
    expect(candidateHeading.classes()).toContain('study-section-heading')
    expect(wrapper.get('.strategy-switcher button').text()).toContain('C2')
    expect(wrapper.get('.strategy-switcher .candidate-parameters-info').attributes('aria-label')).toContain('fast=20 · slow=100')

    await tabs[1].trigger('click')
    expect(wrapper.get('.study-tab-panel').text()).toContain('C2')
    expect(wrapper.get('.study-tab-panel').text()).toContain('fast=20')
    const parameterSummary = wrapper.get('.study-tab-panel .parameter-summary')
    expect(parameterSummary.findAll('.parameter-value').map(item => item.text()))
      .toEqual(['fast=20', 'slow=100'])
    expect(parameterSummary.get('.parameter-overflow').text()).toBe('+2')
    await parameterSummary.get('.parameter-overflow').trigger('mouseenter')
    expect(document.body.querySelector('.field-info-popover')?.textContent.trim())
      .toBe(`threshold=${(0.02).toLocaleString()} · quantity=100`)
    await parameterSummary.get('.parameter-overflow').trigger('mouseleave')
    expect(wrapper.get('.eligibility-heading .field-info').attributes('aria-label')).toContain('minimum-trade requirement')
    await tabs[2].trigger('click')
    expect(wrapper.get('.study-tab-panel').text()).toContain('2022-01-01')
    expect(wrapper.get('.study-tab-panel').text()).toContain('C2')
    await tabs[3].trigger('click')
    expect(wrapper.get('.study-tab-panel').text()).toContain('1 of 2 favorable folds')
    expect(wrapper.get('.best-candidate-card .eyebrow').text()).toBe('Best candidate')
    expect(wrapper.get('.best-candidate-card h3').text()).toBe('C2')

    const resultActions = wrapper.findAll('.result-actions .secondary')
    const rerunButton = resultActions.find(button => button.text().includes('Rerun study'))
    const reuseBestButton = resultActions.find(button => button.text().includes('Reuse best setup'))
    await rerunButton.trigger('click')
    await flushPromises()
    expect(post).toHaveBeenCalledWith('/api/studies/rerun', { study_id: 'experiment-1' })
    expect(JSON.parse(sessionStorage.getItem('backtide:experiment-config'))).toEqual({
      general: { name: 'Momentum study' },
      _study: { parameter_space: { fast: [10, 20] } }
    })

    await reuseBestButton.trigger('click')
    await flushPromises()
    expect(post).toHaveBeenCalledWith('/api/studies/reuse', { study_id: 'experiment-1' })
    expect(JSON.parse(sessionStorage.getItem('backtide:experiment-config'))).toEqual({
      general: { name: 'Momentum study · C002' },
      strategy: { strategies: ['Custom crossover · C002'] }
    })
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
    await mountAndOpen(metricBootstrap)

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
    const durationMetric = overviewMetrics.find(item => item.get('span').text() === 'Duration')
    expect(periodMetric.get('svg').classes()).toContain('lucide-calendar-range-icon')
    expect(startedAtMetric.get('svg').classes()).toContain('lucide-calendar-days-icon')
    expect(durationMetric.get('strong').text()).toBe('1m 30s')
    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'pnl' }))
    expect(post).not.toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'mae_mfe' }))

    const strategyTabs = wrapper.findAll('.strategy-plot-tabs button').map(button => button.text())
    expect(strategyTabs).toEqual(['Metrics', 'MAE / MFE', 'Position size', 'Trades on price', 'Orders'])
    expect(wrapper.findAll('.result-workspace')[1].get('.result-table').exists()).toBe(true)
    expect(wrapper.get('.result-metrics').text()).toContain('PNL')
    expect(wrapper.get('.result-metrics').text()).toContain('1,250.00')
    expect(wrapper.findAll('.result-metrics .metric-card small')).toHaveLength(0)
    expect(wrapper.findAll('.result-metrics .metric-card span').map(item => item.text())).toEqual([
      'Sharpe ratio', 'Maximum DD', 'Total return', 'PNL'
    ])
    expect(wrapper.findAll('.result-table tbody tr').map(row => row.findAll('td')[0].text())).toEqual([
      'Sharpe ratio', 'Maximum DD', 'Total return', 'PNL'
    ])

    await wrapper.get('.results-back').trigger('click')
    expect(wrapper.find('.result-detail-page').exists()).toBe(false)
    expect(wrapper.get('.experiment-result-card').exists()).toBe(true)
  })

  it('uses configured symbols before trade history is loaded', async () => {
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      runs: [{ ...detail.runs[0], trades: [] }]
    })
    await mountAndOpen(metricBootstrap)

    const priceTab = wrapper.findAll('.strategy-plot-tabs button')
      .find(button => button.text() === 'Trades on price')
    await priceTab.trigger('click')
    await flushPromises()

    expect(wrapper.findAll('.result-plot-options option').map(option => option.text())).toEqual([
      'AAPL', 'MSFT'
    ])
    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({
      plot: 'price',
      options: expect.objectContaining({ symbol: 'AAPL' })
    }))
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

  it('uses one deterministic metric order for every strategy in an older experiment', async () => {
    query.mockResolvedValue([{
      ...summary,
      selected_metrics: [],
      runs: [
        {
          ...summary.runs[0],
          strategy_id: 'strategy-a',
          metrics: { alpha: 0.03, pnl: 1250, total_return: 0.12, sharpe_ratio: 1.42, max_drawdown: -0.08 }
        },
        {
          ...summary.runs[0],
          strategy_id: 'strategy-b',
          strategy_name: 'Second engine',
          metrics: { max_drawdown: -0.12, sharpe_ratio: 0.8, total_return: 0.07, pnl: 700, alpha: 0.01 }
        }
      ]
    }])
    wrapper = mount(ResultsPage, { props: { bootstrap: metricBootstrap } })
    await flushPromises()

    await wrapper.get('.breakdown-toggle').trigger('click')

    const orders = wrapper.findAll('.run-summary-metrics').map(row =>
      row.findAll(':scope > div span').map(label => label.text())
    )
    expect(orders).toEqual([
      ['Sharpe ratio', 'Total return', 'PNL', 'Maximum DD', 'Alpha'],
      ['Sharpe ratio', 'Total return', 'PNL', 'Maximum DD', 'Alpha']
    ])
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
      'Symbol', 'Datetime', 'Type', 'Side', 'Qty', 'Price', 'PNL', 'Commission', 'Status'
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

  it('opens a newly completed experiment requested by its background job', async () => {
    sessionStorage.setItem('backtide:result-job-id', 'job-1')
    api.mockImplementation(path => path === '/api/jobs'
      ? [{
          id: 'job-1', kind: 'experiment', status: 'success',
          result: { experiment_id: 'experiment-2' }
        }]
      : detail)

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/experiments/experiment-2')
    expect(wrapper.find('.result-detail-page').exists()).toBe(true)
    expect(sessionStorage.getItem('backtide:result-job-id')).toBeNull()
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

    const summaryBlock = wrapper.get('.result-heading-copy')
    const titleLine = summaryBlock.get('.result-heading-title-line')
    expect(titleLine.element.children[0].classList).toContain('result-heading-icon')
    expect(titleLine.element.children[1].tagName).toBe('H2')
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

  it('downloads the complete saved configuration from its modal', async () => {
    await mountAndOpen()

    const configButton = wrapper.findAll('.result-actions .secondary')
      .find(button => button.text().includes('Config'))
    await configButton.trigger('click')

    const download = wrapper.get('.document-modal-actions a')
    expect(download.text()).toContain('Download config')
    expect(download.attributes('download')).toBe('Momentum-study.toml')
    expect(download.attributes('href')).toBe(
      `data:application/toml;charset=utf-8,${encodeURIComponent(detail.config)}`
    )
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
    expect(tabs[0].text()).toBe('PNL')
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
      if (path.endsWith('/session-config')) return draft
      return detail
    })
    await mountAndOpen()

    const liveButton = wrapper.findAll('.result-actions .secondary')
      .find(button => button.text().includes('Live session'))
    await liveButton.trigger('click')
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/experiments/experiment-1/session-config')
    expect(JSON.parse(sessionStorage.getItem('backtide:session-config'))).toEqual(draft)
    expect(wrapper.emitted('navigate')[0]).toEqual(['live'])
  })
})
