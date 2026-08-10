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
      experiments: [{ id: 'experiment-1', name: 'Momentum study', status: 'Completed' }],
      metrics: {},
      storage: []
    })
    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })
    await flushPromises()

    await wrapper.get('.activity-row').trigger('click')

    expect(sessionStorage.getItem('backtide:result-id')).toBe('experiment-1')
    expect(wrapper.emitted('navigate')).toEqual([['results']])
  })

  it('does not report an empty database while dashboard data is loading', async () => {
    let resolveDashboard
    api.mockReturnValueOnce(new Promise(resolve => { resolveDashboard = resolve }))

    const wrapper = mount(DashboardPage, {
      props: { bootstrap: { display: { logokit_api_key: null } } }
    })

    expect(wrapper.text()).toContain('Loading stored market data…')
    expect(wrapper.text()).not.toContain('Your local database is empty.')

    resolveDashboard({ experiments: [], metrics: {}, storage: [] })
    await flushPromises()

    expect(wrapper.text()).toContain('Your local database is empty.')
  })
})
