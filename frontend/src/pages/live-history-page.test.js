// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import LiveHistoryPage from './live-history-page.vue'

const { api } = vi.hoisted(() => ({ api: vi.fn() }))

vi.mock('../api', () => ({ api }))

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
  })

  it('lists persisted sessions without restart or replay actions', async () => {
    const wrapper = mount(LiveHistoryPage, {
      props: { bootstrap: {} }
    })
    await flushPromises()

    expect(api).toHaveBeenCalledWith('/api/live/sessions')
    expect(wrapper.text()).toContain('Momentum')
    expect(wrapper.text()).toContain('$101,250.00')
    expect(wrapper.findAll('th').map(header => header.text())).toEqual([
      'Started',
      'Finished',
      'Strategies',
      'Status',
      'Final equity',
      ''
    ])
    expect(wrapper.findAll('tbody td')[1].text()).not.toBe('—')

    expect(wrapper.findAll('.compact-button')).toHaveLength(0)
    expect(wrapper.text()).not.toContain('Go live')
  })

  it('summarizes sessions with more than two strategies', async () => {
    api.mockResolvedValue([{
      id: 'multi-strategy-session',
      status: 'stopped',
      started_at: '2026-08-12T12:00:00Z',
      finished_at: '2026-08-12T12:45:00Z',
      config: {
        mode: 'paper',
        strategies: ['BB Mean Reversion', 'Buy & Hold', 'SMA (Naive)', 'RSI Breakout'],
        config: { base_currency: 'USD' }
      },
      snapshot: { equity: 10000 }
    }])
    const wrapper = mount(LiveHistoryPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.get('.session-strategy-visible').text())
      .toBe('BB Mean Reversion, Buy & Hold')
    expect(wrapper.get('.session-strategy-overflow').text()).toBe('+2')
  })

  it('offers Open only for the active session', async () => {
    api.mockResolvedValue([
      {
        id: 'active-session',
        status: 'running',
        started_at: '2026-08-13T12:00:00Z',
        finished_at: null,
        config: {
          mode: 'paper',
          symbols: ['BTC-USD'],
          strategies: ['Momentum'],
          config: { base_currency: 'USD' }
        },
        snapshot: { equity: 10100 }
      },
      {
        id: 'stopped-session',
        status: 'stopped',
        started_at: '2026-08-12T12:00:00Z',
        finished_at: '2026-08-12T12:45:00Z',
        config: {
          mode: 'paper',
          symbols: ['ETH-USD'],
          strategies: ['Momentum'],
          config: { base_currency: 'USD' }
        },
        snapshot: { equity: 10000 }
      }
    ])
    const wrapper = mount(LiveHistoryPage, { props: { bootstrap: {} } })
    await flushPromises()

    const actions = wrapper.findAll('.compact-button')
    expect(actions).toHaveLength(1)
    expect(actions[0].text()).toBe('Open')
    expect(wrapper.get('.badge.running').text()).toBe('running')
    expect(wrapper.text()).not.toContain('Go live')

    await actions[0].trigger('click')

    expect(wrapper.emitted('navigate')[0]).toEqual(['live'])
  })

  it('groups every replay beneath its original session', async () => {
    api.mockResolvedValue([
      {
        id: 'replay-2',
        status: 'stopped',
        started_at: '2026-08-12T14:00:00Z',
        finished_at: '2026-08-12T14:00:01Z',
        config: {
          mode: 'replay',
          source_session_id: 'replay-1',
          strategies: ['Momentum'],
          config: { base_currency: 'USD' }
        },
        snapshot: { equity: 10200 }
      },
      {
        id: 'replay-1',
        status: 'stopped',
        started_at: '2026-08-12T13:00:00Z',
        finished_at: '2026-08-12T13:00:01Z',
        config: {
          mode: 'replay',
          source_session_id: 'session-1',
          strategies: ['Momentum'],
          config: { base_currency: 'USD' }
        },
        snapshot: { equity: 10100 }
      },
      {
        id: 'session-1',
        status: 'stopped',
        started_at: '2026-08-12T12:00:00Z',
        finished_at: '2026-08-12T12:45:00Z',
        config: {
          mode: 'paper',
          strategies: ['Momentum'],
          config: { base_currency: 'USD' }
        },
        snapshot: { equity: 10050 }
      }
    ])
    const wrapper = mount(LiveHistoryPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.findAll('.session-history-row')).toHaveLength(1)
    expect(wrapper.findAll('.session-replay-row')).toHaveLength(0)
    const toggle = wrapper.get('.session-replay-toggle')
    expect(toggle.text()).toContain('2 replays')
    expect(toggle.attributes('aria-expanded')).toBe('false')

    await toggle.trigger('click')

    expect(toggle.attributes('aria-expanded')).toBe('true')
    expect(wrapper.findAll('.session-replay-row')).toHaveLength(2)
    expect(wrapper.findAll('.session-replay-start small').map(label => label.text()))
      .toEqual(['Replay', 'Replay'])
    expect(wrapper.findAll('.session-replay-row')[0].text()).toContain('$10,200.00')

    await toggle.trigger('click')
    expect(wrapper.findAll('.session-replay-row')).toHaveLength(0)
  })

  it('shows no final equity for monitoring-only sessions', async () => {
    api.mockResolvedValue([{
      id: 'monitor-session',
      status: 'stopped',
      started_at: '2026-08-12T12:00:00Z',
      finished_at: '2026-08-12T12:45:00Z',
      config: {
        mode: 'paper',
        strategies: [],
        config: { base_currency: 'USD', initial_cash: 10000 }
      },
      snapshot: { equity: 10000 }
    }])
    const wrapper = mount(LiveHistoryPage, { props: { bootstrap: {} } })
    await flushPromises()

    const cells = wrapper.findAll('tbody td')
    expect(cells[2].text()).toBe('Monitor only')
    expect(cells[4].text()).toBe('—')
  })

})
