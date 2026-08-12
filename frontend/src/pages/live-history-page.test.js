// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import LiveHistoryPage from './live-history-page.vue'

const { api, post } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn()
}))

vi.mock('../api', () => ({ api, post }))

describe('live session history page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue([{
      id: 'session-1',
      status: 'stopped',
      started_at: '2026-08-12T12:00:00Z',
      finished_at: '2026-08-12T12:45:00Z',
      config: {
        mode: 'paper',
        provider: 'kraken',
        interval: '1m',
        symbols: ['BTC-USD'],
        strategies: ['Momentum'],
        indicators: ['RSI'],
        warmup_bars: 500,
        config: { base_currency: 'USD', initial_cash: 10000 }
      },
      snapshot: { equity: 101250 }
    }])
    post.mockReset().mockResolvedValue({ status: 'running' })
  })

  it('lists persisted sessions and starts an exact replay', async () => {
    const wrapper = mount(LiveHistoryPage, {
      props: { bootstrap: {} }
    })
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/live/sessions')
    expect(wrapper.text()).toContain('Momentum')
    expect(wrapper.text()).toContain('$101,250.00')
    expect(wrapper.text()).toContain('Live paper')
    expect(wrapper.findAll('th').map(header => header.text())).toEqual([
      'Started',
      'Finished',
      'Mode',
      'Strategies',
      'Status',
      'Final equity',
      ''
    ])
    expect(wrapper.findAll('tbody td')[1].text()).not.toBe('—')

    expect(wrapper.findAll('.compact-button').map(button => button.text())).toEqual([
      'Replay', 'Go live'
    ])

    await wrapper.findAll('.compact-button')[0].trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/live/replay', { session_id: 'session-1' })
    expect(wrapper.emitted('live-status')[0]).toEqual([{ status: 'running' }])
    expect(wrapper.emitted('navigate')[0]).toEqual(['live'])
  })

  it('starts a new provider session directly from the saved settings', async () => {
    const wrapper = mount(LiveHistoryPage, { props: { bootstrap: {} } })
    await flushPromises()

    await wrapper.findAll('.compact-button')[1].trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/live', {
      provider: 'kraken',
      interval: '1m',
      symbols: ['BTC-USD'],
      strategies: ['Momentum'],
      indicators: ['RSI'],
      warmup_bars: 500,
      config: { base_currency: 'USD', initial_cash: 10000 }
    })
    expect(wrapper.emitted('live-status')[0]).toEqual([{ status: 'running' }])
    expect(wrapper.emitted('toast')[0]).toEqual([
      'Live session started from saved settings.'
    ])
    expect(wrapper.emitted('navigate')[0]).toEqual(['live'])
  })
})
