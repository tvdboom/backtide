// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import DashboardPage from './dashboard-page.vue'

const { api } = vi.hoisted(() => ({ api: vi.fn() }))

vi.mock('../api', () => ({ api }))

describe('dashboard page', () => {
  beforeEach(() => {
    api.mockReset()
    sessionStorage.clear()
  })

  it('opens a recent experiment in the results page', async () => {
    api.mockResolvedValue({
      experiments: [{
        id: 'experiment-1', name: 'Momentum study', icon: '🎯', status: 'Completed',
        primary_metric_name: 'CAGR', primary_metric_value: 0.137, primary_metric_percentage: true
      }],
      metrics: {},
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })
    await flushPromises()

    expect(wrapper.get('.experiment-avatar').text()).toBe('🎯')
    expect(wrapper.get('.primary-metric-value small').text()).toBe('CAGR')
    expect(wrapper.get('.primary-metric-value strong').text()).toBe('13.70%')
    expect(wrapper.get('.primary-metric-value strong').classes()).toContain('positive')

    await wrapper.get('.activity-row').trigger('click')

    expect(sessionStorage.getItem('backtide:result-id')).toBe('experiment-1')
    expect(wrapper.emitted('navigate')).toEqual([['results']])
  })

  it('uses the results-page tone rules for recent primary metrics', async () => {
    api.mockResolvedValue({
      experiments: [
        { id: 'positive', name: 'Positive', primary_metric_name: 'Alpha', primary_metric_value: 1.2 },
        { id: 'negative', name: 'Negative', primary_metric_name: 'Alpha', primary_metric_value: -0.4 },
        { id: 'neutral', name: 'Neutral', primary_metric_name: 'Alpha', primary_metric_value: 0 }
      ],
      metrics: {},
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })
    await flushPromises()

    const values = wrapper.findAll('.primary-metric-value strong')
    expect(values[0].classes()).toContain('positive')
    expect(values[1].classes()).toContain('negative')
    expect(values[2].classes()).not.toContain('positive')
    expect(values[2].classes()).not.toContain('negative')
  })

  it('uses the configured date and time format for recent experiments', async () => {
    api.mockResolvedValue({
      experiments: [{
        id: 'dated', name: 'Dated study', best_sharpe: 1,
        started_at: Date.UTC(2026, 7, 11, 19, 5) / 1000
      }],
      metrics: {},
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: {
        bootstrap: {
          display: {
            date_format: 'DD-MM-YYYY',
            datetime_format: 'DD-MM-YYYY HH:MM',
            timezone: 'UTC'
          }
        }
      }
    })
    await flushPromises()

    expect(wrapper.get('.activity-row small').text()).toBe('11-08-2026 19:05')
  })

  it('opens recent stored data in analysis and renders its price sparkline', async () => {
    api.mockResolvedValue({
      experiments: [],
      metrics: {},
      storage: [{
        symbol: 'AAPL', interval: '1d', provider: 'yahoo', n_rows: 1000,
        sparkline: [180, 182, 179, 185]
      }]
    })
    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })
    await flushPromises()

    expect(wrapper.get('.market-sparkline polyline').attributes('points')).not.toBe('')
    expect(wrapper.get('.market-row').text()).not.toContain('1,000')
    await wrapper.get('.market-row').trigger('click')

    expect(sessionStorage.getItem('backtide:analysis-symbols')).toBe('["AAPL"]')
    expect(wrapper.emitted('navigate')).toEqual([['analysis']])
  })

  it('shows the stored session count and opens recent live sessions in history', async () => {
    api.mockResolvedValue({
      experiments: [],
      metrics: { sessions: 12 },
      sessions: [{
        id: 'session-1',
        status: 'stopped',
        started_at: '2026-08-12T12:00:00Z',
        config: {
          mode: 'paper',
          symbols: ['BTC-USD'],
          strategies: ['Momentum'],
          config: { base_currency: 'EUR', initial_cash: 10000 }
        },
        snapshot: { equity: 10125 }
      }],
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: {
        bootstrap: {
          display: {
            date_format: 'DD-MM-YYYY',
            datetime_format: 'DD-MM-YYYY HH:MM',
            timezone: 'UTC',
            currency_prefix: false,
            logokit_api_key: null
          }
        }
      }
    })
    await flushPromises()

    const sessionMetric = wrapper.findAll('.metric-card')
      .find(card => card.text().includes('Live sessions'))
    expect(sessionMetric.text()).toContain('12')
    expect(wrapper.get('.live-session-row').text()).toContain('BTC-USD')
    expect(wrapper.get('.live-session-row').text()).toContain('Momentum')
    expect(wrapper.findAll('.live-session-row .primary-metric-value small').map(item => item.text()))
      .toEqual(['Starting equity', 'Final P&L'])
    expect(wrapper.findAll('.live-session-row .primary-metric-value strong').map(item => item.text()))
      .toEqual(['10,000.00 €', '125.00 €'])
    expect(wrapper.get('.live-session-row').text()).toContain('12-08-2026 12:00')

    await wrapper.get('.live-session-row').trigger('click')

    expect(wrapper.emitted('navigate')).toEqual([['live-history']])
  })

  it('opens a running live session on the live page', async () => {
    api.mockResolvedValue({
      experiments: [],
      metrics: {},
      sessions: [{ id: 'session-1', status: 'running', config: {} }],
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })
    await flushPromises()

    await wrapper.get('.live-session-row').trigger('click')

    expect(wrapper.emitted('navigate')).toEqual([['live']])
  })

  it('does not report an empty database while dashboard data is loading', async () => {
    let resolveDashboard
    api.mockReturnValueOnce(new Promise(resolve => { resolveDashboard = resolve }))

    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })

    expect(wrapper.text()).toContain('Loading stored market data…')
    expect(wrapper.text()).toContain('Loading recent live sessions…')
    expect(wrapper.text()).not.toContain('Your local database is empty.')

    resolveDashboard({ experiments: [], metrics: {}, sessions: [], storage: [] })
    await flushPromises()

    expect(wrapper.text()).toContain('Your local database is empty.')
    expect(wrapper.text()).toContain('No live sessions yet.')
  })
})
