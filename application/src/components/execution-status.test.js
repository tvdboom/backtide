// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import ExecutionStatus from './execution-status.vue'

describe('execution status', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('renders its reason in the document overlay instead of the clipping container', async () => {
    const container = document.createElement('div')
    container.style.overflow = 'hidden'
    document.body.append(container)
    const wrapper = mount(ExecutionStatus, {
      attachTo: container,
      props: { status: 'Filled', reason: 'live market fill' }
    })
    const badge = wrapper.get('.badge')

    await badge.trigger('mouseenter')
    await flushPromises()

    const tooltip = document.body.querySelector('.execution-status-tooltip')
    expect(tooltip?.textContent.trim()).toBe('live market fill')
    expect(tooltip?.parentElement).toBe(document.body)
    expect(badge.attributes('aria-describedby')).toBe(tooltip?.id)

    await badge.trigger('mouseleave')
    expect(document.body.querySelector('.execution-status-tooltip')).toBeNull()
    wrapper.unmount()
  })

  it('also exposes the reason to keyboard users', async () => {
    const wrapper = mount(ExecutionStatus, {
      props: { status: 'Rejected', reason: 'insufficient cash' }
    })
    const badge = wrapper.get('.badge')

    expect(badge.attributes('tabindex')).toBe('0')
    await badge.trigger('focus')
    await flushPromises()

    expect(document.body.querySelector('.execution-status-tooltip')?.textContent.trim())
      .toBe('insufficient cash')

    await badge.trigger('blur')
    expect(document.body.querySelector('.execution-status-tooltip')).toBeNull()
    wrapper.unmount()
  })

  it.each([
    ['Filled', 'success'],
    ['Rejected', 'error'],
    ['Canceled', 'partial'],
    ['Pending', 'partial'],
    ['Unknown', 'neutral']
  ])('styles %s with the %s tone', (status, tone) => {
    const wrapper = mount(ExecutionStatus, { props: { status } })

    expect(wrapper.get('.badge').classes()).toContain(tone)

    wrapper.unmount()
  })
})
