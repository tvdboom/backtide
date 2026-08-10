// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
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
    await wrapper.get('.search-menu button').trigger('mousedown')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual([['MSFT']])
  })

  it('removes selected tags', async () => {
    const wrapper = mount(SearchSelect, {
      props: { modelValue: ['AAPL'], options: [], allowCustom: true }
    })

    await wrapper.get('.tag button').trigger('click')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual([[]])
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
})
