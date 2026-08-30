// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import BenchmarkSelect from './benchmark-select.vue'

describe('benchmark-select', () => {
  it('replaces one selected benchmark and preserves it when clicking outside', async () => {
    let wrapper
    wrapper = mount(BenchmarkSelect, {
      attachTo: document.body,
      props: {
        modelValue: 'AAPL',
        options: ['AAPL', 'ASML.AS'],
        descriptions: { AAPL: 'Apple Inc.', 'ASML.AS': 'ASML Holding N.V.' },
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })

    const input = wrapper.get('input')
    expect(input.element.value).toBe('AAPL')
    expect(wrapper.find('.tag').exists()).toBe(false)
    await input.trigger('focus')
    await input.setValue('ASML')
    await wrapper.get('[role="option"]').trigger('click')
    await wrapper.vm.$nextTick()
    expect(input.element.value).toBe('ASML.AS')

    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    await wrapper.vm.$nextTick()
    expect(input.element.value).toBe('ASML.AS')
    expect(wrapper.emitted('update:modelValue')).toEqual([['ASML.AS']])
    wrapper.unmount()
  })

  it('restores the current value when an unfinished search loses focus', async () => {
    const wrapper = mount(BenchmarkSelect, {
      attachTo: document.body,
      props: { modelValue: 'AAPL', options: ['AAPL', 'MSFT'] }
    })

    const input = wrapper.get('input')
    await input.trigger('focus')
    await input.setValue('unfinished')
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    await wrapper.vm.$nextTick()

    expect(input.element.value).toBe('AAPL')
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
    wrapper.unmount()
  })

  it('preserves mixed-case names in single-selection mode', async () => {
    const wrapper = mount(BenchmarkSelect, {
      props: {
        modelValue: '',
        options: ['Custom crossover'],
        uppercaseValue: false,
        selectionName: 'strategy'
      }
    })

    await wrapper.get('input').trigger('focus')
    await wrapper.get('[role="option"]').trigger('click')

    expect(wrapper.emitted('update:modelValue')).toEqual([['Custom crossover']])
  })

  it('shows twenty prefix-ranked matches while searching a large catalog', async () => {
    const options = [
      ...Array.from({ length: 25 }, (_, index) => `SHELL${String(index).padStart(2, '0')}`),
      ...Array.from({ length: 25 }, (_, index) => `OTHER${index}`),
      'SHELL'
    ]
    const wrapper = mount(BenchmarkSelect, {
      props: { modelValue: '', options }
    })

    await wrapper.get('input').trigger('focus')
    await wrapper.get('input').setValue('SHELL')

    const matches = wrapper.findAll('[role="option"]')
    expect(matches).toHaveLength(20)
    expect(matches[0].text()).toContain('SHELL')
    expect(matches.every(option => option.text().startsWith('SHELL'))).toBe(true)
  })
})
