// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, nextTick, ref } from 'vue'
import IntervalPicker from '../components/interval-picker.vue'
import SearchSelect from '../components/search-select.vue'
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
      { code: 'EUR', name: 'Euro', country_code: 'eu', flag: '🇪🇺', decimals: 2 },
      {
        code: 'USD', name: 'United States Dollar', country_code: 'us', flag: '🇺🇸', decimals: 2
      }
    ]
  },
  display: { currency_prefix: true, logokit_api_key: 'test-token' },
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
    sessionStorage.clear()
    api.mockReset().mockResolvedValue({
      status: 'idle', config: {}, snapshot: {}, updates: [], error: null
    })
    post.mockReset()
    query.mockReset().mockResolvedValue(liveInstrumentCatalog)
  })

  it('uses a valid and editable initial cash default', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.live-form-tabs button')[1].trigger('click')
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
    const tabs = wrapper.findAll('.live-form-tabs button')
    await tabs[4].trigger('click')
    const percentageLimits = [wrapper.get('input[min="0.01"][max="100"]')]
    await tabs[5].trigger('click')
    percentageLimits.push(wrapper.get('input[min="0.01"][max="100"]'))

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
      '2Portfolio',
      '3Strategy',
      '4Metrics',
      '5Execution',
      '6Risk',
      '7Engine'
    ])
    const visibleLabels = () => Array.from(wrapper.get('.form-section .form-grid').element.children)
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
      'Interval'
    ])
    expect(wrapper.get('.live-interval-field').classes()).toContain('wide')
    expect(wrapper.get('.live-interval-field').classes()).toContain('interval-picker-field')

    await wrapper.findAll('.live-form-tabs button')[1].trigger('click')
    expect(wrapper.get('.section-copy h3').text()).toBe('Starting portfolio')
    expect(wrapper.get('.live-portfolio-basics').exists()).toBe(true)
    expect(wrapper.get('.live-base-currency').classes()).toContain('currency-picker-field')

    await wrapper.findAll('.live-form-tabs button')[2].trigger('click')
    expect(visibleLabels()).toEqual([
      'Strategies',
      'Indicators'
    ])

    await wrapper.findAll('.live-form-tabs button')[3].trigger('click')
    expect(visibleLabels()).toEqual([
      'Metrics'
    ])

    await wrapper.findAll('.live-form-tabs button')[5].trigger('click')
    expect(wrapper.findAll('.settings-group .form-grid').map(grid => grid.classes())).toEqual([
      ['form-grid', 'two'],
      ['form-grid', 'two']
    ])

    await wrapper.findAll('.live-form-tabs button')[6].trigger('click')
    expect(visibleLabels()).toEqual([
      'Warm-up bars',
      'Risk-free rate (%)',
      'History limit',
      'Trade partial bars'
    ])
  })

  it('reorders and removes live metrics from the full draggable cards', async () => {
    const pageBootstrap = {
      ...bootstrap,
      metrics: {
        builtin: [
          {
            key: 'total_return', name: 'Total return', description: 'Net return.',
            builtin: true, percentage: true
          },
          {
            key: 'final_equity', name: 'Final equity', description: 'Ending value.',
            builtin: true, percentage: false
          },
          {
            key: 'sharpe', name: 'Sharpe ratio', description: 'Risk adjusted.',
            builtin: true, percentage: false
          },
          {
            key: 'n_trades', name: 'Number of trades', description: 'Completed trades.',
            builtin: true, percentage: false
          }
        ],
        saved: [{
          key: 'Custom score', name: 'Custom score', description: 'User metric.',
          builtin: false, percentage: false
        }]
      }
    }
    sessionStorage.setItem('backtide:session-config', JSON.stringify({
      config: { metrics: ['total_return', 'n_trades', 'final_equity', 'sharpe', 'Custom score'] }
    }))
    const wrapper = mount(LivePage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.live-form-tabs button')[3].trigger('click')

    const selectedKeys = () => wrapper.getComponent(SearchSelect).props('modelValue')
    expect(selectedKeys()).toEqual([
      'total_return', 'n_trades', 'final_equity', 'sharpe', 'Custom score'
    ])
    expect(wrapper.getComponent(SearchSelect).props('options')).toContain('Custom score')
    expect(wrapper.findAll('.metric-selection-card strong').map(item => item.text()))
      .toEqual(['Total return', 'Number of trades', 'Final equity', 'Sharpe ratio', 'Custom score'])
    expect(wrapper.findAll('.metric-selection-card').every(card =>
      card.attributes('draggable') === 'true'
    )).toBe(true)

    const dataTransfer = {
      effectAllowed: '',
      setData: vi.fn(),
      setDragImage: vi.fn()
    }
    let metricCards = wrapper.findAll('.metric-selection-card')
    await metricCards[3].trigger('dragstart', { dataTransfer })
    expect(dataTransfer.setDragImage).toHaveBeenCalled()
    expect(document.body.querySelector('.metric-drag-preview')).not.toBeNull()
    await metricCards[0].trigger('dragover', { dataTransfer })
    expect(selectedKeys()).toEqual([
      'sharpe', 'total_return', 'n_trades', 'final_equity', 'Custom score'
    ])
    metricCards = wrapper.findAll('.metric-selection-card')
    expect(metricCards[0].classes()).toContain('dragging')
    expect(metricCards[1].classes()).toContain('drop-target')
    await metricCards[0].trigger('drop', { dataTransfer })
    expect(document.body.querySelector('.metric-drag-preview')).toBeNull()

    await wrapper.get('[aria-label="Remove Final equity live metric"]').trigger('click')
    expect(selectedKeys()).toEqual(['sharpe', 'total_return', 'n_trades', 'Custom score'])
    expect(wrapper.find('[aria-label="Remove final_equity"]').exists()).toBe(false)

    await wrapper.get('[aria-label="Clear all live metrics"]').trigger('click')
    expect(selectedKeys()).toEqual([])
    expect(wrapper.find('[aria-label="Selected live metric details"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('shows library icons for live strategies and indicators', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [
      { name: 'Momentum', builtin: true },
      { name: 'Custom strategy', builtin: false }
    ]
    pageBootstrap.indicators = { saved: [
      { name: 'Simple Moving Average', builtin: true },
      { name: 'Custom indicator', builtin: false }
    ] }
    const wrapper = mount(LivePage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.live-form-tabs button')[2].trigger('click')

    const strategyInput = wrapper.get('#live-strategies')
    await strategyInput.trigger('focus')
    const strategyIcons = strategyInput.element.closest('.search-select')
      .querySelectorAll('.search-option-logo svg')
    expect(strategyIcons[0].classList).toContain('lucide-bot-icon')
    expect(strategyIcons[1].classList).toContain('lucide-square-code-icon')

    const indicatorInput = wrapper.get('#live-indicators')
    expect(indicatorInput.attributes('placeholder')).toBe('Select optional indicators...')
    await indicatorInput.trigger('focus')
    const indicatorIcons = indicatorInput.element.closest('.search-select')
      .querySelectorAll('.search-option-logo svg')
    expect(indicatorIcons[0].classList).toContain('lucide-shapes-icon')
    expect(indicatorIcons[1].classList).toContain('lucide-braces-icon')
  })

  it('keeps selected strategies when the help text below the selector is clicked', async () => {
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.saved = [{ name: 'Momentum', builtin: true }]
    const wrapper = mount(LivePage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()
    await wrapper.findAll('.live-form-tabs button')[2].trigger('click')

    await wrapper.get('#live-strategies').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')
    expect(wrapper.get('.tag').text()).toContain('Momentum')

    const strategyField = wrapper.get('#live-strategies').element.closest('.field-label')
    strategyField.lastElementChild.click()
    await nextTick()

    expect(wrapper.get('.tag').text()).toContain('Momentum')
  })

  it('can start from every setup step with sensible defaults', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    const tabs = wrapper.findAll('.live-form-tabs button')

    expect(wrapper.get('.live-button').element.disabled).toBe(false)
    await tabs[1].trigger('click')
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

    let settingCount = 0
    for (const tab of wrapper.findAll('.live-form-tabs button')) {
      await tab.trigger('click')
      const settings = wrapper.findAll(
        '.form-section label, .form-section .field-label, .form-section .provider-field'
      )
      settingCount += settings.length
      expect(settings.every(setting => setting.find('.field-info').exists())).toBe(true)
      expect(wrapper.findAll('.toggle-label').every(toggle =>
        toggle.get('.toggle-description').text().length > 0
      )).toBe(true)
    }
    expect(settingCount).toBeGreaterThan(20)
  })

  it('refreshes a cached page and opens a replay instead of setup', async () => {
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
    expect(wrapper.get('.session-actions').text()).toContain('Replay')
    expect(wrapper.get('.session-actions').text()).not.toContain('complete')
    expect(wrapper.get('.session-actions').text()).not.toContain('Stop')
    expect(wrapper.get('.session-actions').text()).toContain('New configuration')
    expect(wrapper.get('.session-actions').text()).toContain('Session history')
    wrapper.unmount()
  })

  it.each([
    ['live session', 'live'],
    ['replay', 'replay']
  ])('keeps a stopped %s visible until a new configuration is requested', async (_, mode) => {
    const running = {
      id: `${mode}-session`,
      status: 'running',
      config: {
        mode,
        provider: 'kraken',
        interval: '1m',
        symbols: ['BTC-USD'],
        strategies: [],
        config: { initial_cash: 10_000, base_currency: 'EUR' }
      },
      snapshot: {
        latest_prices: { 'BTC-USD': 60_000 },
        equity: 10_000,
        portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
      },
      updates: [],
      error: null
    }
    api.mockResolvedValue(running)
    post.mockResolvedValue({ ...running, status: 'stopped' })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('.danger.secondary').trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/live/stop')
    expect(wrapper.find('.live-setup').exists()).toBe(false)
    expect(wrapper.find('.live-dashboard').exists()).toBe(true)
    expect(wrapper.get('.session-actions').text())
      .toContain(mode === 'replay' ? 'Replay stopped' : 'Session stopped')
    expect(wrapper.findAll('.session-actions button').map(button => button.text()))
      .toEqual(['New configuration', 'Session history'])

    await wrapper.get('.session-actions .secondary').trigger('click')
    expect(wrapper.emitted('navigate')[0]).toEqual(['live-history'])

    await wrapper.get('.session-actions .primary').trigger('click')
    expect(wrapper.find('.live-dashboard').exists()).toBe(false)
    expect(wrapper.find('.live-setup').exists()).toBe(true)
    expect(wrapper.findAll('.live-form-tabs button')[0].classes()).toContain('active')
    wrapper.unmount()
  })

  it('shows a failed session as a red badge with the full error on hover', async () => {
    const error = 'websocket error: IO error: the remote host closed the connection'
    api.mockResolvedValue({
      id: 'failed-session',
      status: 'error',
      config: {
        mode: 'live', provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategies: []
      },
      snapshot: {},
      updates: [],
      error
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const badge = wrapper.get('.session-actions .status-pill')
    const tooltip = wrapper.get('.session-status-tooltip')
    expect(badge.text()).toContain('Session failed')
    expect(badge.classes()).toContain('failed')
    expect(badge.attributes('tabindex')).toBe('0')
    expect(badge.attributes('aria-describedby')).toBe('session-error-tooltip')
    expect(tooltip.text()).toBe(error)
    expect(wrapper.find('.callout.error-state').exists()).toBe(false)

    wrapper.unmount()
  })

  it('clears a failed session error when opening a new configuration', async () => {
    const error = 'websocket error: IO error: the remote host closed the connection'
    api.mockResolvedValue({
      id: 'failed-session',
      status: 'error',
      config: {
        mode: 'live', provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategies: []
      },
      snapshot: {},
      updates: [],
      error
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('.session-actions .primary').trigger('click')

    expect(wrapper.find('.live-setup').exists()).toBe(true)
    expect(wrapper.text()).not.toContain(error)
    expect(wrapper.find('.session-status-tooltip').exists()).toBe(false)
    expect(wrapper.emitted('live-status').at(-1)[0]).toMatchObject({
      id: null,
      status: 'idle',
      error: null
    })
    wrapper.unmount()
  })

  it('applies every experiment prefill when a cached Live trading page is reopened', async () => {
    post.mockResolvedValue({
      status: 'running', config: {}, snapshot: {}, strategies: {}, updates: [], health: {}, error: null
    })
    const show = ref(true)
    const Host = defineComponent({
      components: { LivePage },
      setup: () => ({ bootstrap, show }),
      template: '<KeepAlive><LivePage v-if="show" :bootstrap="bootstrap" /></KeepAlive>'
    })
    const wrapper = mount(Host)
    await flushPromises()

    await wrapper.findAll('.live-form-tabs button')[4].trigger('click')
    const partialFills = wrapper.findAll('label')
      .find(label => label.text().includes('Volume-constrained fills'))
    await partialFills.get('input').setValue(true)
    const volumeParticipation = wrapper.findAll('label')
      .find(label => label.text().includes('Max volume participation'))
    await volumeParticipation.get('input').setValue(25)
    await wrapper.findAll('.live-form-tabs button')[5].trigger('click')
    const maxDrawdown = wrapper.findAll('label')
      .find(label => label.text().includes('Maximum drawdown halt'))
    await maxDrawdown.get('input').setValue(20)
    await wrapper.findAll('.live-form-tabs button')[6].trigger('click')
    const historyLimit = wrapper.findAll('label')
      .find(label => label.text().includes('History limit'))
    await historyLimit.get('input').setValue(500)
    const partialBars = wrapper.findAll('label')
      .find(label => label.text().includes('Trade partial bars'))
    await partialBars.get('input').setValue(true)

    show.value = false
    await nextTick()
    sessionStorage.setItem('backtide:session-config', JSON.stringify({
      provider: 'binance',
      interval: '5m',
      symbols: ['DOGE-USDT'],
      strategies: ['Momentum'],
      indicators: ['Fast SMA'],
      warmup_bars: 120,
      config: {
        initial_cash: 25_000,
        base_currency: 'USD',
        commission_pct: 0.25,
        commission_fixed: 1.5,
        slippage: 0.15,
        allow_short: true,
        allow_margin: true,
        max_leverage: 3,
        initial_margin: 40,
        maintenance_margin: 20,
        margin_interest: 4,
        borrow_rate: 2,
        max_position_size: 35,
        allowed_order_types: ['Market', 'Limit'],
        partial_fills: true,
        metrics: ['total_return', 'sharpe', 'n_trades'],
        risk_free_rate: 2.5
      }
    }))

    show.value = true
    await nextTick()
    await flushPromises()
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/live', expect.objectContaining({
      provider: 'binance',
      interval: '5m',
      symbols: ['DOGE-USDT'],
      strategies: ['Momentum'],
      indicators: ['Fast SMA'],
      warmup_bars: 120,
      config: expect.objectContaining({
        initial_cash: 25_000,
        base_currency: 'USD',
        commission_pct: 0.25,
        commission_fixed: 1.5,
        slippage: 0.15,
        allow_short: true,
        allow_margin: true,
        max_leverage: 3,
        initial_margin: 40,
        maintenance_margin: 20,
        margin_interest: 4,
        borrow_rate: 2,
        max_position_size: 35,
        allowed_order_types: ['Market', 'Limit'],
        partial_fills: true,
        max_volume_participation: 100,
        max_drawdown: 0,
        max_history: 10000,
        trade_on_partial: false,
        metrics: ['total_return', 'sharpe', 'n_trades'],
        risk_free_rate: 2.5
      })
    }))
    expect(sessionStorage.getItem('backtide:session-config')).toBeNull()
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
    await wrapper.findAll('.live-form-tabs button')[1].trigger('click')

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

  it('uses provider logos and disables unsupported Coinbase intervals', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const coinbase = wrapper.get('[aria-label="Coinbase"]')

    expect(wrapper.find('[aria-label="Yahoo, unavailable"]').exists()).toBe(false)
    expect(coinbase.element.disabled).toBe(false)
    expect(coinbase.get('img').attributes('src')).toBe('/providers/coinbase.png')

    await coinbase.trigger('click')

    const interval = wrapper.getComponent(IntervalPicker)
    expect(interval.props('modelValue')).toBe('5m')
    expect(interval.findAll('button').map(button => button.text())).toEqual(['1m', '5m'])
    expect(interval.findAll('button')[0].element.disabled).toBe(true)
    expect(interval.findAll('button')[1].element.disabled).toBe(false)
    expect(interval.get('[aria-checked="true"]').text()).toBe('5m')
    expect(coinbase.attributes('aria-checked')).toBe('true')
    expect(query).toHaveBeenLastCalledWith('/api/live/instruments', {
      provider: 'coinbase',
      limit: 10000
    })
    expect(wrapper.find('.provider-support').exists()).toBe(false)
    expect(wrapper.get('.safety-panel').text()).not.toContain('Coinbase live candles')
    expect(wrapper.find('.safety-panel .callout').exists()).toBe(false)
  })

  it('describes order types without decorative initials or repeated selected details', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    await wrapper.findAll('.tabs button')[4].trigger('click')

    const marketTag = wrapper.findAll('.tag')[0]
    expect(marketTag.text().replace('×', '').trim()).toBe('Market')
    expect(marketTag.find('.tag-copy').exists()).toBe(false)
    expect(marketTag.find('.selected-symbol-logo').exists()).toBe(false)

    await marketTag.get('button').trigger('click')
    await wrapper.get('#live-order-types').trigger('focus')

    const option = wrapper.get('.search-menu button')
    expect(option.get('strong').text()).toBe('Market')
    expect(option.get('small').text()).toBe('Execute at the best available market price.')
    expect(option.find('.search-option-logo').exists()).toBe(false)
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
          received_at: '2023-11-14T22:13:20.125Z',
          market: {
            symbol: 'BTC-USD', received_ts: 1_700_000_000,
            open: 59_900, high: 60_100, low: 59_800, close: 60_000,
            volume: 2.5, is_final: false
          },
          fills: [], snapshot: { equity: 10_000 }
        },
        {
          received_at: '2023-11-14T22:14:20.987Z',
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
    const wrapper = mount(LivePage, {
      props: {
        bootstrap: {
          ...bootstrap,
          display: {
            ...bootstrap.display,
            datetime_format: 'YYYY-MM-DD HH:mm',
            timezone: 'UTC'
          }
        }
      }
    })
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
    expect(wrapper.findAll('.event-log-header [role="columnheader"]').map(value => value.text()))
      .toEqual(['Time', 'Status', 'Symbol', 'Open', 'High', 'Low', 'Close', 'Volume', 'Fills'])
    expect(wrapper.findAll('.event-log-row time').map(value => value.text())).toEqual([
      '22:14:20.987',
      '22:13:20.125'
    ])
    expect(wrapper.findAll('.event-open').slice(1).map(value => value.text())).toEqual([
      '60,000.00',
      '59,900.00'
    ])
    expect(wrapper.findAll('.event-high').slice(1).map(value => value.text())).toEqual([
      '60,300.00',
      '60,100.00'
    ])
    expect(wrapper.findAll('.event-low').slice(1).map(value => value.text())).toEqual([
      '59,950.00',
      '59,800.00'
    ])
    expect(wrapper.findAll('.event-close').slice(1).map(value => value.text())).toEqual([
      '60,250.00',
      '60,000.00'
    ])
    expect(wrapper.findAll('.event-volume').map(value => value.text())).toEqual([
      'Volume',
      '3',
      '2.5'
    ])
    expect(wrapper.findAll('.event-symbol img')).toHaveLength(2)
    expect(wrapper.get('.event-symbol img').attributes('src')).toContain('/crypto/BTC')
    wrapper.unmount()
  })

  it('switches strategy-specific live details from the selector above metrics', async () => {
    const pageBootstrap = {
      ...bootstrap,
      metrics: {
        builtin: [
          { key: 'total_return', name: 'Total return', percentage: true },
          { key: 'final_equity', name: 'Final equity', percentage: false },
          { key: 'n_trades', name: 'Number of trades', percentage: false },
          { key: 'sharpe', name: 'Sharpe ratio', percentage: false }
        ]
      }
    }
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken',
        interval: '1m',
        symbols: ['BTC-USD', 'ETH-USD'],
        strategies: ['Buy & Hold', 'BB Mean Reversion'],
        config: {
          initial_cash: 10_000,
          base_currency: 'EUR',
          allow_short: true,
          allow_margin: true,
          metrics: ['total_return', 'n_trades', 'final_equity']
        }
      },
      snapshot: {
        latest_prices: { 'BTC-USD': 110, 'ETH-USD': 220 },
        equity: 22_300,
        portfolio: { positions: { 'BTC-USD': 1, 'ETH-USD': 2 }, cash: { EUR: 19_000 } }
      },
      strategies: {
        'Buy & Hold': {
          equity: 10_100,
          realized_pnl: 100,
          unrealized_pnl: 0,
          processed_bars: 2,
          gross_exposure: 110,
          net_exposure: 110,
          leverage: 0.01,
          buying_power: 9_990,
          drawdown: 0,
          total_costs: 1,
          metrics: { sharpe: 99, n_trades: 3, final_equity: 10_100, total_return: 0.01 },
          portfolio: { positions: { 'BTC-USD': 1 }, cash: { EUR: 9_990 } }
        },
        'BB Mean Reversion': {
          equity: 12_200,
          realized_pnl: 2_200,
          unrealized_pnl: 0,
          processed_bars: 2,
          gross_exposure: 440,
          net_exposure: 440,
          leverage: 0.04,
          buying_power: 11_760,
          drawdown: 0,
          total_costs: 2,
          metrics: { sharpe: 88, n_trades: 8, final_equity: 12_200, total_return: 0.22 },
          portfolio: { positions: { 'ETH-USD': 2 }, cash: { EUR: 11_760 } }
        }
      },
      updates: [
        {
          market: { symbol: 'BTC-USD', received_ts: 1_700_000_000 },
          strategies: {
            'Buy & Hold': {
              fills: [{ status: 'Filled', order: { symbol: 'BTC-USD' } }],
              indicators: {},
              snapshot: { equity: 10_000 }
            },
            'BB Mean Reversion': {
              fills: [{ status: 'Filled', order: { symbol: 'ETH-USD' } }],
              indicators: {},
              snapshot: { equity: 10_000 }
            }
          }
        },
        {
          market: { symbol: 'BTC-USD', received_ts: 1_700_000_060 },
          strategies: {
            'Buy & Hold': {
              fills: [],
              indicators: { 'Fast SMA': { 'BTC-USD': [[100]] } },
              snapshot: { equity: 10_100 }
            },
            'BB Mean Reversion': { fills: [], indicators: {}, snapshot: { equity: 12_200 } }
          }
        }
      ],
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()

    const tabs = wrapper.findAll('.live-strategy-panel [role="tab"]')
    expect(tabs.map(tab => tab.text())).toEqual(['Buy & Hold', 'BB Mean Reversion'])
    expect(tabs[0].attributes('aria-selected')).toBe('true')
    expect(wrapper.get('.live-strategy-panel').element.compareDocumentPosition(
      wrapper.get('.live-metrics').element
    ) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(wrapper.get('.live-portfolio-panel').element.compareDocumentPosition(
      wrapper.get('.live-chart').element
    ) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(wrapper.get('.live-chart .chart-stub').element.compareDocumentPosition(
      wrapper.get('.live-plot-tabs').element
    ) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    const plotTabs = wrapper.findAll('.live-plot-tabs [role="tab"]')
    expect(plotTabs.map(tab => tab.text())).toEqual([
      'Price', 'P&L', 'Equity', 'Exposure', 'Drawdown'
    ])
    expect(plotTabs[0].attributes('aria-selected')).toBe('true')
    expect(wrapper.get('.live-metrics').text()).toContain('€10,100.00')
    expect(wrapper.get('.live-observability').text()).toContain('1.00%')
    expect(wrapper.get('.live-observability').text()).not.toContain('22.00%')
    const riskMetrics = wrapper.findAll('[data-risk-metric]')
    expect(riskMetrics.map(item => item.attributes('data-risk-metric'))).toEqual([
      'completed-trades',
      'gross-exposure',
      'net-exposure',
      'leverage',
      'buying-power',
      'drawdown',
      'total-costs'
    ])
    expect(riskMetrics.every(item => item.find('dt svg').exists())).toBe(true)
    expect(riskMetrics.every(item => item.find('.risk-metric-help').exists())).toBe(true)
    expect(riskMetrics.map(item => item.get('.risk-metric-help').attributes('aria-label')))
      .toEqual([
        'About this setting: Number of completed trades recorded for the selected strategy in this session.',
        'About this setting: Total market value of all open positions, adding long and short positions without offsetting them.',
        'About this setting: Signed market value of open positions: long exposure minus short exposure.',
        'About this setting: Gross exposure divided by account equity. 1.00x means gross positions equal current equity.',
        'About this setting: Additional gross exposure available before reaching configured leverage or margin limits.',
        'About this setting: Percentage change from the highest equity reached. A negative value shows how far equity is below its peak.',
        'About this setting: Cumulative simulated commissions and financing costs charged during this session.'
      ])
    expect(wrapper.findAll('.live-metrics-table th').map(item => item.text()))
      .toEqual(['Metric', 'Value'])
    expect(wrapper.findAll('.live-metrics-table tbody td:first-child').map(item => item.text()))
      .toEqual(['Total return', 'Final equity'])
    expect(wrapper.find('.live-selected-metrics .metric-card').exists()).toBe(false)
    expect(wrapper.get('[data-risk-metric="completed-trades"] dd').text()).toBe('3')
    expect(wrapper.get('.live-observability article:first-child h3').text())
      .toBe('Trading, exposure & controls')
    expect(wrapper.get('.live-observability article:last-child h3').text()).toBe('Metrics')
    expect(wrapper.get('.live-observability article:last-child').text()).not.toContain('Sharpe ratio')
    expect(wrapper.get('.live-observability article:last-child').text()).not.toContain('Fast SMA')
    expect(wrapper.get('.live-observability article:first-child').text()).toContain(
      'Fast SMA · BTC-USD'
    )
    expect(wrapper.get('.live-tables').element.compareDocumentPosition(
      wrapper.get('.live-observability').element
    ) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    await plotTabs[1].trigger('click')
    expect(wrapper.getComponent({ name: 'ChartPanelStub' }).props('figure').data[0].y)
      .toEqual([0, 100])
    expect(wrapper.get('.live-portfolio-panel').text()).toContain('BTC-USD')
    expect(wrapper.get('.live-portfolio-panel').text()).not.toContain('ETH-USD')

    await tabs[1].trigger('click')

    expect(tabs[1].attributes('aria-selected')).toBe('true')
    expect(wrapper.get('.live-metrics').text()).toContain('€12,200.00')
    expect(wrapper.get('.live-observability').text()).toContain('22.00%')
    expect(wrapper.get('.live-observability').text()).not.toContain('1.00%')
    expect(wrapper.get('[data-risk-metric="completed-trades"] dd').text()).toBe('8')
    expect(wrapper.getComponent({ name: 'ChartPanelStub' }).props('figure').data[0].y)
      .toEqual([0, 2_200])
    expect(wrapper.get('.live-portfolio-panel').text()).toContain('ETH-USD')
    expect(wrapper.get('.live-portfolio-panel').text()).not.toContain('BTC-USD')
    wrapper.unmount()
  })

  it('shows only relevant exposure metrics for a cash-only long account', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken',
        interval: '1m',
        symbols: ['BTC-USD'],
        strategies: ['Buy & Hold'],
        config: {
          initial_cash: 10_000,
          base_currency: 'EUR',
          allow_short: false,
          allow_margin: false
        }
      },
      snapshot: {
        equity: 10_000,
        latest_prices: { 'BTC-USD': 100 },
        portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
      },
      strategies: {
        'Buy & Hold': {
          equity: 10_000,
          gross_exposure: 2_500,
          net_exposure: 2_500,
          leverage: 0.25,
          buying_power: 7_500,
          drawdown: 0,
          total_costs: 0,
          portfolio: { positions: {}, cash: { EUR: 10_000 }, orders: [] }
        }
      },
      updates: [],
      health: {},
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const riskMetrics = wrapper.findAll('[data-risk-metric]')
    expect(riskMetrics.map(item => item.attributes('data-risk-metric'))).toEqual([
      'completed-trades',
      'gross-exposure',
      'buying-power',
      'drawdown',
      'total-costs'
    ])
    expect(wrapper.get('[data-risk-metric="completed-trades"] dd').text()).toBe('—')
    expect(wrapper.get('[data-risk-metric="buying-power"] .risk-metric-help')
      .attributes('aria-label'))
      .toBe('About this setting: Account value still available for additional positions without borrowing funds.')
    wrapper.unmount()
  })

  it('switches among live price and strategy performance plots', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken', interval: '1m', symbols: ['BTC-USD'], strategy: 'Momentum',
        config: { initial_cash: 10_000, base_currency: 'EUR', allow_short: true }
      },
      snapshot: {
        latest_prices: { 'BTC-USD': 110 }, equity: 10_025,
        portfolio: { positions: { 'BTC-USD': 1 }, cash: { EUR: 9_915 }, orders: [] }
      },
      updates: [
        {
          market: {
            symbol: 'BTC-USD', received_ts: 1_700_000_000,
            open: 99, high: 101, low: 98, close: 100, volume: 2, is_final: true
          },
          fills: [{ status: 'Filled', order: { symbol: 'BTC-USD' } }],
          snapshot: { equity: 10_000, gross_exposure: 0, net_exposure: 0, drawdown: 0 }
        },
        {
          market: {
            symbol: 'BTC-USD', received_ts: 1_700_000_060,
            open: 100, high: 112, low: 100, close: 110, volume: 3, is_final: true
          },
          fills: [],
          snapshot: {
            equity: 10_025, gross_exposure: 110, net_exposure: 110, drawdown: -0.0025
          }
        }
      ],
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const chart = wrapper.getComponent({ name: 'ChartPanelStub' })
    expect(chart.props('emptyMessage')).toBe('')
    expect(chart.props('figure').data[0].y).toEqual([100, 110])
    expect(chart.props('figure').layout.yaxis.title).toBe('Price')
    expect(wrapper.get('.live-chart').text()).toContain('Live market prices')

    const plotTabs = wrapper.findAll('.live-plot-tabs [role="tab"]')
    await plotTabs[1].trigger('click')
    expect(chart.props('figure').data[0].y).toEqual([0, 25])
    expect(chart.props('figure').data[0].fill).toBeUndefined()
    expect(chart.props('figure').layout.yaxis.title).toBe('Net P&L (EUR)')

    await plotTabs[2].trigger('click')
    expect(chart.props('figure').data[0].y).toEqual([10_000, 10_025])
    expect(chart.props('figure').layout.yaxis.title).toBe('Equity (EUR)')

    await plotTabs[3].trigger('click')
    expect(chart.props('figure').data.map(trace => trace.name))
      .toEqual(['Gross exposure', 'Net exposure'])
    expect(chart.props('figure').data[0].y).toEqual([0, 110])
    expect(chart.props('figure').layout.yaxis.title).toBe('Exposure (EUR)')

    await plotTabs[4].trigger('click')
    expect(chart.props('figure').data[0].y).toEqual([0, -0.25])
    expect(chart.props('figure').layout.yaxis.title).toBe('Drawdown (%)')
    expect(wrapper.get('.live-metrics').text()).toContain('Equity')
    expect(wrapper.get('.live-portfolio-panel').text()).toContain('Positions & cash')
    expect(wrapper.get('.live-tables').text()).toContain('Recent orders')
    expect(wrapper.findAll('.live-tables > article')).toHaveLength(1)
    expect(wrapper.text()).toContain('Cancel orders')
    expect(wrapper.text()).toContain('Flatten')
    expect(wrapper.findAll('.action-popover').map(item => item.text())).toEqual([
      'Cancel every open simulated order before the next market update.',
      'Close all simulated positions on the next market update.'
    ])
    wrapper.unmount()
  })

  it('shows replay speed, warm-up restoration, and playback progress', async () => {
    api.mockResolvedValue({
      ...completedReplay,
      status: 'running',
      config: { ...completedReplay.config, playback_speed: 2 },
      replay: {
        speed: 2,
        processed_events: 25,
        total_events: 100,
        progress: 0.25,
        source_duration_seconds: 185,
        warmup_source: 'recorded',
        warmup_bars_loaded: 500
      }
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.replay-playback-panel').text()).toContain('2× playback')
    expect(wrapper.get('.replay-playback-panel').text())
      .toContain('Restored the original 500 warm-up bars.')
    expect(wrapper.get('.replay-progress').text())
      .toContain('25 / 100 events · 25.0% · 3m 5s recorded time')
    expect(wrapper.get('.replay-progress-track').attributes('aria-valuenow')).toBe('25')
    expect(wrapper.get('.replay-progress-track span').attributes('style')).toContain('25%')
    wrapper.unmount()
  })

  it('places base-currency units after monetary metrics when configured', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken', interval: '1m', symbols: ['BTC-USD'],
        strategies: ['Buy & Hold'],
        config: {
          initial_cash: 10_000,
          base_currency: 'EUR',
          metrics: ['total_return', 'pnl', 'final_equity', 'sharpe']
        }
      },
      snapshot: { equity: 9_984.2214, portfolio: { positions: {}, cash: { EUR: 9_984.2214 } } },
      strategies: {
        'Buy & Hold': {
          equity: 9_984.2214,
          metrics: {
            total_return: -0.0016,
            pnl: -15.7786,
            final_equity: 9_984.2214,
            sharpe: -46.1945
          },
          portfolio: { positions: {}, cash: { EUR: 9_984.2214 }, orders: [] }
        }
      },
      updates: [],
      error: null
    })
    const wrapper = mount(LivePage, {
      props: {
        bootstrap: {
          ...bootstrap,
          display: { ...bootstrap.display, currency_prefix: false },
          metrics: {
            builtin: [
              { key: 'total_return', name: 'Total return', percentage: true },
              { key: 'pnl', name: 'Profit and loss', percentage: false },
              { key: 'final_equity', name: 'Final equity', percentage: false },
              { key: 'sharpe', name: 'Sharpe ratio', percentage: false }
            ]
          }
        }
      }
    })
    await flushPromises()

    const values = wrapper.findAll('.live-metrics-table tbody td:last-child')
      .map(item => item.text())
    expect(values).toEqual(['-0.16%', '-15.78 €', '9,984.22 €', '-46.1945'])
    wrapper.unmount()
  })

  it('formats position quantities to at most eight decimal places', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'binance', interval: '1m', symbols: ['AAVE-ETH'], strategy: 'Buy & Hold',
        config: { initial_cash: 10_000, base_currency: 'EUR' }
      },
      snapshot: {
        equity: 10_000,
        portfolio: { positions: { 'AAVE-ETH': 132.63135111647918 }, cash: { EUR: 0 } }
      },
      updates: [],
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('.live-portfolio-panel').text()).toContain('132.63135112')
    expect(wrapper.get('.live-portfolio-panel').text()).not.toContain('132.63135111647918')
    expect(wrapper.get('.execution-panel').text()).toContain(
      'Older completed orders can leave this bounded list while positions remain open.'
    )
    wrapper.unmount()
  })

  it('shows dedicated recent order outcomes after their market updates age out', async () => {
    api.mockResolvedValue({
      status: 'running',
      config: {
        provider: 'kraken', interval: '1m', symbols: ['BTC-USD'],
        strategies: ['Buy & Hold'], config: { initial_cash: 10_000, base_currency: 'EUR' }
      },
      snapshot: { equity: 10_000, portfolio: { positions: { 'BTC-USD': 0.15 }, cash: { EUR: 500 } } },
      strategies: {
        'Buy & Hold': {
          equity: 10_000,
          portfolio: { positions: { 'BTC-USD': 0.15 }, cash: { EUR: 500 }, orders: [] }
        }
      },
      updates: Array.from({ length: 500 }, () => ({
        market: { symbol: 'BTC-USD', received_ts: 1_700_000_000 },
        strategies: { 'Buy & Hold': { fills: [], indicators: {}, snapshot: { equity: 10_000 } } }
      })),
      recent_order_outcomes: {
        'Buy & Hold': [{
          timestamp: 1_700_000_060,
          status: 'Filled',
          fill_price: 62_820.68,
          commission: 4.34,
          realized_pnl: -4.34,
          reason: 'live market fill',
          order: {
            id: 'buy-order', symbol: 'BTC-USD', quantity: 0.15, order_type: 'Market'
          }
        }, {
          timestamp: 1_700_000_120,
          status: 'Filled',
          fill_price: 62_900,
          commission: 0,
          realized_pnl: 12,
          order: {
            id: 'profit-order', symbol: 'BTC-USD', quantity: -0.05, order_type: 'Market'
          }
        }, {
          timestamp: 1_700_000_180,
          status: 'Filled',
          fill_price: 62_700,
          commission: 0,
          realized_pnl: -8,
          order: {
            id: 'loss-order', symbol: 'BTC-USD', quantity: -0.05, order_type: 'Market'
          }
        }]
      },
      error: null
    })
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    const execution = wrapper.get('.execution-panel')
    expect(wrapper.findAll('.live-metrics .metric-card small')).toHaveLength(0)
    expect(execution.findAll('th').map(column => column.text())).toEqual([
      'Time', 'Symbol', 'Side', 'Type', 'Quantity', 'Fill price', 'P&L', 'Commission',
      'Status'
    ])
    expect(execution.get('.execution-time').text()).not.toBe('—')
    expect(execution.get('.order-side').text()).toBe('Buy')
    expect(execution.get('.order-side').classes()).toContain('positive')
    expect(execution.get('tbody').text()).toContain('BTC-USD')
    expect(execution.get('tbody').text()).toContain('0.15')
    expect(execution.get('tbody').text()).toContain('Market')
    expect(execution.get('tbody').text()).toContain('62,820.68')
    expect(execution.get('tbody').text()).toContain('€4.34')
    expect(execution.get('tbody').text()).toContain('€0.00')
    const pnlCells = execution.findAll('tbody tr').map(row => row.findAll('td')[6])
    expect(pnlCells[0].classes()).not.toContain('positive')
    expect(pnlCells[0].classes()).not.toContain('negative')
    expect(pnlCells[1].classes()).toContain('positive')
    expect(pnlCells[2].classes()).toContain('negative')
    expect(wrapper.find('.execution-panel .empty-state').exists()).toBe(false)
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
    expect(wrapper.findAll('.quote-row > span small')).toHaveLength(0)
    expect(quotes[0].get('img').attributes('src')).toContain('img.logokit.com/crypto/BTC')
    expect(quotes[0].text()).toContain('60,000')
    expect(quotes[1].get('img').attributes('src')).toContain('img.logokit.com/crypto/ETH')
    expect(quotes[1].text()).toContain('Waiting for price')
    expect(wrapper.get('.live-metrics').text()).toContain('€10,000.00')

    const executions = wrapper.findAll('.live-execution-table tbody tr')
    expect(executions).toHaveLength(12)
    const status = executions[0].get('.badge')
    expect(status.classes()).toContain('error')
    expect(status.attributes('tabindex')).toBe('0')
    await status.trigger('mouseenter')
    await flushPromises()
    const tooltip = document.body.querySelector('.execution-status-tooltip')
    expect(status.attributes('aria-describedby')).toBe(tooltip?.id)
    expect(tooltip?.textContent.trim()).toBe('insufficient cash')
    expect(tooltip?.parentElement).toBe(document.body)
    expect(executions[0].find('.execution-reason').exists()).toBe(false)
    wrapper.unmount()
  })
})
