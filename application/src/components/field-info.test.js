// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FieldInfo from './field-info.vue'

describe('field info', () => {
  it('exposes its explanation to hover and keyboard users', () => {
    const wrapper = mount(FieldInfo, { props: { text: 'Controls the test interval.' } })

    expect(wrapper.attributes('tabindex')).toBe('0')
    expect(wrapper.attributes('aria-label')).toContain('Controls the test interval.')
    expect(wrapper.get('[role="tooltip"]').text()).toBe('Controls the test interval.')
  })
})
