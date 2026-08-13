// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import IntervalPicker from './interval-picker.vue'

describe('interval picker', () => {
  it('selects one serialized interval at a time', async () => {
    const wrapper = mount(IntervalPicker, {
      props: {
        modelValue: 'OneDay',
        options: ['1h', '1d'],
        values: { '1h': 'OneHour', '1d': 'OneDay' },
        label: 'Experiment interval',
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })

    expect(wrapper.attributes('role')).toBe('radiogroup')
    expect(wrapper.get('[aria-checked="true"]').text()).toBe('1d')

    await wrapper.findAll('button')[0].trigger('click')

    expect(wrapper.props('modelValue')).toBe('OneHour')
    expect(wrapper.findAll('[aria-checked="true"]')).toHaveLength(1)
    expect(wrapper.get('[aria-checked="true"]').text()).toBe('1h')
  })

  it('selects multiple intervals but keeps the final selection', async () => {
    const wrapper = mount(IntervalPicker, {
      props: {
        modelValue: ['1d'],
        options: ['1d', '1w'],
        multiple: true,
        label: 'Download intervals',
        'onUpdate:modelValue': value => wrapper.setProps({ modelValue: value })
      }
    })
    const buttons = wrapper.findAll('button')

    await buttons[0].trigger('click')
    expect(wrapper.props('modelValue')).toEqual(['1d'])

    await buttons[1].trigger('click')
    expect(wrapper.props('modelValue')).toEqual(['1d', '1w'])

    await buttons[0].trigger('click')
    await buttons[1].trigger('click')

    expect(wrapper.props('modelValue')).toEqual(['1w'])
    expect(wrapper.findAll('[aria-pressed="true"]')).toHaveLength(1)
  })
})
