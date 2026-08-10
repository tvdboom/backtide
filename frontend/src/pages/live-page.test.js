// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import LivePage from './live-page.vue'

const { api, post } = vi.hoisted(() => ({ api: vi.fn(), post: vi.fn() }))

vi.mock('../api', () => ({ api, post }))

const supported = reason => ({ supported: true, reason })
const unsupported = reason => ({ supported: false, reason })
const bootstrap = {
  enums: { intervals: ['1m', '5m'] },
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

describe('live page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue({
      status: 'idle', config: {}, snapshot: {}, updates: [], error: null
    })
    post.mockReset()
  })

  it('uses a valid and editable initial cash default', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()
    const input = wrapper.get('#live-initial-cash').element

    expect(input.value).toBe('100000')
    expect(input.min).toBe('0')
    expect(input.step).toBe('0.01')
    expect(input.validity.valid).toBe(true)
  })

  it('prevents choosing providers without a WebSocket for the selected interval', async () => {
    const wrapper = mount(LivePage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('option[value="yahoo"]').element.disabled).toBe(true)
    expect(wrapper.get('option[value="coinbase"]').element.disabled).toBe(true)
    expect(wrapper.get('option[value="kraken"]').element.disabled).toBe(false)
  })
})
