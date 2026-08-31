// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import IntervalPicker from '../components/interval-picker.vue'
import SearchSelect from '../components/search-select.vue'
import DownloadPage from './download-page.vue'

const { api, post, query } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  query: vi.fn()
}))

vi.mock('../api', () => ({ api, post, query }))

const bootstrap = {
  enums: {
    instrument_types: ['Stocks', 'ETF', 'Forex', 'Crypto'],
    intervals: ['1d', '1w']
  },
  display: { logokit_api_key: 'test token', date_format: 'DD-MM-YYYY' }
}

const plan = {
  available_start: '2020-01-01',
  available_end: '2026-08-10',
  summary: {
    estimated_bars: 1650,
    estimated_seconds: 0.04,
    estimated_bytes: 198000,
    series: 1
  },
  profiles: [{
    symbol: 'AAPL',
    name: 'Apple Inc.',
    instrument_type: 'Stocks',
    provider: 'yahoo',
    exchange: 'XNAS',
    market_country_code: 'us',
    quote: 'USD',
    currency_country_code: 'us',
    legs: [],
    intervals: [{
      interval: '1d',
      available_start: '2020-01-01',
      available_end: '2026-08-10',
      download_start: '2020-01-01',
      download_end: '2026-08-10',
      estimated_bars: 1650,
      days: 2414
    }]
  }, {
    symbol: 'EUR-USD',
    name: 'EUR/USD',
    instrument_type: 'Forex',
    provider: 'yahoo',
    exchange: 'CCY',
    quote: 'USD',
    currency_country_code: 'us',
    legs: [],
    intervals: [{
      interval: '1d',
      available_start: '2020-01-01',
      available_end: '2026-08-10',
      download_start: '2020-01-01',
      download_end: '2026-08-10',
      estimated_bars: 1650,
      days: 2414
    }]
  }, {
    symbol: 'BTC-USDT',
    name: 'Bitcoin / Tether',
    instrument_type: 'Crypto',
    provider: 'binance',
    exchange: 'BINANCE',
    quote: 'USDT',
    currency_country_code: 'us',
    legs: [],
    intervals: [{
      interval: '1d',
      available_start: '2020-01-01',
      available_end: '2026-08-10',
      download_start: '2020-01-01',
      download_end: '2026-08-10',
      estimated_bars: 1650,
      days: 2414
    }]
  }]
}

describe('download page', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    sessionStorage.clear()
    api.mockReset().mockResolvedValue(plan)
    post.mockReset()
    query.mockReset().mockResolvedValue([
      { symbol: 'AAPL', name: 'Apple Inc.', instrument_type: 'stocks' }
    ])
  })

  afterEach(() => vi.useRealTimers())

  it('shows plain intervals and provider planning details after selecting a symbol', async () => {
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    const selectors = wrapper.findAllComponents(SearchSelect)
    expect(selectors).toHaveLength(1)
    expect(wrapper.getComponent(IntervalPicker).props()).toMatchObject({
      modelValue: ['1d'],
      multiple: true
    })

    await selectors[0].get('input').trigger('focus')
    await selectors[0].get('.search-menu button').trigger('click')
    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toBe(
      'https://img.logokit.com/ticker/AAPL?token=test%20token'
    )
    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/downloads/plan', expect.objectContaining({
      method: 'POST',
      body: expect.objectContaining({ symbols: ['AAPL'], intervals: ['1d'] })
    }))
    expect(wrapper.get('.download-metrics').text()).toContain('1,650')
    expect(wrapper.findAll('.download-metrics .metric-icon svg')).toHaveLength(3)
    expect(wrapper.get('.download-profile').text()).toContain('Apple Inc.')
    expect(wrapper.find('.download-summary').exists()).toBe(false)
    expect(wrapper.get('.download-provider').text()).toBe('')
    expect(wrapper.get('.download-provider img').attributes('src')).toBe('/providers/yahoo.png')
    expect(wrapper.get('.download-provider img').attributes('alt')).toBe('yahoo provider')
    expect(wrapper.get('.download-profile-meta').text()).toContain('ExchangeXNAS')
    expect(wrapper.get('.download-profile-meta').text()).toContain('CurrencyUSD')
    expect(wrapper.get('.download-profile-exchange img').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    expect(wrapper.get('.download-profile-currency img').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    const profiles = wrapper.findAll('.download-profile')
    expect(profiles[1].get('.download-profile-icon img').attributes('src')).toBe(
      'https://img.logokit.com/ticker/EURUSD:CUR?token=test%20token'
    )
    expect(profiles[1].find('.download-profile-exchange').exists()).toBe(false)
    expect(profiles[1].find('.download-profile-currency').exists()).toBe(true)
    expect(profiles[2].find('.download-profile-exchange').exists()).toBe(false)
    expect(profiles[2].find('.download-profile-currency').exists()).toBe(false)
    const interval = wrapper.get('.download-interval-row').text()
    expect(interval).toContain('Provider availability')
    expect(interval).toContain('6 years 224 days')
    expect(interval).toContain('Download range')
    expect(interval).toContain('01-01-2020')
    expect(interval).toContain('10-08-2026')
    expect(wrapper.get('.download-row-value').text()).toContain('1,650bars')
  })

  it('keeps the current plan mounted while refreshing date availability', async () => {
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    const symbolSelect = wrapper.findAllComponents(SearchSelect)[0]
    await symbolSelect.get('input').trigger('focus')
    await symbolSelect.get('.search-menu button').trigger('click')
    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()

    const details = wrapper.get('.download-details').element
    let resolvePlan
    api.mockImplementationOnce(() => new Promise(resolve => { resolvePlan = resolve }))

    await wrapper.get('.toggle').setValue(false)

    expect(wrapper.get('.download-details').element).toBe(details)
    expect(wrapper.find('.download-plan-loading').exists()).toBe(false)

    await vi.advanceTimersByTimeAsync(300)
    resolvePlan(plan)
    await flushPromises()
  })

  it('finishes successful downloads and offers an analysis action', async () => {
    post.mockResolvedValue({ id: 'download-1', status: 'queued', result: null })
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('.wide-control').findAll('button')[1].trigger('click')
    await flushPromises()
    const symbolSelect = wrapper.findAllComponents(SearchSelect)[0]
    await symbolSelect.get('input').trigger('focus')
    await symbolSelect.get('.search-menu button').trigger('click')
    const intervalSelect = wrapper.getComponent(IntervalPicker)
    await intervalSelect.findAll('button')[1].trigger('click')
    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()

    api.mockResolvedValueOnce({
      id: 'download-1',
      status: 'success',
      result: { n_succeeded: 2, n_failed: 0, warnings: [] },
      error: null
    })
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(wrapper.get('.job-card').text()).toContain('Download complete')
    expect(wrapper.get('.job-card').text()).toContain('2 series downloaded and stored locally.')
    expect(wrapper.find('.job-card .spinner').exists()).toBe(false)
    expect(wrapper.get('.wide-control button.active').text()).toContain('ETF')
    expect(symbolSelect.props('modelValue')).toEqual([])
    expect(intervalSelect.props('modelValue')).toEqual(['1d'])

    await wrapper.get('.job-card button').trigger('click')
    expect(wrapper.emitted('navigate')).toEqual([['analysis']])
    expect(JSON.parse(sessionStorage.getItem('backtide:analysis-symbols'))).toEqual(['AAPL'])
  })

  it('keeps at least one download interval selected', async () => {
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()
    const intervalSelect = wrapper.getComponent(IntervalPicker)
    const intervalButtons = intervalSelect.findAll('button')

    await intervalButtons[0].trigger('click')
    expect(intervalSelect.props('modelValue')).toEqual(['1d'])

    await intervalButtons[1].trigger('click')
    await intervalButtons[0].trigger('click')
    await intervalButtons[1].trigger('click')

    expect(intervalSelect.props('modelValue')).toEqual(['1w'])
    expect(intervalSelect.findAll('[aria-pressed="true"]')).toHaveLength(1)
  })

  it('matches the experiment instrument tabs and clears search when the type changes', async () => {
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.findAll('.segmented button svg')).toHaveLength(4)
    await wrapper.get('#download-symbols').setValue('AAPL')

    query.mockResolvedValueOnce([
      { symbol: 'BTC-USD', name: 'Bitcoin / US Dollar', instrument_type: 'Crypto' }
    ])
    await wrapper.findAll('.segmented button')[3].trigger('click')
    await flushPromises()

    expect(wrapper.get('#download-symbols').element.value).toBe('')
    expect(query).toHaveBeenLastCalledWith('/api/instruments', {
      instrument_type: 'crypto',
      source: 'catalog',
      limit: 1500
    })
  })

  it('loads a logo for a selected custom symbol outside the current catalog', async () => {
    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    const input = wrapper.get('#download-symbols')
    await input.setValue('AVIANRO')
    await input.trigger('keydown.enter')

    expect(wrapper.get('.tag').text()).toContain('AVIANRO')
    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toBe(
      'https://img.logokit.com/ticker/AVIANRO?token=test%20token'
    )
    expect(wrapper.find('.logo-attribution').exists()).toBe(false)
  })

  it('surfaces symbol catalog failures in the form', async () => {
    query.mockRejectedValueOnce(new Error('Provider unavailable.'))

    const wrapper = mount(DownloadPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.download-plan-error').text()).toContain(
      'Could not load the symbol catalog. Provider unavailable.'
    )
    expect(wrapper.emitted('toast')[0]).toEqual([
      'Could not load the symbol catalog. Provider unavailable.',
      'error'
    ])
  })
})
