// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ToggleField from './toggle-field.vue'

describe('toggle field', () => {
  it('presents its title, help, description, and switch as one field', async () => {
    const wrapper = mount(ToggleField, {
      props: {
        modelValue: false,
        label: 'Margin trading',
        description: 'Allow positions to use borrowed funds.',
        help: 'Allow simulated positions to use borrowed funds.'
      }
    })

    expect(wrapper.get('.toggle-title').text()).toBe('Margin trading')
    expect(wrapper.get('.field-info').attributes('aria-label')).toContain('borrowed funds')
    expect(wrapper.get('.toggle-description').text()).toBe(
      'Allow positions to use borrowed funds.'
    )

    await wrapper.get('.toggle').setValue(true)

    expect(wrapper.emitted('update:modelValue')).toEqual([[true]])
    expect(wrapper.emitted('change')).toHaveLength(1)
  })
})
