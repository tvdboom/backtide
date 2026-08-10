// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ExperimentPage from './experiment-page.vue'

const { post, query } = vi.hoisted(() => ({ post: vi.fn(), query: vi.fn() }))

vi.mock('../api', () => ({ post, query }))

const bootstrap = {
  defaults: {
    general: { name: '', tags: [], description: '' },
    data: {
      instrument_type: 'stocks', symbols: [], interval: 'OneDay', full_history: true,
      start_date: null, end_date: null
    },
    portfolio: { initial_cash: 10000, base_currency: 'USD', starting_positions: {} },
    strategy: { strategies: [], benchmark: null },
    indicators: { indicators: [] },
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
  indicators: { saved: [] }
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
    expect(wrapper.get('select').element.value).toBe('OneDay')
    expect(wrapper.get('select').find('option:checked').text()).toBe('1d')

    await wrapper.findAll('.tabs button')[4].trigger('click')
    expect(wrapper.get('select').element.value).toBe('Percentage')
    expect(wrapper.get('select').find('option:checked').text()).toBe('Percentage (%)')

    await wrapper.findAll('.tabs button')[6].trigger('click')
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

  it('uses a valid initial cash default', async () => {
    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.findAll('.tabs button')[2].trigger('click')
    const input = wrapper.get('#experiment-initial-cash').element

    expect(input.value).toBe('10000')
    expect(input.step).toBe('1')
    expect(input.validity.valid).toBe(true)
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

    await options[0].trigger('click')
    expect(wrapper.get('#experiment-base-currency').text()).toContain('EUR')
  })

  it('opens the failing tab, shows the error, and focuses its widget', async () => {
    const wrapper = mount(ExperimentPage, { attachTo: document.body, props: { bootstrap } })
    await flushPromises()
    await wrapper.get('#experiment-name').setValue('Validation study')
    await wrapper.findAll('.tabs button')[6].trigger('click')

    await wrapper.get('button[type="submit"]').trigger('submit')
    await flushPromises()

    expect(wrapper.get('.tabs button.active').text()).toContain('Market data')
    expect(wrapper.get('.form-alert').text()).toContain('Select at least one market symbol.')
    expect(document.activeElement?.id).toBe('experiment-symbols')
    expect(wrapper.emitted('toast')[0]).toEqual([
      'Select at least one market symbol.',
      'error'
    ])
    wrapper.unmount()
  })

  it('surfaces catalog failures on the market-data tab', async () => {
    query.mockRejectedValueOnce(new Error('Provider unavailable.'))

    const wrapper = mount(ExperimentPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.tabs button.active').text()).toContain('Market data')
    expect(wrapper.get('.form-alert').text()).toContain(
      'Could not load the symbol catalog. Provider unavailable.'
    )
    expect(wrapper.emitted('toast')[0]).toEqual([
      'Could not load the symbol catalog. Provider unavailable.',
      'error'
    ])
  })
})
