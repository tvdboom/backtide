// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import CurrencySelect from './currency-select.vue'

const options = [
  { code: 'EUR', name: 'Euro', flag: '🇪🇺', country_code: 'eu' },
  { code: 'USD', name: 'United States Dollar', flag: '🇺🇸', country_code: 'us' }
]

describe('currency-select', () => {
  it('renders a compact selection and full currency details in the menu', async () => {
    const wrapper = mount(CurrencySelect, {
      props: { modelValue: 'USD', options, inputId: 'base-currency' }
    })

    expect(wrapper.get('.currency-trigger .currency-flag').attributes('src')).toBe(
      'https://flagcdn.com/us.svg'
    )
    expect(wrapper.get('.currency-trigger').text()).toContain('USD')
    expect(wrapper.get('.currency-trigger').text()).not.toContain('United States Dollar')

    await wrapper.get('.currency-trigger').trigger('click')

    const items = wrapper.findAll('[role="option"]')
    expect(items.map(option => option.get('strong').text())).toEqual(['EUR', 'USD'])
    expect(items.map(option => option.get('small').text())).toEqual([
      'Euro',
      'United States Dollar'
    ])
    expect(items.map(option => option.get('.currency-flag').attributes('src'))).toEqual([
      'https://flagcdn.com/eu.svg',
      'https://flagcdn.com/us.svg'
    ])
  })

  it('falls back to the country code when a flag image fails', async () => {
    const wrapper = mount(CurrencySelect, { props: { modelValue: 'USD', options } })

    await wrapper.get('.currency-trigger .currency-flag').trigger('error')

    expect(wrapper.find('.currency-trigger img').exists()).toBe(false)
    expect(wrapper.get('.currency-trigger .currency-flag-fallback').text()).toBe('US')
  })

  it('emits the chosen currency and closes the menu', async () => {
    const wrapper = mount(CurrencySelect, { props: { modelValue: 'USD', options } })

    await wrapper.get('.currency-trigger').trigger('click')
    await wrapper.findAll('[role="option"]')[0].trigger('click')

    expect(wrapper.emitted('update:modelValue')[0]).toEqual(['EUR'])
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false)
  })

  it('filters currencies by code or name as the user types', async () => {
    const wrapper = mount(CurrencySelect, { props: { modelValue: 'USD', options } })

    await wrapper.get('.currency-trigger').trigger('click')
    const search = wrapper.get('[aria-label="Search base currencies"]')

    await search.setValue('euro')
    expect(wrapper.findAll('[role="option"]').map(option => option.get('strong').text())).toEqual([
      'EUR'
    ])

    await search.setValue('dollar')
    expect(wrapper.findAll('[role="option"]').map(option => option.get('strong').text())).toEqual([
      'USD'
    ])
  })

  it('starts filtering when the user types while the compact trigger is focused', async () => {
    const extendedOptions = [
      ...options,
      { code: 'CAD', name: 'Canadian Dollar', flag: '', country_code: 'ca' }
    ]
    const wrapper = mount(CurrencySelect, {
      props: { modelValue: 'USD', options: extendedOptions }
    })

    await wrapper.get('.currency-trigger').trigger('keydown', { key: 'E' })

    expect(wrapper.get('[aria-label="Search base currencies"]').element.value).toBe('E')
    expect(wrapper.findAll('[role="option"]').map(option => option.get('strong').text())).toEqual([
      'EUR',
      'USD'
    ])
  })
})
