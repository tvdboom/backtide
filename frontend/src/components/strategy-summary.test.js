// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import StrategySummary from './strategy-summary.vue'

describe('strategy summary', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('shows two names and reveals the remaining names from the overflow pill', async () => {
    const wrapper = mount(StrategySummary, {
      props: {
        names: ['BB Mean Reversion', 'Buy & Hold', 'SMA (Naive)', 'RSI Breakout']
      }
    })

    expect(wrapper.get('.session-strategy-visible').text())
      .toBe('BB Mean Reversion, Buy & Hold')
    const overflow = wrapper.get('.session-strategy-overflow')
    expect(overflow.text()).toBe('+2')
    expect(overflow.attributes('aria-label')).toBe('2 more strategies')
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull()

    await overflow.trigger('mouseenter')

    const tooltip = document.body.querySelector('[role="tooltip"]')
    expect(tooltip?.textContent).toContain('SMA (Naive), RSI Breakout')
    expect(overflow.attributes('aria-describedby')).toBe(tooltip?.id)

    await overflow.trigger('mouseleave')
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull()

    await overflow.trigger('focus')
    expect(document.body.querySelector('[role="tooltip"]')?.textContent)
      .toContain('SMA (Naive), RSI Breakout')

    await overflow.trigger('blur')
    wrapper.unmount()
  })

  it('does not render an overflow pill for two or fewer strategies', () => {
    const wrapper = mount(StrategySummary, {
      props: { names: ['Momentum', 'Buy & Hold'] }
    })

    expect(wrapper.get('.session-strategy-visible').text()).toBe('Momentum, Buy & Hold')
    expect(wrapper.find('.session-strategy-overflow').exists()).toBe(false)
  })

  it('labels sessions without strategies as monitor-only', () => {
    const wrapper = mount(StrategySummary)

    expect(wrapper.text()).toBe('Monitor only')
  })
})
