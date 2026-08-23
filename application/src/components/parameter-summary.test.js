// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ParameterSummary from './parameter-summary.vue'

describe('parameter summary', () => {
  it('shows both values without an overflow balloon when there are two parameters', () => {
    const wrapper = mount(ParameterSummary, {
      props: { parameters: { quantity: 150, leverage: 2 } }
    })

    expect(wrapper.findAll('.parameter-value').map(item => item.text())).toEqual([
      'quantity=150',
      'leverage=2'
    ])
    expect(wrapper.find('.parameter-overflow').exists()).toBe(false)
  })

  it('shows two values when they fit and puts the remainder in a hover balloon', async () => {
    const wrapper = mount(ParameterSummary, {
      props: { parameters: { fast: 10, slow: 100, threshold: 0.02, quantity: 50 } }
    })
    const summary = wrapper.get('.parameter-summary')

    expect(summary.findAll('.parameter-value').map(item => item.text())).toEqual([
      'fast=10',
      'slow=100'
    ])
    expect(summary.get('.parameter-overflow').text()).toBe('+2')

    await summary.get('.parameter-overflow').trigger('mouseenter')
    expect(document.body.querySelector('[role="tooltip"]')?.textContent.trim())
      .toBe(`threshold=${(0.02).toLocaleString()} · quantity=50`)

    wrapper.unmount()
  })
})
