// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import InstrumentSelect from './instrument-select.vue'

describe('instrument-select', () => {
  it('renders a conventional single-symbol trigger and selects one option', async () => {
    const wrapper = mount(InstrumentSelect, {
      attachTo: document.body,
      props: {
        modelValue: '',
        options: ['AAPL', 'MSFT'],
        descriptions: { AAPL: 'Apple Inc.', MSFT: 'Microsoft Corporation' },
        logos: { AAPL: 'https://example.test/aapl.png' },
        optionDetails: {
          AAPL: {
            exchange_mic: 'XNAS',
            exchange_name: 'NASDAQ Global Select Market',
            quote: 'USD',
            provider: 'yahoo'
          }
        },
        label: 'Starting position symbol'
      }
    })

    expect(wrapper.find('.tag').exists()).toBe(false)
    expect(wrapper.get('.instrument-select-trigger').text()).toContain('Select a symbol')
    await wrapper.get('.instrument-select-trigger').trigger('click')
    expect(wrapper.findAll('[role="option"]')).toHaveLength(2)
    expect(wrapper.get('[role="option"] img').attributes('src')).toContain('aapl.png')
    expect(wrapper.get('.instrument-menu-details').text()).toContain('XNAS')
    expect(wrapper.get('.instrument-menu-details').text()).toContain(
      'NASDAQ Global Select Market'
    )
    expect(wrapper.get('.instrument-menu-logo img').attributes('src')).toContain('aapl.png')
    expect(wrapper.find('.instrument-menu-sparkline').exists()).toBe(false)
    await wrapper.findAll('[role="option"]')[0].trigger('click')

    expect(wrapper.emitted('update:modelValue')).toEqual([['AAPL']])
    expect(wrapper.find('.instrument-select-menu').exists()).toBe(false)
    wrapper.unmount()
  })
})
