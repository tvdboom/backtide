// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { BrainCircuit } from 'lucide-vue-next'
import { describe, expect, it } from 'vitest'
import SearchSelect from './search-select.vue'

describe('search-select', () => {
  it('filters options and emits a selected value', async () => {
    const wrapper = mount(SearchSelect, {
      props: { modelValue: [], options: ['AAPL', 'MSFT', 'AMZN'], label: 'Symbols' }
    })
    const input = wrapper.get('input')

    await input.trigger('focus')
    await input.setValue('MS')
    await wrapper.get('.search-menu button').trigger('click')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual([['MSFT']])
  })

  it('removes selected tags', async () => {
    const wrapper = mount(SearchSelect, {
      props: { modelValue: ['AAPL'], options: [], allowCustom: true }
    })

    await wrapper.get('.tag button').trigger('click')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual([[]])
  })

  it('clears every selected value from the compact selector action', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['AAPL', 'MSFT'],
        options: ['AAPL', 'MSFT'],
        clearable: true,
        clearLabel: 'symbols'
      }
    })

    await wrapper.get('[aria-label="Clear all symbols"]').trigger('click')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual([[]])
  })

  it('reorders selected tags by dragging when enabled', async () => {
    let wrapper
    wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['Sharpe', 'Return', 'PNL'],
        options: [],
        reorderable: true,
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })
    const dataTransfer = { effectAllowed: '', setData: () => {} }

    await wrapper.findAll('.tag')[0].trigger('dragstart', { dataTransfer })
    const target = wrapper.findAll('.tag')[2]
    target.element.getBoundingClientRect = () => ({ left: 50, width: 50 })
    await target.trigger('dragover', { clientX: 80 })

    expect(dataTransfer.effectAllowed).toBe('move')
    expect(wrapper.emitted('update:modelValue').at(-1)).toEqual([['Return', 'PNL', 'Sharpe']])
  })

  it('shows an available instrument logo in the selected tag', () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['ASML'],
        options: ['ASML'],
        logos: { ASML: 'https://example.test/asml.png' }
      }
    })

    expect(wrapper.get('.tag img').attributes('src')).toBe('https://example.test/asml.png')
    expect(wrapper.get('.tag img').attributes('fetchpriority')).toBe('high')
    expect(wrapper.get('.tag img').attributes('loading')).toBe('eager')
    expect(wrapper.get('.tag').text()).toContain('ASML')
  })

  it('turns each entered custom tag into a case-preserved pill', async () => {
    let wrapper
    wrapper = mount(SearchSelect, {
      props: {
        modelValue: [],
        options: [],
        allowCustom: true,
        uppercaseCustom: false,
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })
    const input = wrapper.get('input')

    await input.setValue('Momentum research')
    await input.trigger('keydown.enter')
    await wrapper.vm.$nextTick()
    await input.setValue('Q3')
    await input.trigger('keydown.enter')
    await wrapper.vm.$nextTick()

    expect(wrapper.findAll('.tag').map(tag => tag.text().replace('×', '').trim())).toEqual([
      'Momentum research',
      'Q3'
    ])
    expect(input.element.value).toBe('')
  })

  it('searches instrument names and renders its logo, name, and symbol', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: [],
        options: ['AAPL', 'MSFT'],
        descriptions: { AAPL: 'Apple Inc.', MSFT: 'Microsoft Corporation' },
        logos: { MSFT: 'https://example.test/msft.png' }
      }
    })

    await wrapper.get('input').trigger('focus')
    await wrapper.get('input').setValue('Microsoft')

    const option = wrapper.get('.search-menu button')
    expect(option.get('img').attributes('src')).toBe('https://example.test/msft.png')
    expect(option.get('strong').text()).toBe('Microsoft Corporation')
    expect(option.get('small').text()).toBe('MSFT')
  })

  it('renders plain options without a logo or secondary label', async () => {
    const wrapper = mount(SearchSelect, {
      props: { modelValue: [], options: ['1m', '1d'], plainOptions: true }
    })

    await wrapper.get('input').trigger('focus')

    const options = wrapper.findAll('.search-menu button')
    expect(options.map(option => option.text())).toEqual(['1m', '1d'])
    expect(wrapper.find('.search-option-logo').exists()).toBe(false)
    expect(wrapper.find('.search-option-copy').exists()).toBe(false)
  })

  it('shows an explicit loading state while options are being retrieved', async () => {
    const wrapper = mount(SearchSelect, {
      props: { modelValue: [], options: [], loading: true, label: 'Symbols' }
    })

    await wrapper.get('input').trigger('focus')

    expect(wrapper.get('[role="status"]').text()).toContain('Loading options')
  })

  it('renders library names first with their algorithm or custom type below', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: [],
        options: ['Momentum study'],
        descriptions: { 'Momentum study': 'MACD' },
        optionNameFirst: true
      }
    })

    await wrapper.get('input').trigger('focus')

    expect(wrapper.get('.search-option-copy strong').text()).toBe('Momentum study')
    expect(wrapper.get('.search-option-copy small').text()).toBe('MACD')
  })

  it('shows up to twenty options, supports a visual icon, and replaces a single selection', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['OLD'],
        options: Array.from({ length: 15 }, (_, index) => `ITEM${index + 1}`),
        multiple: false,
        optionIcon: BrainCircuit
      }
    })

    await wrapper.get('input').trigger('focus')

    expect(wrapper.findAll('.search-menu button')).toHaveLength(15)
    expect(wrapper.get('.search-option-logo svg').exists()).toBe(true)
    await wrapper.get('.search-menu button').trigger('click')
    expect(wrapper.emitted('update:modelValue')[0]).toEqual([['ITEM1']])
    expect(wrapper.find('.search-menu').exists()).toBe(false)
  })

  it('ranks ticker prefixes first and bounds a large searchable result set', async () => {
    const options = [
      ...Array.from({ length: 30 }, (_, index) => `SHELL${String(index).padStart(2, '0')}`),
      ...Array.from({ length: 30 }, (_, index) => `ITEM${index}`),
      'INGA.AS',
      'SHELL'
    ]
    const descriptions = Object.fromEntries(options.map(option => [
      option,
      option === 'INGA.AS' ? 'ING GROEP N.V.' : `${option} trading company`
    ]))
    const wrapper = mount(SearchSelect, {
      props: { modelValue: [], options, descriptions }
    })

    await wrapper.get('input').trigger('focus')
    await wrapper.get('input').setValue('SHELL')

    expect(wrapper.findAll('.search-menu button')).toHaveLength(20)
    expect(wrapper.get('.search-menu button small').text()).toBe('SHELL')
    expect(wrapper.findAll('.search-menu button small').every(option =>
      option.text().startsWith('SHELL'))).toBe(true)
  })

  it('does not queue off-screen logo requests ahead of a selected symbol', async () => {
    const options = Array.from({ length: 100 }, (_, index) => `ITEM${index}`)
    const logos = Object.fromEntries(options.map(option => [
      option,
      `https://example.test/${option}.png`
    ]))
    const wrapper = mount(SearchSelect, {
      props: { modelValue: [], options, logos }
    })

    await wrapper.get('input').trigger('focus')

    const images = wrapper.findAll('.search-menu img')
    expect(images).toHaveLength(12)
    expect(images.every(image => image.attributes('fetchpriority') === 'low')).toBe(true)
    expect(images.every(image => image.attributes('loading') === 'lazy')).toBe(true)

    await wrapper.get('input').setValue('ITEM99')

    expect(wrapper.get('.search-menu img').attributes('src')).toBe(
      'https://example.test/ITEM99.png'
    )
  })

  it('retries a selected logo even if its menu image failed', async () => {
    let wrapper
    wrapper = mount(SearchSelect, {
      props: {
        modelValue: [],
        options: ['INGA.AS'],
        logos: { 'INGA.AS': 'https://example.test/inga.png' },
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })

    await wrapper.get('input').trigger('focus')
    await wrapper.get('.search-menu img').trigger('error')
    await wrapper.get('.search-menu button').trigger('click')
    await wrapper.vm.$nextTick()

    expect(wrapper.get('.tag img').attributes('src')).toBe('https://example.test/inga.png')
  })

  it('falls back to the dedicated selected logo if the menu logo fails in the chip', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['AVIANRO'],
        options: [],
        logos: { AVIANRO: 'https://example.test/menu-logo.png' },
        selectedLogos: { AVIANRO: 'https://example.test/selected-logo.png' }
      }
    })

    const selectedLogo = wrapper.get('.selected-symbol-logo')
    expect(selectedLogo.get('svg').exists()).toBe(true)
    expect(selectedLogo.get('img').attributes('src')).toBe('https://example.test/menu-logo.png')

    await selectedLogo.get('img').trigger('error')
    expect(selectedLogo.get('img').attributes('src')).toBe(
      'https://example.test/selected-logo.png'
    )

    await selectedLogo.get('img').trigger('load')
    expect(selectedLogo.find('svg').exists()).toBe(false)
    expect(selectedLogo.get('img').classes()).toContain('loaded')
  })

  it('retries a transient selected-logo failure with a cache-busting URL', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['ASML'],
        options: ['ASML'],
        logos: { ASML: 'https://example.test/asml.png?token=test' }
      }
    })

    const image = wrapper.get('.selected-symbol-logo img')
    await image.trigger('error')
    expect(image.attributes('src')).toBe(
      'https://example.test/asml.png?token=test&selected_retry=1'
    )

    await image.trigger('load')
    expect(image.classes()).toContain('loaded')
  })

  it('retains the same working menu logo after selecting a symbol', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['ABN.AS'],
        options: ['ABN.AS'],
        logos: { 'ABN.AS': 'https://example.test/menu-logo.png' },
        selectedLogos: { 'ABN.AS': 'https://example.test/selected-logo.png' }
      }
    })

    expect(wrapper.get('.selected-symbol-logo img').attributes('src')).toBe(
      'https://example.test/menu-logo.png'
    )
  })

  it('keeps one detailed benchmark selection after the control closes', async () => {
    let wrapper
    wrapper = mount(SearchSelect, {
      props: {
        modelValue: ['ASML'],
        options: ['ASML', 'AAPL'],
        descriptions: { ASML: 'ASML Holding N.V.', AAPL: 'Apple Inc.' },
        logos: { ASML: 'https://example.test/asml.png' },
        multiple: false,
        showSelectedDescription: true,
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })

    expect(wrapper.get('.tag').text()).toContain('ASML Holding N.V.')
    await wrapper.get('input').trigger('focus')
    await wrapper.get('.search-menu button').trigger('click')
    await wrapper.vm.$nextTick()

    expect(wrapper.findAll('.tag')).toHaveLength(1)
    expect(wrapper.get('.tag').text()).toContain('AAPL')
    expect(wrapper.find('.search-menu').exists()).toBe(false)
  })

  it('shows an explanation below plain order-type options without an image', async () => {
    const wrapper = mount(SearchSelect, {
      props: {
        modelValue: [],
        options: ['Market', 'Limit'],
        descriptions: {
          Market: 'Execute at the best available market price.',
          Limit: 'Execute only at the chosen price or better.'
        },
        plainOptions: true
      }
    })

    await wrapper.get('input').trigger('focus')

    expect(wrapper.get('.search-option-plain strong').text()).toBe('Market')
    expect(wrapper.get('.search-option-plain small').text()).toContain('best available')
    expect(wrapper.find('.search-option-logo').exists()).toBe(false)
  })
})
