// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import FieldInfo from './field-info.vue'

describe('field info', () => {
  it('shows its explanation in a viewport overlay for hover and keyboard users', async () => {
    const wrapper = mount(FieldInfo, { props: { text: 'Controls the test interval.' } })

    expect(wrapper.attributes('tabindex')).toBe('0')
    expect(wrapper.attributes('aria-label')).toContain('Controls the test interval.')
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull()

    await wrapper.trigger('mouseenter')
    const tooltip = document.body.querySelector('[role="tooltip"]')
    expect(tooltip?.textContent.trim()).toBe('Controls the test interval.')
    expect(tooltip?.parentElement).toBe(document.body)
    expect(wrapper.attributes('aria-describedby')).toBe(tooltip?.id)

    await wrapper.trigger('mouseleave')
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull()

    await wrapper.trigger('focus')
    expect(document.body.querySelector('[role="tooltip"]')?.textContent.trim())
      .toBe('Controls the test interval.')

    wrapper.unmount()
  })
})
