// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, nextTick, ref } from 'vue'
import LivePage from './live-page.vue'

const { api, post, query } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  query: vi.fn()
}))

vi.mock('../api', () => ({ api, post, query }))
vi.mock('../components/chart-panel.vue', () => ({
  default: {
    name: 'ChartPanelStub',
    props: { figure: Object, emptyMessage: String },
    template: '<div class="chart-stub" />'
  }
}))

const supported = reason => ({ supported: true, reason })
const unsupported = reason => ({ supported: false, reason })
const liveInstrumentCatalog = [
  { symbol: 'ADA-USD', name: 'Cardano', instrument_type: 'Crypto', provider: 'Kraken' },
  { symbol: 'BTC-USD', name: 'Bitcoin', instrument_type: 'Crypto', provider: 'Kraken' },
  { symbol: 'ETH-USD', name: 'Ethereum', instrument_type: 'Crypto', provider: 'Kraken' }
]
const bootstrap = {
  defaults: { portfolio: { initial_cash: 10000, base_currency: 'EUR' } },
  enums: {
    intervals: ['1m', '5m'],
    currencies: [
      { code: 'EUR', name: 'Euro', country_code: 'eu', flag: '🇪🇺' },
      { code: 'USD', name: 'United States Dollar', country_code: 'us', flag: '🇺🇸' }
    ]
  },
  display: { logokit_api_key: 'test-token' },
  live: {
    providers: {
      kraken: {
        supported: true,
        intervals: {
          '1m': supported('Kraken public OHLC WebSocket v2'),
          '5m': supported('Kraken public OHLC WebSocket v2')
        }
      },
      binance: {
        supported: true,
        intervals: {
          '1m': supported('Binance public kline WebSocket'),
          '5m': supported('Binance public kline WebSocket')
        }
      },
      coinbase: {
        supported: true,
        intervals: {
          '1m': unsupported('Coinbase candles are available at 5m only.'),
          '5m': supported('Coinbase Advanced Trade public candles WebSocket')
        }
      },
      yahoo: {
        supported: false,
        intervals: {
          '1m': unsupported('Yahoo Finance has no official market-data WebSocket.'),
          '5m': unsupported('Yahoo Finance has no official market-data WebSocket.')
        }
      }
    }
  },
  strategies: { saved: [] }
}

const completedReplay = {
  status: 'stopped',
  config: {
    mode: 'replay', provider: 'kraken', interval: '1m', symbols: ['BTC-USD'],
    strategies: [], config: { initial_cash: 10_000, base_currency: 'EUR' }
  },
  snapshot: {
    latest_prices: { 'BTC-USD': 60_000 }, equity: 10_000,
    portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
  },
  updates: [{
    market: {
      symbol: 'BTC-USD', received_ts: 1_700_000_000,
      open: 59_900, high: 60_100, low: 59_800, close: 60_000,
      volume: 2.5, is_final: true
    },
    fills: [], snapshot: { equity: 10_000 }
  }],
  error: null
}

describe('live page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue({
      status: 'idle', config: {}, snapshot: {}, updates: [], error: null
    })
    post.mockReset()
    query.mockReset().mockResolvedValue(liveInstrumentCatalog)
  })

  it('uses a valid and editable initial cash default', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    const input = wrapper.get('#live-initial-cash').element

    expect(input.value).toBe('10000')
    expect(input.min).toBe('0')
    expect(input.step).toBe('100')
    expect(input.validity.valid).toBe(true)
    input.stepUp()
    expect(input.value).toBe('10100')
    input.stepDown()
    expect(input.value).toBe('10000')
  })

  it('keeps percentage limits valid at their one-hundred-percent defaults', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    const percentageLimits = wrapper.findAll('input[min="0.01"][max="100"]')

    expect(percentageLimits).toHaveLength(2)
    for (const input of percentageLimits) {
      expect(input.attributes('step')).toBe('0.01')
      expect(input.element.value).toBe('100')
      expect(input.element.validity.valid).toBe(true)
    }
  })

  it('aligns live configuration labels with the experiment setup', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.findAll('.live-form-tabs button').map(button => button.text())).toEqual([
      '1Market data',
      '2Strategy & metrics',
      '3Portfolio & execution',
      '4Risk'
    ])
    const visibleLabels = () => Array.from(wrapper.get('.form-grid').element.children)
      .filter(field => field.style.display !== 'none')
      .map(field => {
        const heading = field.tagName === 'FIELDSET' ? field.querySelector('legend') : field
        const directText = [...heading.childNodes].find(node =>
          node.nodeType === Node.TEXT_NODE && node.textContent.trim())
        if (directText) return directText.textContent.trim()
        return [...heading.firstElementChild.childNodes].find(node =>
          node.nodeType === Node.TEXT_NODE && node.textContent.trim())?.textContent.trim()
      })

    expect(visibleLabels()).toEqual([
      'Provider',
      'Symbols',
      'Interval',
      'Warm-up bars',
      'History limit'
    ])
    expect(wrapper.get('.live-interval-field').classes()).not.toContain('wide')
    await wrapper.findAll('.live-form-tabs button')[1].trigger('click')
    expect(visibleLabels()).toEqual([
      'Strategies',
      'Indicators',
      'Metrics',
      'Risk-free rate (%)',
      'Trade partial bars'
    ])
  })

  it('can start from every setup step with sensible defaults', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    const tabs = wrapper.findAll('.live-form-tabs button')

    expect(wrapper.get('.live-button').element.disabled).toBe(false)
    expect(wrapper.get('#live-initial-cash').element.value).toBe('10000')
    for (const tab of tabs) {
      await tab.trigger('click')
      expect(wrapper.get('.live-button').text()).toContain('Start live session')
      expect(wrapper.get('.live-button').element.disabled).toBe(false)
    }
  })

  it('explains every live-session setting', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const settings = [...wrapper.get('.form-grid').element.children]
    expect(settings.length).toBeGreaterThan(20)
    expect(settings.every(setting => setting.querySelector('.field-info'))).toBe(true)
    expect(wrapper.findAll('.toggle-label small')).toHaveLength(0)
  })

  it('refreshes a cached page and opens a completed replay instead of setup', async () => {
    api.mockReset()
      .mockResolvedValueOnce({
        status: 'idle', config: {}, snapshot: {}, updates: [], error: null
      })
      .mockResolvedValueOnce(completedReplay)
    const show = ref(true)
    const Host = defineComponent({
      components: { LivePage },
      setup: () => ({ bootstrap, show }),
      template: '<KeepAlive><LivePage v-if="show" :bootstrap="bootstrap" /></KeepAlive>'
    })
    const wrapper = mount(Host)
    await flushPromises()
    expect(wrapper.find('.live-setup').exists()).toBe(true)

    show.value = false
    await nextTick()
    show.value = true
    await nextTick()
    await flushPromises()

    expect(api).toHaveBeenCalledTimes(2)
    expect(wrapper.find('.live-setup').exists()).toBe(false)
    expect(wrapper.find('.live-dashboard').exists()).toBe(true)
    expect(wrapper.text()).toContain('Live market prices')
    expect(wrapper.get('.session-actions').text()).toContain('Replay complete')
    expect(wrapper.get('.session-actions').text()).not.toContain('Stop')
    wrapper.unmount()
  })

  it('submits complete execution and risk defaults from the wizard', async () => {
    post.mockResolvedValue({
      status: 'running',
      config: {},
      snapshot: {},
      strategies: {},
      updates: [],
      health: {},
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/live', expect.objectContaining({
      provider: 'kraken',
      interval: '1m',
      warmup_bars: 500,
      strategies: [],
      indicators: [],
      config: expect.objectContaining({
        allow_margin: false,
        max_leverage: 2,
        initial_margin: 50,
        maintenance_margin: 25,
        max_position_size: 100,
        max_drawdown: 0,
        partial_fills: false,
        max_volume_participation: 100,
        metrics: expect.arrayContaining(['total_return', 'sharpe', 'max_dd'])
      })
    }))
  })

  it('uses the searchable currency selector with flags', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const trigger = wrapper.get('#live-base-currency')
    expect(trigger.text()).toContain('EUR')
    expect(trigger.get('img').attributes('src')).toBe('https://flagcdn.com/eu.svg')

    await trigger.trigger('click')
    const search = wrapper.get('[aria-label="Search base currencies"]')
    await search.setValue('euro')
    await wrapper.get('[role="option"]').trigger('click')

    expect(trigger.text()).toContain('EUR')
    expect(trigger.get('img').attributes('src')).toBe('https://flagcdn.com/eu.svg')
  })

  it('uses provider logos and selects the first supported Coinbase interval', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const coinbase = wrapper.get('[aria-label="Coinbase"]')

    expect(wrapper.find('[aria-label="Yahoo, unavailable"]').exists()).toBe(false)
    expect(coinbase.element.disabled).toBe(false)
    expect(coinbase.get('img').attributes('src')).toBe('/providers/coinbase.png')

    await coinbase.trigger('click')

    const interval = wrapper.get('select')
    expect(interval.element.value).toBe('5m')
    expect(interval.findAll('option').map(option => option.text())).toEqual(['5m'])
    expect(coinbase.attributes('aria-checked')).toBe('true')
    expect(query).toHaveBeenLastCalledWith('/api/live/instruments', {
      provider: 'coinbase',
      limit: 10000
    })
    expect(wrapper.find('.provider-support').exists()).toBe(false)
    expect(wrapper.get('.safety-panel').text()).not.toContain('Coinbase live candles')
    expect(wrapper.find('.safety-panel .callout').exists()).toBe(false)
  })

  it('shows symbol logos in the menu and selected tags', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toContain(
      'img.logokit.com/crypto/BTC'
    )

    await wrapper.get('#live-symbols').trigger('focus')

    expect(wrapper.findAll('.search-option-logo img').map(image => image.attributes('src'))).toContain(
      'https://img.logokit.com/crypto/ETH?token=test-token'
    )
    expect(wrapper.find('.logo-attribution').exists()).toBe(false)
  })

  it('searches the selected provider catalog beyond the former static shortlist', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(query).toHaveBeenCalledWith('/api/live/instruments', {
      provider: 'kraken',
      limit: 10000
    })

    const search = wrapper.get('#live-symbols')
    await search.trigger('focus')
    await search.setValue('cardano')

    const options = wrapper.findAll('.search-menu button')
    expect(options).toHaveLength(1)
    expect(options[0].text()).toContain('ADA-USD')
    expect(options[0].text()).toContain('Cardano')
  })

  it('plots the WebSocket close price with OHLCV details while monitoring', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategy: '',
        config: { initial_cash: 10_000, base_currency: 'EUR' }
      },
      snapshot: {
        latest_prices: { 'BTC-USD': 60_000 }, equity: 10_000,
        portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
      },
      updates: [
        {
          market: {
            symbol: 'BTC-USD', received_ts: 1_700_000_000,
            open: 59_900, high: 60_100, low: 59_800, close: 60_000,
            volume: 2.5, is_final: false
          },
          fills: [], snapshot: { equity: 10_000 }
        },
        {
          market: {
            symbol: 'BTC-USD', received_ts: 1_700_000_060,
            open: 60_000, high: 60_300, low: 59_950, close: 60_250,
            volume: 3, is_final: true
          },
          fills: [], snapshot: { equity: 10_000 }
        }
      ],
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const chart = wrapper.getComponent({ name: 'ChartPanelStub' })
    const figure = chart.props('figure')
    expect(chart.props('emptyMessage')).toBe('')
    expect(figure.data[0].name).toBe('BTC-USD')
    expect(figure.data[0].y).toEqual([60_000, 60_250])
    expect(figure.data[0].customdata[1]).toEqual([
      60_000, 60_300, 59_950, 60_250, 3, 'Closed candle'
    ])
    expect(figure.data[0].hovertemplate).toContain('Volume')
    expect(wrapper.get('.live-chart').text()).toContain('Live market prices')
    expect(wrapper.get('.live-chart').text()).not.toContain('Live updates')
    expect(wrapper.get('.quote-board').text()).toContain('Watchlist')
    expect(wrapper.get('.event-feed').text()).toContain('Market event feed')
    expect(wrapper.find('.live-metrics').exists()).toBe(false)
    expect(wrapper.find('.live-tables').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Cancel orders')
    expect(wrapper.text()).not.toContain('Flatten')
    expect(wrapper.text()).not.toContain('Equity')
    expect(wrapper.text()).not.toContain('Realized P&L')
    expect(wrapper.text()).not.toContain('Unrealized P&L')
    expect(wrapper.text()).not.toContain('Open positions')
    expect(wrapper.text()).not.toContain('Positions & cash')
    expect(wrapper.text()).not.toContain('Recent order outcomes')
    expect(wrapper.findAll('.event-volume').map(value => value.text())).toEqual([
      'volume 3',
      'volume 2.5'
    ])
    wrapper.unmount()
  })

  it('plots net profit and loss after a filled strategy order', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategy: 'Momentum',
        config: { initial_cash: 10_000, base_currency: 'EUR' }
      },
      snapshot: {
        latest_prices: { 'BTC-USD': 110 }, equity: 10_025,
        portfolio: { positions: { 'BTC-USD': 1 }, cash: { EUR: 9_915 }, orders: [] }
      },
      updates: [
        {
          market: { symbol: 'BTC-USD', received_ts: 1_700_000_000 },
          fills: [{ status: 'Filled', order: { symbol: 'BTC-USD' } }],
          snapshot: { equity: 10_000 }
        },
        {
          market: { symbol: 'BTC-USD', received_ts: 1_700_000_060 },
          fills: [], snapshot: { equity: 10_025 }
        }
      ],
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const chart = wrapper.getComponent({ name: 'ChartPanelStub' })
    expect(chart.props('emptyMessage')).toBe('')
    expect(chart.props('figure').data[0].y).toEqual([0, 25])
    expect(chart.props('figure').data[0].fill).toBeUndefined()
    expect(chart.props('figure').layout.yaxis.title).toBe('Net P&L (EUR)')
    expect(wrapper.get('.live-metrics').text()).toContain('Equity')
    expect(wrapper.get('.live-tables').text()).toContain('Positions & cash')
    expect(wrapper.get('.live-tables').text()).toContain('Recent order outcomes')
    expect(wrapper.text()).toContain('Cancel orders')
    expect(wrapper.text()).toContain('Flatten')
    expect(wrapper.findAll('.action-popover').map(item => item.text())).toEqual([
      'Cancel every open simulated order before the next market update.',
      'Close all simulated positions on the next market update.'
    ])
    wrapper.unmount()
  })

  it('renders a multi-symbol live watchlist and explains bounded rejections', async () => {
    const updates = Array.from({ length: 15 }, (_, index) => ({
      market: {
        symbol: 'BTC-USDT',
        received_ts: 1_700_000_000 + index,
        close_ts: 1_700_000_060 + index,
        is_final: true,
        close: 60_000,
        volume: 2
      },
      fills: [{
        timestamp: 1_700_000_060 + index,
        status: 'Rejected',
        reason: 'insufficient cash',
        order: { id: `order-${index}`, symbol: 'BTC-USDT' }
      }],
      snapshot: { equity: 10_000 }
    }))
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'binance',
        interval: '1m',
        symbols: ['BTC-USDT', 'ETH-USDT'],
        strategy: 'Momentum',
        config: { base_currency: 'EUR' }
      },
      snapshot: {
        latest_prices: { 'BTC-USDT': 60_000 },
        equity: 10_000,
        realized_pnl: 0,
        unrealized_pnl: 0,
        processed_bars: 15,
        portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
      },
      updates,
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const quotes = wrapper.findAll('.quote-row')
    expect(quotes).toHaveLength(2)
    expect(quotes[0].get('img').attributes('src')).toContain('img.logokit.com/crypto/BTC')
    expect(quotes[0].text()).toContain('60,000')
    expect(quotes[1].get('img').attributes('src')).toContain('img.logokit.com/crypto/ETH')
    expect(quotes[1].text()).toContain('Waiting for price')
    expect(wrapper.get('.live-metrics').text()).toContain('€10,000.00')

    const executions = wrapper.findAll('.live-execution-table tbody tr')
    expect(executions).toHaveLength(12)
    expect(executions[0].get('.badge').classes()).toContain('error')
    expect(executions[0].text()).toContain('insufficient cash')
    wrapper.unmount()
  })
})
