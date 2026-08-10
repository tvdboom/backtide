// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ResultsPage from './results-page.vue'

const { api, post, query, remove } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  query: vi.fn(),
  remove: vi.fn()
}))

vi.mock('../api', () => ({ api, post, query, remove }))
vi.mock('../components/chart-panel.vue', () => ({
  default: { template: '<div class="chart-stub" />' }
}))

const detail = {
  experiment: {
    id: 'experiment-1',
    name: 'Momentum study',
    status: 'Success',
    tags: []
  },
  config: '[general]\nname = "Momentum study"',
  logs: '',
  runs: [{
    strategy_id: 'strategy-1',
    strategy_name: 'Momentum',
    is_benchmark: false,
    metrics: { total_return: 0.12 },
    trades: [{ symbol: 'AAPL' }],
    orders: []
  }]
}

describe('results page', () => {
  let wrapper

  beforeEach(() => {
    sessionStorage.clear()
    query.mockReset().mockResolvedValue([{ id: 'experiment-1', name: 'Momentum study' }])
    api.mockReset().mockImplementation(path => path === '/api/jobs' ? [] : detail)
    post.mockReset().mockImplementation((path) => {
      if (path === '/api/config/parse') return { general: { name: 'Momentum study' } }
      return { data: [], layout: {} }
    })
    remove.mockReset()
  })

  afterEach(() => wrapper?.unmount())

  it('separates experiment plots from per-strategy results', async () => {
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    expect(wrapper.text()).toContain('Experiment overview')
    expect(wrapper.text()).toContain('Strategies')
    expect(wrapper.get('.result-workspace').text()).toContain('Rolling Sharpe')
    expect(wrapper.findAll('.result-workspace')[1].text()).toContain('Trades on price')
    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'pnl' }))
    expect(post).toHaveBeenCalledWith('/api/results/plot', expect.objectContaining({ plot: 'mae_mfe' }))
  })

  it('opens a new experiment with the saved configuration', async () => {
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    const [reuseButton, configButton] = wrapper.findAll('.result-actions .secondary')
    expect(reuseButton.text()).toContain('Reuse setup')
    expect(reuseButton.classes()).toEqual(configButton.classes())
    await reuseButton.trigger('click')
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/api/config/parse', {
      suffix: '.toml',
      text: detail.config
    })
    expect(JSON.parse(sessionStorage.getItem('backtide:experiment-config'))).toEqual({
      general: { name: 'Momentum study' }
    })
    expect(wrapper.emitted('navigate')).toContainEqual(['experiment'])
  })

  it('places status and tags below the title and omits an empty description', async () => {
    api.mockImplementation(path => path === '/api/jobs' ? [] : {
      ...detail,
      experiment: { ...detail.experiment, description: '   ', tags: ['momentum', 'daily'] }
    })

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    const summary = wrapper.get('.result-heading > div')
    expect(summary.element.children[0].tagName).toBe('H2')
    expect(summary.element.children[1].classList).toContain('result-title')
    expect(summary.get('.result-title').text()).toContain('Success')
    expect(summary.get('.result-title').text()).toContain('momentum')
    expect(summary.find('p').exists()).toBe(false)
    expect(summary.text()).not.toContain('No description was provided.')
  })

  it('shows experiment details while plots are still loading', async () => {
    const resolvePlots = []
    post.mockImplementation(path => {
      if (path === '/api/config/parse') return { general: { name: 'Momentum study' } }
      return new Promise(resolve => { resolvePlots.push(resolve) })
    })

    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()

    expect(wrapper.text()).toContain('Momentum study')
    expect(wrapper.text()).not.toContain('Loading results…')

    resolvePlots.forEach(resolve => resolve({ data: [], layout: {} }))
    await flushPromises()
  })

  it('asks in-page before deleting an experiment', async () => {
    wrapper = mount(ResultsPage, { props: { bootstrap: {} } })
    await flushPromises()
    await flushPromises()

    await wrapper.get('[aria-label="Delete experiment"]').trigger('click')

    expect(wrapper.get('[role="alertdialog"]').text()).toContain('Delete Momentum study?')
    expect(remove).not.toHaveBeenCalled()

    await wrapper.get('.confirm-submit').trigger('click')
    await flushPromises()

    expect(remove).toHaveBeenCalledWith('/api/experiments/experiment-1')
  })
})
