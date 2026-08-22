// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import StrategySummary from './strategy-summary.vue'

describe('strategy summary', () => {
  afterEach(() => {
    document.body.innerHTML = ''
    vi.unstubAllGlobals()
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

  it('moves the second strategy into the overflow pill when both names do not fit', async () => {
    let resize
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback) { resize = callback }
      observe() {}
      disconnect() {}
    })
    const wrapper = mount(StrategySummary, {
      props: { names: ['Buy & Hold', 'AlphaRSI Pro'] }
    })
    await flushPromises()
    const visible = wrapper.get('.session-strategy-visible')
    Object.defineProperties(visible.element, {
      clientWidth: { configurable: true, value: 100 },
      scrollWidth: { configurable: true, value: 180 }
    })

    resize()
    await flushPromises()

    expect(visible.text()).toBe('Buy & Hold')
    expect(wrapper.get('.session-strategy-overflow').text()).toBe('+1')
    expect(wrapper.get('.session-strategy-overflow').attributes('aria-label'))
      .toBe('1 more strategy')
  })

  it('labels sessions without strategies as monitor-only', () => {
    const wrapper = mount(StrategySummary)

    expect(wrapper.text()).toBe('Monitor only')
  })
})
