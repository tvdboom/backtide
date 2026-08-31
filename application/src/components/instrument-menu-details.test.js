// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import InstrumentMenuDetails from './instrument-menu-details.vue'

const { query } = vi.hoisted(() => ({ query: vi.fn() }))

vi.mock('../api', () => ({ query }))

describe('instrument-menu-details', () => {
  beforeEach(() => query.mockReset())

  it('shows the instrument, currency flag, provider, and full market identity without fetching', () => {
    const wrapper = mount(InstrumentMenuDetails, {
      props: {
        symbol: 'AAPL',
        logo: 'https://example.test/aapl.png',
        details: {
          name: 'Apple Inc.',
          exchange: 'XNAS',
          exchange_mic: 'XNAS',
          exchange_name: 'NASDAQ Global Select Market',
          market_country_code: 'us',
          quote: 'usd',
          currency_country_code: 'us',
          provider: 'yahoo',
          sparkline: [180, 182, 179, 185]
        }
      }
    })

    expect(wrapper.text()).toContain('Apple Inc.')
    expect(wrapper.get('.instrument-market-identity').text()).toBe(
      'NASDAQ Global Select Market (XNAS)'
    )
    expect(wrapper.text()).toContain('USD')
    expect(wrapper.get('.instrument-menu-heading').element.firstElementChild).toBe(
      wrapper.get('.instrument-menu-logo').element
    )
    expect(wrapper.get('.instrument-menu-logo img').attributes('src')).toBe(
      'https://example.test/aapl.png'
    )
    expect(wrapper.get('.instrument-currency-fact img').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    expect(wrapper.get('.instrument-market-fact img').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    expect(wrapper.get('.instrument-menu-provider img').attributes('src')).toBe(
      '/providers/yahoo.png'
    )
    expect(wrapper.find('.instrument-provider-fact').exists()).toBe(false)
    expect(wrapper.findAll('dt').map(item => item.text())).not.toContain('Provider')
    expect(wrapper.find('svg.instrument-menu-sparkline').exists()).toBe(false)
    expect(query).not.toHaveBeenCalled()
  })

  it('fetches direct-provider prices only when activated and renders chart axes', async () => {
    query.mockResolvedValue({
      exchange_mic: 'XNAS',
      exchange_name: 'NASDAQ Global Select Market',
      quote: 'USD',
      provider: 'yahoo',
      sparkline: [180, 182, 179, 185],
      sparkline_ts: [1_693_526_400, 1_693_612_800, 1_693_699_200, 1_693_785_600]
    })
    const wrapper = mount(InstrumentMenuDetails, {
      props: {
        symbol: 'AAPL',
        display: { date_format: 'DD-MM-YYYY', timezone: 'UTC' },
        details: { name: 'Apple Inc.', instrument_type: 'stocks', provider: 'yahoo' }
      }
    })

    expect(query).not.toHaveBeenCalled()
    expect(wrapper.find('svg.instrument-menu-sparkline').exists()).toBe(false)

    await wrapper.setProps({ loadGraph: true })
    await flushPromises()

    expect(query).toHaveBeenCalledWith('/api/instrument-overview', {
      symbol: 'AAPL', instrument_type: 'stocks', provider: 'yahoo'
    })
    expect(wrapper.get('svg.instrument-menu-sparkline').attributes('aria-label')).toBe(
      'AAPL recent daily closing-price trend'
    )
    expect(wrapper.findAll('.instrument-menu-chart-grid line')).toHaveLength(3)
    expect(wrapper.get('polyline').attributes('points')).toMatch(/^1\.00,.*359\.00,/)
    expect(wrapper.get('polygon').attributes('points')).not.toBe('')
    expect(wrapper.get('.instrument-menu-chart-heading').text()).toContain('+2.78%')
    expect(wrapper.findAll('.instrument-menu-chart-y-axis span')).toHaveLength(3)
    expect(wrapper.findAll('.instrument-menu-chart-x-axis span').map(item => item.text())).toEqual([
      '01-09-2023',
      '04-09-2023'
    ])
  })

  it.each([
    {
      instrumentType: 'forex',
      provider: 'yahoo',
      market: false,
      currency: true
    },
    {
      instrumentType: 'crypto',
      provider: 'binance',
      market: false,
      currency: false
    }
  ])('shows the applicable $instrumentType metadata in one row', ({
    instrumentType, provider, market, currency
  }) => {
    const wrapper = mount(InstrumentMenuDetails, {
      props: {
        symbol: instrumentType === 'crypto' ? 'BTC-USDT' : 'EUR-USD',
        details: {
          instrument_type: instrumentType,
          exchange_mic: 'XNAS',
          exchange_name: 'NASDAQ Global Select Market',
          market_country_code: 'us',
          quote: 'USD',
          currency_country_code: 'us',
          provider
        }
      }
    })

    expect(wrapper.find('.instrument-market-fact').exists()).toBe(market)
    expect(wrapper.find('.instrument-currency-fact').exists()).toBe(currency)
    expect(wrapper.find('.instrument-menu-provider').exists()).toBe(true)
    expect(wrapper.find('.instrument-provider-fact').exists()).toBe(false)
  })

  it('omits the graph when the direct provider has no recent prices', async () => {
    query.mockResolvedValue({ exchange_mic: 'XNYS', quote: 'USD', sparkline: [] })
    const wrapper = mount(InstrumentMenuDetails, {
      props: { symbol: 'MSFT', details: { provider: 'yahoo' }, loadGraph: true }
    })
    await flushPromises()

    expect(wrapper.find('svg.instrument-menu-sparkline').exists()).toBe(false)
    expect(wrapper.text()).toContain('No recent price data.')
  })
})
