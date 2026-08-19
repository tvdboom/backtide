// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, markRaw, shallowRef } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import IntervalPicker from '../components/interval-picker.vue'
import SearchSelect from '../components/search-select.vue'
import ExperimentPage from './experiment-page.vue'

const { post, query } = vi.hoisted(() => ({ post: vi.fn(), query: vi.fn() }))

vi.mock('../api', () => ({ post, query }))

const bootstrap = {
  defaults: {
    general: { name: '', icon: '', tags: [], description: '' },
    data: {
      instrument_type: 'stocks', symbols: [], interval: 'OneDay', full_history: true,
      start_date: null, end_date: null
    },
    portfolio: { initial_cash: 10000, base_currency: 'USD', starting_positions: {} },
    strategy: { strategies: [], benchmark: null },
    indicators: { indicators: [] },
    metrics: { metrics: ['total_return', 'sharpe'], main_metric: 'sharpe' },
    exchange: {
      commission_type: 'Percentage', commission_pct: 0.1, commission_fixed: 0,
      slippage: 0.05, partial_fills: false, allowed_order_types: ['Market'],
      allow_margin: false, max_leverage: 2, initial_margin: 50, maintenance_margin: 25,
      margin_interest: 0, raise_on_margin_limit: false, allow_short_selling: false,
      borrow_rate: 0, raise_on_short_violation: false, max_position_size: 100,
      conversion_mode: 'Immediate', conversion_threshold: 0, conversion_period: null,
      conversion_interval: null
    },
    engine: {
      warmup_period: 0, risk_free_rate: 0, trade_on_close: false,
      exclusive_orders: false, empty_bar_policy: 'ForwardFill'
    }
  },
  enums: {
    instrument_types: ['Stocks', 'ETF', 'Forex', 'Crypto'],
    intervals: ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w'],
    commission_types: ['Percentage (%)', 'Fixed amount', 'Percentage + Fixed'],
    order_types: ['Market', 'Limit'],
    conversion_modes: ['Immediate', 'HoldUntilThreshold', 'EndOfPeriod', 'CustomInterval'],
    conversion_periods: ['day', 'week', 'month', 'year'],
    empty_bar_policies: ['Skip', 'ForwardFill', 'FillWithNaN'],
    currencies: [
      { code: 'EUR', name: 'Euro', flag: '🇪🇺', country_code: 'eu' },
      { code: 'USD', name: 'United States Dollar', flag: '🇺🇸', country_code: 'us' }
    ]
  },
  display: { logokit_api_key: '' },
  strategies: { saved: [] },
  indicators: { saved: [] },
  metrics: {
    builtin: [
      { key: 'total_return', name: 'Total return', description: 'Net return.', builtin: true, percentage: true },
      { key: 'sharpe', name: 'Sharpe ratio', description: 'Risk adjusted.', builtin: true, percentage: false }
    ],
    saved: []
  }
}

describe('experiment page', () => {
  beforeEach(() => {
    sessionStorage.clear()
    query.mockReset().mockResolvedValue([])
    post.mockReset()
  })

  it('uses neutral prompts for the experiment name and description', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[0].trigger('click')

    expect(wrapper.get('#experiment-name').attributes('placeholder')).toBe('Enter a name...')
    expect(wrapper.get('textarea').attributes('placeholder')).toBe('Add a description...')
    expect(wrapper.get('#experiment-icon').findAll('option').length).toBeGreaterThan(5)
  })

  it('explains every experiment setting', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    for (const tab of wrapper.findAll('.tabs button')) {
      await tab.trigger('click')
      const settings = wrapper.findAll(
        '.experiment-builder label, .experiment-builder .field-label, '
        + '.experiment-builder .position-field, .experiment-builder .field-control-label'
      )
      expect(settings.length).toBeGreaterThan(0)
      expect(settings.every(setting => setting.find('.field-info').exists())).toBe(true)
      expect(wrapper.findAll('.toggle-label').every(toggle =>
        toggle.get('.toggle-description').text().length > 0
      )).toBe(true)
    }
  })

  it('labels selected metrics as built-in or custom without implementation details', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.defaults.metrics.metrics.push('MyMetric')
    pageBootstrap.metrics.saved.push({
      key: 'MyMetric', name: 'My metric', description: 'A custom result.', builtin: false
    })
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[4].trigger('click')

    const details = wrapper.get('[aria-label="Selected metric details"]')
    expect(details.text()).toContain('Built-in')
    expect(details.text()).toContain('Custom')
    expect(details.text()).not.toContain('Rust built-in')
    expect(details.text()).not.toContain('Custom Python')
  })

  it('orders default metrics by importance and lets users reorder non-main metrics', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.defaults.metrics = {
      metrics: ['final_equity', 'win_rate', 'pnl', 'total_return', 'sharpe', 'max_dd'],
      main_metric: 'sharpe'
    }
    pageBootstrap.metrics.builtin.push(
      { key: 'final_equity', name: 'Final equity', description: 'Ending value.', builtin: true },
      { key: 'win_rate', name: 'Win rate', description: 'Winning trades.', builtin: true, percentage: true },
      { key: 'pnl', name: 'Profit and loss', description: 'Net profit.', builtin: true },
      { key: 'max_dd', name: 'Maximum drawdown', description: 'Peak decline.', builtin: true, percentage: true }
    )
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[4].trigger('click')

    const selectedKeys = () => wrapper.getComponent(SearchSelect).props('modelValue')
    expect(selectedKeys()).toEqual([
      'sharpe', 'total_return', 'pnl', 'max_dd', 'win_rate', 'final_equity'
    ])
    expect(wrapper.get('[aria-label="Move Sharpe ratio down"]').attributes('disabled')).toBeDefined()

    await wrapper.get('[aria-label="Move Profit and loss up"]').trigger('click')
    expect(selectedKeys()).toEqual([
      'sharpe', 'pnl', 'total_return', 'max_dd', 'win_rate', 'final_equity'
    ])

    await wrapper.get('#experiment-main-metric').setValue('max_dd')
    await flushPromises()
    expect(selectedKeys()).toEqual([
      'max_dd', 'sharpe', 'pnl', 'total_return', 'win_rate', 'final_equity'
    ])

    let metricCards = wrapper.findAll('.metric-selection-card')
    expect(metricCards[0].attributes('draggable')).toBe('false')
    expect(metricCards[1].attributes('draggable')).toBe('true')
    const dataTransfer = {
      effectAllowed: '',
      getData: vi.fn(() => 'final_equity'),
      setData: vi.fn()
    }
    await metricCards[5].trigger('dragstart', { dataTransfer })
    metricCards = wrapper.findAll('.metric-selection-card')
    await metricCards[2].trigger('dragover', { dataTransfer })
    expect(metricCards[2].classes()).toContain('drop-target')
    await metricCards[2].trigger('drop', { dataTransfer })
    expect(dataTransfer.setData).toHaveBeenCalledWith('text/plain', 'final_equity')
    expect(selectedKeys()).toEqual([
      'max_dd', 'sharpe', 'final_equity', 'pnl', 'total_return', 'win_rate'
    ])

    const clearMetrics = wrapper.get('[aria-label="Clear all metrics"]')
    await clearMetrics.trigger('click')
    expect(selectedKeys()).toEqual([])
    expect(wrapper.get('#experiment-main-metric').element.value).toBe('')
    expect(clearMetrics.attributes('disabled')).toBeDefined()
  })

  it('shows serialized defaults with friendly labels and loads the legacy catalog size', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    expect(query).toHaveBeenCalledWith('/api/instruments', {
      instrument_type: 'stocks',
      source: 'catalog',
      limit: 1500
    })

    await wrapper.findAll('.tabs button')[1].trigger('click')
    expect(wrapper.get('.segmented button').classes()).toContain('active')
    expect(wrapper.findAll('.segmented button svg')).toHaveLength(4)
    const interval = wrapper.getComponent(IntervalPicker)
    expect(interval.props('modelValue')).toBe('OneDay')
    expect(interval.get('[aria-checked="true"]').text()).toBe('1d')
    expect(interval.element.parentElement.classList).toContain('interval-picker-field')

    await interval.findAll('button')[1].trigger('click')
    expect(interval.props('modelValue')).toBe('FiveMinutes')
    expect(interval.findAll('[aria-checked="true"]')).toHaveLength(1)

    await wrapper.findAll('.tabs button')[2].trigger('click')
    expect(wrapper.get('.currency-picker-field').exists()).toBe(true)

    await wrapper.findAll('.tabs button')[5].trigger('click')
    expect(wrapper.find('select').exists()).toBe(false)
    expect(wrapper.get('#experiment-commission-pct').element.value).toBe('0.1')
    expect(wrapper.get('#experiment-commission-fixed').element.value).toBe('0')

    await wrapper.findAll('.tabs button')[7].trigger('click')
    expect(wrapper.get('select').element.value).toBe('ForwardFill')
    expect(wrapper.get('select').find('option:checked').text()).toBe('Forward Fill')
  })

  it('keeps the run action available on every experiment tab', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    for (const tabButton of wrapper.findAll('.tabs button')) {
      await tabButton.trigger('click')
      expect(wrapper.get('button[type="submit"]').text()).toContain('Run experiment')
    }
  })

  it('clears the symbol search when the instrument type changes', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.get('#experiment-symbols').setValue('AAPL')

    await wrapper.findAll('.segmented button')[3].trigger('click')
    await flushPromises()

    expect(wrapper.get('#experiment-symbols').element.value).toBe('')
    expect(query).toHaveBeenLastCalledWith('/api/instruments', {
      instrument_type: 'crypto',
      source: 'catalog',
      limit: 1500
    })
  })

  it('loads a logo for a selected custom symbol outside the current catalog', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.display.logokit_api_key = 'test token'
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[1].trigger('click')

    const input = wrapper.get('#experiment-symbols')
    await input.setValue('AVIANRO')
    await input.trigger('keydown.enter')

    expect(wrapper.get('.tag').text()).toContain('AVIANRO')
    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toBe(
      'https://img.logokit.com/ticker/AVIANRO?token=test%20token'
    )
    expect(wrapper.find('.logo-attribution').exists()).toBe(false)
  })

  it('does not remove a selected symbol when its field label is clicked', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' }
    ])
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.get('#experiment-symbols').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')

    await wrapper.get('.symbol-select-field > span').trigger('click')

    expect(wrapper.findAll('.symbol-select-field .tag')).toHaveLength(1)
    expect(wrapper.get('.symbol-select-field .tag').text()).toContain('AAPL')
  })

  it('applies a reused setup when the cached experiment builder is reactivated', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' }
    ])
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [{
      name: 'Momentum', type: 'Macd', builtin: true, description: 'Follow trends.',
      required_indicators: []
    }]
    const away = markRaw({ template: '<div class="away" />' })
    const current = shallowRef(markRaw(ExperimentPage))
    const Host = defineComponent({
      setup: () => ({ current, pageBootstrap }),
      template: '<KeepAlive><component :is="current" :bootstrap="pageBootstrap" /></KeepAlive>'
    })
    const wrapper = mount(Host)
    await flushPromises()

    current.value = away
    await wrapper.vm.$nextTick()
    const reused = structuredClone(pageBootstrap.defaults)
    reused.general.name = 'Reused momentum setup'
    reused.data.symbols = ['AAPL']
    reused.portfolio.initial_cash = 25000
    reused.strategy.strategies = ['Momentum']
    sessionStorage.setItem('backtide:experiment-config', JSON.stringify(reused))

    current.value = markRaw(ExperimentPage)
    await wrapper.vm.$nextTick()
    await flushPromises()

    expect(wrapper.get('#experiment-name').element.value).toBe('Reused momentum setup')
    await wrapper.findAll('.tabs button')[1].trigger('click')
    expect(wrapper.get('.symbol-select-field .tag').text()).toContain('AAPL')
    await wrapper.findAll('.tabs button')[2].trigger('click')
    expect(wrapper.get('#experiment-initial-cash').element.value).toBe('25000')
    await wrapper.findAll('.tabs button')[3].trigger('click')
    expect(wrapper.get('#experiment-strategies').element.closest('.search-select').textContent)
      .toContain('Momentum')
  })

  it('uses a valid initial cash default', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[2].trigger('click')
    const input = wrapper.get('#experiment-initial-cash').element

    expect(input.value).toBe('10000')
    expect(input.step).toBe('100')
    expect(input.validity.valid).toBe(true)
    input.stepUp()
    expect(input.value).toBe('10100')
    input.stepDown()
    expect(input.value).toBe('10000')
  })

  it('uses the configured base currency in a new experiment', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.defaults.portfolio.base_currency = 'EUR'
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[2].trigger('click')

    const trigger = wrapper.get('#experiment-base-currency')
    expect(trigger.text()).toContain('EUR')
    expect(trigger.get('.currency-flag').attributes('src')).toBe(
      'https://flagcdn.com/eu.svg'
    )
  })

  it('uses the compact currency dropdown backed by currency metadata', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[2].trigger('click')

    const trigger = wrapper.get('#experiment-base-currency')
    expect(trigger.get('.currency-flag').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    expect(trigger.text()).toContain('USD')

    await trigger.trigger('click')
    const options = wrapper.findAll('.currency-menu [role="option"]')
    expect(options.map(option => option.get('strong').text())).toEqual(['EUR', 'USD'])
    expect(options.map(option => option.get('small').text())).toEqual([
      'Euro',
      'United States Dollar'
    ])
    expect(options.map(option => option.get('.currency-flag').attributes('src'))).toEqual([
      'https://flagcdn.com/eu.svg',
      'https://flagcdn.com/us.svg'
    ])

    await wrapper.get('[aria-label="Search base currencies"]').setValue('euro')
    expect(wrapper.findAll('.currency-menu [role="option"]')).toHaveLength(1)

    await wrapper.get('.currency-menu [role="option"]').trigger('click')
    expect(wrapper.get('#experiment-base-currency').text()).toContain('EUR')
  })

  it('builds starting positions from the selected market symbols', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' },
      { symbol: 'MSFT', name: 'Microsoft Corporation', instrument_type: 'stocks' }
    ])
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.get('#experiment-symbols').trigger('focus')
    await wrapper.findAll('.search-menu button')[0].trigger('click')

    await wrapper.findAll('.tabs button')[2].trigger('click')
    expect(wrapper.get('.portfolio-basics').exists()).toBe(true)
    expect(wrapper.text()).toContain('begin entirely in cash')
    await wrapper.get('.starting-positions-heading button').trigger('click')

    expect(wrapper.find('.position-row .tag').exists()).toBe(false)
    expect(wrapper.get('.instrument-select-trigger').text()).toContain('AAPL')
    expect(wrapper.get('.instrument-select-trigger').text()).not.toContain('Apple Inc.')
    await wrapper.get('.instrument-select-trigger').trigger('click')
    expect(wrapper.get('.instrument-select-menu').text()).toContain('Apple Inc.')
    expect(wrapper.find('textarea').exists()).toBe(false)
  })

  it('does not submit the experiment when Enter is pressed in the name', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[0].trigger('click')

    const name = wrapper.get('#experiment-name')
    await name.setValue('Keyboard-safe study')
    await name.trigger('keydown.enter')
    await flushPromises()

    expect(wrapper.get('.tabs button.active').text()).toContain('General')
    expect(wrapper.find('.form-alert').exists()).toBe(false)
    expect(wrapper.emitted('toast')).toBeUndefined()
  })

  it('uses icons for strategies and a searchable single benchmark selector', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' }
    ])
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [{
      name: 'Momentum',
      type: 'Macd',
      description: 'Follows persistent price trends.',
      builtin: true,
      required_indicators: [{ name: 'SMA 20', description: 'A moving price average.' }]
    }]
    pageBootstrap.indicators.saved = [{
      name: 'RSI', type: 'CustomRsi', description: 'Measures relative price strength.', builtin: false
    }]
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.get('#experiment-symbols').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')
    await wrapper.findAll('.tabs button')[3].trigger('click')

    await wrapper.get('#experiment-strategies').trigger('focus')
    const strategyOption = wrapper.get('.search-menu button')
    expect(strategyOption.get('.search-option-logo svg').exists()).toBe(true)
    expect(strategyOption.get('strong').text()).toBe('Momentum')
    expect(strategyOption.get('small').text()).toBe('MACD')
    await strategyOption.trigger('click')
    const strategyInsight = wrapper.get('[aria-label="Selected strategy details"]')
    expect(strategyInsight.text()).toContain('Follows persistent price trends.')
    expect(strategyInsight.text()).toContain('SMA 20')
    expect(strategyInsight.text()).toContain('Injected indicators')
    expect(strategyInsight.text()).not.toContain('Built-in')
    expect(strategyInsight.get('.metric-icon svg').exists()).toBe(true)

    await wrapper.get('input[aria-label="Experiment indicators"]').trigger('focus')
    const indicatorOption = wrapper.get('.search-menu button')
    expect(indicatorOption.get('strong').text()).toBe('RSI')
    expect(indicatorOption.get('small').text()).toBe('Custom')
    await indicatorOption.trigger('click')
    const indicatorInsight = wrapper.get('[aria-label="Selected indicator details"]')
    expect(indicatorInsight.text()).toContain('Measures relative price strength.')
    expect(indicatorInsight.get('.metric-icon svg').exists()).toBe(true)

    const benchmarkInput = wrapper.get('input[aria-label="Experiment benchmark"]')
    const benchmarkControl = benchmarkInput.element.closest('.benchmark-select')
    const strategyInput = wrapper.get('#experiment-strategies')
    const benchmarkPosition = benchmarkInput.element.compareDocumentPosition(strategyInput.element)
    expect(benchmarkPosition & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(benchmarkInput.element.value).toBe('SPY')
    expect(wrapper.text()).toContain('Compare performance against a passive benchmark')
    await benchmarkInput.trigger('focus')
    const benchmarkMenu = wrapper.findAll('.search-menu').at(-1)
    expect(benchmarkMenu.text()).toContain('AAPL')
    await benchmarkMenu.get('button').trigger('click')
    await wrapper.get('.section-copy h3').trigger('click')
    expect(benchmarkInput.element.value).toBe('AAPL')
    expect(benchmarkControl.querySelector('.tag')).toBeNull()

    await benchmarkInput.trigger('focus')
    await benchmarkInput.setValue('SPY')
    await benchmarkInput.trigger('keydown.enter')
    await wrapper.vm.$nextTick()
    expect(benchmarkInput.element.value).toBe('SPY')
    expect(benchmarkControl.querySelector('.tag')).toBeNull()
    expect(benchmarkControl.querySelector('.search-menu')).toBeNull()
    expect(wrapper.text()).not.toContain('managed in their dedicated library pages')
  })

  it('updates the automatic benchmark for currency and asset-class changes', async () => {
    query.mockResolvedValue([
      { symbol: 'BTC-EUR', name: 'Bitcoin / Euro', instrument_type: 'crypto' }
    ])
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[2].trigger('click')
    await wrapper.get('#experiment-base-currency').trigger('click')
    await wrapper.get('[aria-label="Search base currencies"]').setValue('euro')
    await wrapper.get('.currency-menu [role="option"]').trigger('click')

    await wrapper.findAll('.tabs button')[3].trigger('click')
    expect(wrapper.get('input[aria-label="Experiment benchmark"]').element.value).toBe('EXW1.DE')

    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.findAll('.segmented button')[3].trigger('click')
    await flushPromises()
    await wrapper.findAll('.tabs button')[3].trigger('click')

    const benchmarkInput = wrapper.get('input[aria-label="Experiment benchmark"]')
    expect(benchmarkInput.element.value).toBe('BTC-EUR')
    await benchmarkInput.trigger('focus')
    await benchmarkInput.setValue('ETH-EUR')
    await benchmarkInput.trigger('keydown.enter')
    expect(benchmarkInput.element.value).toBe('ETH-EUR')

    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.findAll('.segmented button')[2].trigger('click')
    await flushPromises()
    await wrapper.findAll('.tabs button')[3].trigger('click')
    expect(wrapper.get('input[aria-label="Experiment benchmark"]').element.value).toBe('')
  })

  it('omits injected-indicator chrome when a strategy has no injected indicators', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [{
      name: 'Buy and hold', type: 'BuyAndHold', description: 'Hold one asset.',
      builtin: true, required_indicators: []
    }]
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[3].trigger('click')
    await wrapper.get('#experiment-strategies').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')

    expect(wrapper.find('.required-indicators').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('does not require')
    expect(wrapper.text()).not.toContain('Built-in')
  })

  it('describes order types without decorative initials or logos', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[5].trigger('click')
    expect(wrapper.get('.tag').text()).toContain('Market')
    expect(wrapper.get('.tag').text()).not.toContain('best available market price')
    expect(wrapper.find('.tag-copy').exists()).toBe(false)
    await wrapper.get('#experiment-order-types').trigger('focus')

    const option = wrapper.get('.search-menu button')
    expect(option.text()).toContain('Limit')
    expect(option.text()).toContain('chosen price or better')
    expect(option.find('.search-option-logo').exists()).toBe(false)
  })

  it('always shows percentage and fixed commission inputs without a model dropdown', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[5].trigger('click')

    expect(wrapper.text()).toContain('Commission (%)')
    expect(wrapper.text()).toContain('Fixed commission')
    expect(wrapper.text()).toContain('Slippage (%)')
    expect(wrapper.find('#experiment-commission-type').exists()).toBe(false)
    expect(wrapper.get('#experiment-commission-pct').exists()).toBe(true)
    expect(wrapper.get('#experiment-commission-fixed').exists()).toBe(true)
    expect(wrapper.get('#experiment-slippage').exists()).toBe(true)
  })

  it('resets a queued experiment while preserving its selected asset class', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'etf' }
    ])
    post.mockResolvedValue({ id: 'experiment-1' })
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [{
      name: 'Momentum', type: 'Macd', builtin: true, description: 'Follow trends.',
      required_indicators: []
    }]
    const wrapper = mount(ExperimentPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()

    await wrapper.get('#experiment-name').setValue('Persistent draft')
    await wrapper.findAll('.tabs button')[1].trigger('click')
    await wrapper.get('.wide-control').findAll('button')[1].trigger('click')
    await flushPromises()
    await wrapper.get('#experiment-symbols').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')
    await wrapper.findAll('.tabs button')[3].trigger('click')
    await wrapper.get('#experiment-strategies').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')

    await wrapper.get('button[type="submit"]').trigger('submit')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/experiments', expect.objectContaining({
      general: expect.objectContaining({ name: 'Persistent draft' }),
      data: expect.objectContaining({ instrument_type: 'etf', symbols: ['AAPL'] }),
      metrics: { metrics: ['sharpe', 'total_return'], main_metric: 'sharpe' },
      exchange: expect.objectContaining({ commission_type: 'PercentagePlusFixed' })
    }))
    expect(wrapper.emitted('navigate')).toEqual([['results']])
    expect(sessionStorage.getItem('backtide:results-overview')).toBe('true')
    expect(wrapper.get('.tabs button.active').text()).toContain('General')
    expect(wrapper.get('#experiment-name').element.value).toBe('')

    await wrapper.findAll('.tabs button')[1].trigger('click')
    expect(wrapper.get('.wide-control button.active').text()).toContain('ETF')
    expect(wrapper.find('.tag').exists()).toBe(false)
  })

  it('opens the failing tab, shows the error, and focuses its widget', async () => {
    const wrapper = mount(ExperimentPage, { attachTo: document.body, props: { bootstrap } })
    await flushPromises()
    await wrapper.get('#experiment-name').setValue('Validation study')
    await wrapper.findAll('.tabs button')[7].trigger('click')

    await wrapper.get('button[type="submit"]').trigger('submit')
    await flushPromises()

    expect(wrapper.get('.tabs button.active').text()).toContain('Market data')
    expect(wrapper.get('.form-alert').text()).toContain('Select at least one market symbol.')
    expect(document.activeElement?.id).toBe('experiment-symbols')
    expect(wrapper.emitted('toast')).toBeUndefined()
    wrapper.unmount()
  })

  it('clears a validation warning as soon as the issue is corrected', async () => {
    query.mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' }
    ])
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[0].trigger('click')
    await wrapper.get('#experiment-name').setValue('Correctable study')
    await wrapper.get('button[type="submit"]').trigger('submit')
    await flushPromises()

    expect(wrapper.get('.form-alert').text()).toContain('Select at least one market symbol.')
    await wrapper.get('#experiment-symbols').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('.form-alert').exists()).toBe(false)
    expect(wrapper.emitted('toast')).toBeUndefined()
  })

  it('automatically dismisses an inline validation warning', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(ExperimentPage, { props: { bootstrap } })
      await wrapper.get('button[type="submit"]').trigger('submit')
      expect(wrapper.find('.form-alert').exists()).toBe(true)

      await vi.advanceTimersByTimeAsync(4500)
      expect(wrapper.find('.form-alert').exists()).toBe(false)
      wrapper.unmount()
    } finally {
      vi.useRealTimers()
    }
  })

  it('surfaces catalog failures on the market-data tab', async () => {
    query.mockRejectedValueOnce(new Error('Provider unavailable.'))

    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.tabs button.active').text()).toContain('Market data')
    expect(wrapper.get('.form-alert').text()).toContain(
      'Could not load the symbol catalog. Provider unavailable.'
    )
    expect(wrapper.emitted('toast')).toBeUndefined()
  })
})
