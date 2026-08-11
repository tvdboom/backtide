// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import LibraryPage from './library-page.vue'

const { api, post } = vi.hoisted(() => ({ api: vi.fn(), post: vi.fn() }))
vi.mock('../api', () => ({ api, post, put: vi.fn(), remove: vi.fn() }))
vi.mock('../components/python-editor.vue', () => ({
  default: { props: ['modelValue'], template: '<pre>{{ modelValue }}</pre>' }
}))

const catalog = {
  builtin: [{ key: 'sharpe', name: 'Sharpe ratio', type: 'sharpe', builtin: true, description: 'Risk adjusted.' }],
  saved: []
}

beforeEach(() => {
  location.hash = '#metrics'
  api.mockReset().mockResolvedValue(catalog)
  post.mockReset().mockResolvedValue({ saved: 'My metric' })
})

afterEach(() => { location.hash = '' })

it('shows Rust built-ins and opens a custom-only metric editor', async () => {
  const wrapper = mount(LibraryPage, {
    props: { bootstrap: { metrics: catalog, display: { dataframe_class: 'pd.DataFrame' } } }
  })
  await flushPromises()

  expect(wrapper.text()).toContain('Sharpe ratio')
  expect(wrapper.get('.library-card h3').text()).toBe('Sharpe ratio')
  expect(wrapper.find('.library-card .code-name').exists()).toBe(false)
  await wrapper.get('.page-intro .primary').trigger('click')
  expect(wrapper.text()).toContain('Python source')
  expect(wrapper.text()).toContain('BaseMetric')
  expect(wrapper.text()).toContain('return result')
  expect(wrapper.text()).not.toContain('Describe what this metric measures')
  expect(wrapper.find('.library-editor-mode').exists()).toBe(false)
})
