// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import LibraryPage from './library-page.vue'

const { api, post, put, remove } = vi.hoisted(() => ({
  api: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  remove: vi.fn()
}))

vi.mock('../api', () => ({ api, post, put, remove }))

const bootstrap = {
  strategies: {
    builtin: [{
      type: 'BuyAndHold',
      name: 'Buy and hold',
      parameters: [{ name: 'symbol', label: 'Symbol', kind: 'text', default: null, required: false }]
    }],
    saved: [{ name: 'Long term', type: 'BuyAndHold', builtin: true, description: 'Hold an asset.', params: { symbol: 'AAPL' } }]
  },
  indicators: {
    builtin: [{
      type: 'SMA',
      name: 'Simple moving average',
      parameters: [{ name: 'period', label: 'Period', kind: 'number', default: 14, required: false }]
    }],
    saved: [{ name: 'Fast SMA', type: 'SMA', builtin: true, description: 'Follow the trend.', params: { period: 8 } }]
  },
  sizers: {
    builtin: [{
      type: 'FixedFractional',
      name: 'Fixed Fractional',
      parameters: [{ name: 'fraction', label: 'Fraction', kind: 'number', default: 0.1, required: false }]
    }],
    saved: [{ name: 'Ten percent', type: 'FixedFractional', builtin: true, description: 'Use ten percent.', params: { fraction: 0.1 } }]
  },
  metrics: {
    builtin: [{ key: 'sharpe', name: 'Sharpe ratio', builtin: true, description: 'Risk-adjusted return.' }],
    saved: []
  }
}

describe('library page', () => {
  beforeEach(() => {
    api.mockReset()
    api.mockImplementation(async endpoint => {
      if (endpoint === '/api/strategies') return bootstrap.strategies
      if (endpoint === '/api/sizers') return bootstrap.sizers
      if (endpoint === '/api/metrics') return bootstrap.metrics
      return bootstrap.indicators
    })
    post.mockReset()
    put.mockReset()
    remove.mockReset()
  })

  it('refreshes strategies after rendering bootstrap data', async () => {
    location.hash = '#strategies'
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.text()).toContain('Long term')
    expect(wrapper.find('.loading-screen').exists()).toBe(false)
    expect(api).toHaveBeenCalledWith('/api/strategies')
    expect(wrapper.emitted('catalog-updated').at(-1)[0]).toEqual({
      key: 'strategies',
      catalog: bootstrap.strategies
    })
  })

  it.each(['strategies', 'indicators', 'sizers'])('labels %s editor options without implementation details', async page => {
    location.hash = `#${page}`
    const wrapper = mount(LibraryPage, {
      props: { bootstrap },
      global: { stubs: { PythonEditor: { template: '<div class="python-editor-stub" />' } } }
    })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')

    const labels = wrapper.findAll('.library-editor-mode button').map(button => button.text())
    expect(labels).toEqual(['Built-in', 'Custom'])
    expect(wrapper.get('.library-editor-mode').text()).not.toContain('Python')
    expect(wrapper.get('.library-editor input[required]').attributes('maxlength')).toBe('20')

    await wrapper.findAll('.library-editor-mode button')[1].trigger('click')
    const customName = wrapper.get('.custom-source-row input')
    expect(customName.attributes('maxlength')).toBe('20')
    expect(customName.attributes('required')).toBeUndefined()
    expect(customName.attributes('placeholder')).toBe('Defaults to Python class name')
  })

  it.each([
    ['strategies', 'Buy and hold'],
    ['indicators', 'Simple moving average'],
    ['sizers', 'Fixed Fractional']
  ])('uses the selected built-in %s class name as the default asset name', async (page, name) => {
    location.hash = `#${page}`
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')

    expect(wrapper.get('.library-editor input[required]').element.value).toBe(name)
  })

  it('updates the default name when another built-in class is selected', async () => {
    location.hash = '#strategies'
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.builtin.push({
      type: 'AdaptiveRsi',
      name: 'Adaptive RSI',
      parameters: []
    })
    api.mockResolvedValue(pageBootstrap.strategies)
    const wrapper = mount(LibraryPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')
    await wrapper.get('.library-editor select').setValue('AdaptiveRsi')

    expect(wrapper.get('.library-editor input[required]').element.value).toBe('Adaptive RSI')
  })

  it('uses the shared titled toggle field for boolean constructor options', async () => {
    location.hash = '#strategies'
    const pageBootstrap = structuredClone(bootstrap)
    pageBootstrap.strategies.builtin[0].parameters = [{
      name: 'enabled', label: 'Enabled', kind: 'boolean', default: true, required: false
    }]
    api.mockResolvedValue(pageBootstrap.strategies)
    const wrapper = mount(LibraryPage, { props: { bootstrap: pageBootstrap } })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')

    const field = wrapper.get('.toggle-label')
    expect(field.get('.toggle-title').text()).toBe('Enabled')
    expect(field.get('.field-info').exists()).toBe(true)
    expect(field.get('.toggle-description').text()).toBe('Turn this option on or off.')
    expect(field.get('.toggle').element.checked).toBe(true)
  })

  it.each([
    ['strategies', '/api/strategies', 'MomentumStrategy'],
    ['indicators', '/api/indicators', 'MomentumIndicator'],
    ['sizers', '/api/sizers', 'MomentumSizer'],
    ['metrics', '/api/metrics', 'MomentumMetric']
  ])('prefills a custom %s name from its Python class', async (page, endpoint, className) => {
    location.hash = `#${page}`
    post.mockResolvedValue({ saved: className })
    const wrapper = mount(LibraryPage, {
      props: { bootstrap },
      global: {
        stubs: {
          PythonEditor: {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<textarea class="python-editor-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          }
        }
      }
    })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')
    if (page !== 'metrics') {
      await wrapper.findAll('.library-editor-mode button')[1].trigger('click')
    }
    const name = wrapper.get('.custom-source-row input')
    expect(name.attributes('required')).toBeUndefined()
    expect(name.attributes('placeholder')).toBe('Defaults to Python class name')
    expect(name.element.value).toBe('')

    const code = `class ${className}:\n    pass\n\n${className}()`
    await wrapper.get('.python-editor-stub').setValue(code)
    await flushPromises()
    expect(name.element.value).toBe(className)

    await wrapper.get('.library-editor').trigger('submit')
    await flushPromises()

    expect(post).toHaveBeenCalledWith(endpoint, expect.objectContaining({
      kind: 'custom',
      name: className,
      code
    }))
  })

  it.each([
    ['strategies', 'MyMomentumStrategy'],
    ['indicators', 'myCustomIndicator'],
    ['sizers', 'MyPositionSizer'],
    ['metrics', 'MyCustomMetric']
  ])('leaves the custom %s name empty for a MyXxx class', async (page, className) => {
    location.hash = `#${page}`
    const wrapper = mount(LibraryPage, {
      props: { bootstrap },
      global: {
        stubs: {
          PythonEditor: {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<textarea class="python-editor-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          }
        }
      }
    })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')
    if (page !== 'metrics') {
      await wrapper.findAll('.library-editor-mode button')[1].trigger('click')
    }
    await wrapper.get('.python-editor-stub').setValue(
      `class ${className}:\n    pass\n\n${className}()`
    )
    await flushPromises()

    expect(wrapper.get('.custom-source-row input').element.value).toBe('')
  })

  it('keeps an inferred name in sync until the user enters a custom name', async () => {
    location.hash = '#strategies'
    const wrapper = mount(LibraryPage, {
      props: { bootstrap },
      global: {
        stubs: {
          PythonEditor: {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<textarea class="python-editor-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          }
        }
      }
    })
    await flushPromises()

    await wrapper.get('.page-intro .primary').trigger('click')
    await wrapper.findAll('.library-editor-mode button')[1].trigger('click')
    const code = wrapper.get('.python-editor-stub')
    const name = wrapper.get('.custom-source-row input')

    await code.setValue('class M:\n    pass\n\nM()')
    await flushPromises()
    expect(name.element.value).toBe('M')

    await code.setValue('class MomentumStrategy:\n    pass\n\nMomentumStrategy()')
    await flushPromises()
    expect(name.element.value).toBe('MomentumStrategy')

    await name.setValue('Manual name')
    await code.setValue('class RevisedStrategy:\n    pass\n\nRevisedStrategy()')
    await flushPromises()
    expect(name.element.value).toBe('Manual name')

    await name.setValue('')
    await flushPromises()
    expect(name.element.value).toBe('RevisedStrategy')
  })

  it('refreshes indicators after rendering bootstrap data', async () => {
    location.hash = '#indicators'
    api.mockResolvedValue({
      ...bootstrap.indicators,
      saved: [
        ...bootstrap.indicators.saved,
        { name: 'ind11', type: 'BollingerBands', builtin: true, description: 'Freshly saved.' }
      ]
    })
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.text()).toContain('Fast SMA')
    expect(wrapper.text()).toContain('ind11')
    expect(wrapper.find('.loading-screen').exists()).toBe(false)
    expect(api).toHaveBeenCalledWith('/api/indicators')
  })

  it('manages sizers as a first-class library asset', async () => {
    location.hash = '#sizers'
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    expect(wrapper.get('h2').text()).toBe('Sizers')
    expect(wrapper.text()).toContain('Ten percent')
    expect(api).toHaveBeenCalledWith('/api/sizers')

    await wrapper.get('.primary').trigger('click')
    expect(wrapper.get('.library-editor').text()).toContain('Fixed Fractional')
    expect(wrapper.get('.library-editor input[required]').element.value).toBe('Fixed Fractional')
    expect(wrapper.get('.library-editor input[type="number"]').element.value).toBe('0.1')
    expect(wrapper.text()).toContain('custom strategy order logic')
  })

  it.each(['strategies', 'indicators'])('distinguishes an empty %s library from no search matches', async page => {
    location.hash = `#${page}`
    const title = page[0].toUpperCase() + page.slice(1)
    const emptyBootstrap = {
      ...bootstrap,
      [page]: { ...bootstrap[page], saved: [] }
    }
    api.mockResolvedValue(emptyBootstrap[page])
    const emptyWrapper = mount(LibraryPage, { props: { bootstrap: emptyBootstrap } })
    await flushPromises()

    expect(emptyWrapper.text()).toContain(`No saved ${page}`)
    expect(emptyWrapper.text()).not.toContain(`No matching ${page}`)
    emptyWrapper.unmount()

    api.mockResolvedValue(bootstrap[page])
    const filteredWrapper = mount(LibraryPage, { props: { bootstrap } })
    await filteredWrapper.get('.search-box input').setValue(`Missing ${title}`)

    expect(filteredWrapper.text()).toContain(`No matching ${page}`)
    expect(filteredWrapper.text()).toContain('Try changing your search or filter.')
    expect(filteredWrapper.text()).not.toContain(`No saved ${page}`)
  })

  it('releases the loading state and offers a retry after an API error', async () => {
    location.hash = '#strategies'
    api.mockRejectedValueOnce(new Error('Catalog unavailable'))
    const wrapper = mount(LibraryPage, {
      props: { bootstrap: { strategies: null, indicators: null } }
    })
    await flushPromises()

    expect(wrapper.find('.loading-screen').exists()).toBe(false)
    expect(wrapper.get('.error-state').text()).toContain('Catalog unavailable')

    api.mockResolvedValueOnce(bootstrap.strategies)
    await wrapper.get('.error-state button').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Long term')
    expect(wrapper.find('.error-state').exists()).toBe(false)
  })

  it('asks in-page before deleting a saved strategy', async () => {
    location.hash = '#strategies'
    api.mockResolvedValue(bootstrap.strategies)
    remove.mockResolvedValue(undefined)
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get('[aria-label="Delete Long term"]').trigger('click')

    expect(wrapper.get('[role="alertdialog"]').text()).toContain('Delete Long term?')
    expect(remove).not.toHaveBeenCalled()

    await wrapper.get('.confirm-submit').trigger('click')
    await flushPromises()

    expect(remove).toHaveBeenCalledWith('/api/strategies/Long%20term')
  })

  it.each([
    ['strategies', 'from backtide.strategies import BaseStrategy', 'MyStrategy()'],
    ['indicators', 'from backtide.indicators import BaseIndicator', 'MyIndicator()']
  ])('prefills the legacy custom Python starter for %s', async (page, importLine, instanceLine) => {
    location.hash = `#${page}`
    const wrapper = mount(LibraryPage, {
      props: {
        bootstrap: { ...bootstrap, display: { dataframe_class: 'pl.DataFrame' } }
      },
      global: {
        stubs: {
          PythonEditor: {
            props: ['modelValue'],
            template: '<textarea class="python-editor-stub" :value="modelValue" />'
          }
        }
      }
    })

    await wrapper.get('.page-intro .primary').trigger('click')
    await wrapper.findAll('.modal > .segmented button')[1].trigger('click')

    const source = wrapper.get('.python-editor-stub').element.value
    expect(source).toContain(importLine)
    expect(source).toContain('data : pl.DataFrame')
    expect(source).toContain(instanceLine)
    expect(wrapper.get('.custom-source-row').find('input[required]').exists()).toBe(false)
    expect(wrapper.get('.upload-code-button').text()).toBe('Load Python file')
  })

  it.each([
    ['strategies', 'Strategy', '/api/strategies', 'Long term', 'BuyAndHold', 'AAPL', 'MSFT', 'Core holding'],
    ['indicators', 'Indicator', '/api/indicators', 'Fast SMA', 'SMA', 8, 21, 'Medium SMA']
  ])('prefills and replaces every field of a saved built-in %s', async (
    page,
    singular,
    endpoint,
    originalName,
    type,
    originalValue,
    updatedValue,
    updatedName
  ) => {
    location.hash = `#${page}`
    api.mockResolvedValue(bootstrap[page])
    put.mockResolvedValue({ saved: updatedName })
    const wrapper = mount(LibraryPage, { props: { bootstrap } })
    await flushPromises()

    await wrapper.get(`[aria-label="Edit ${originalName}"]`).trigger('click')
    const modal = wrapper.get('.modal')
    const configurationInput = modal.findAll('.form-grid input')[1]
    expect(modal.text()).toContain(`Edit ${singular}`)
    expect(modal.findAll('.segmented button')).toHaveLength(2)
    expect(modal.get('select').element.value).toBe(type)
    expect(configurationInput.element.value).toBe(String(originalValue))

    await modal.get('input[required]').setValue(updatedName)
    await configurationInput.setValue(updatedValue)
    await modal.trigger('submit')
    await flushPromises()

    const parameterName = page === 'strategies' ? 'symbol' : 'period'
    expect(put).toHaveBeenCalledWith(`${endpoint}/${encodeURIComponent(originalName)}`, {
      kind: 'builtin',
      name: updatedName,
      type,
      code: '',
      params: { [parameterName]: updatedValue }
    })
  })

  it.each([
    ['strategies', 'Strategy', '/api/strategies', 'Custom momentum'],
    ['indicators', 'Indicator', '/api/indicators', 'Custom signal']
  ])('edits the saved name and Python source for %s', async (page, singular, endpoint, name) => {
    location.hash = `#${page}`
    const source = `class Original${singular}:\n    pass\n`
    const updatedSource = `class Updated${singular}:\n    pass\n`
    const pageCatalog = {
      ...bootstrap[page],
      saved: [{ name, type: `Original${singular}`, builtin: false, description: 'Custom.', source }]
    }
    api.mockResolvedValue(pageCatalog)
    put.mockResolvedValue({ saved: `Updated ${singular}` })
    const wrapper = mount(LibraryPage, {
      props: { bootstrap: { ...bootstrap, [page]: pageCatalog } },
      global: {
        stubs: {
          PythonEditor: {
            props: ['modelValue', 'readonly'],
            emits: ['update:modelValue'],
            template: '<textarea class="python-editor-stub" :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          }
        }
      }
    })
    await flushPromises()

    expect(wrapper.find('.library-card details').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Source code')

    await wrapper.get(`[aria-label="Edit ${name}"]`).trigger('click')
    const modal = wrapper.get('.modal')
    expect(modal.text()).toContain(`Edit ${singular}`)
    expect(modal.get('.python-editor-stub').element.value).toBe(source)

    await modal.get('.custom-source-row input').setValue(`Updated ${singular}`)
    await modal.get('.python-editor-stub').setValue(updatedSource)
    await modal.trigger('submit')
    await flushPromises()

    expect(put).toHaveBeenCalledWith(`${endpoint}/${encodeURIComponent(name)}`, {
      kind: 'custom',
      name: `Updated ${singular}`,
      type: `Original${singular}`,
      params: {},
      code: updatedSource
    })
  })
})
