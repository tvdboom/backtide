// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import StoragePage from './storage-page.vue'

const { api, remove } = vi.hoisted(() => ({
  api: vi.fn(),
  remove: vi.fn()
}))

vi.mock('../api', () => ({ api, remove }))

const series = [{
  symbol: 'AAPL',
  interval: '1d',
  provider: 'yahoo',
  name: 'Apple Inc.',
  instrument_type: 'stocks',
  n_rows: 250
}]

describe('storage page', () => {
  beforeEach(() => {
    api.mockReset().mockResolvedValue(series)
    remove.mockReset().mockResolvedValue({ deleted: 250 })
  })

  it('shows an icon for each storage metric', async () => {
    const wrapper = mount(StoragePage, { props: { bootstrap: {} } })
    await flushPromises()

    const metricIcons = wrapper.findAll('.metric-card .metric-icon')
    expect(metricIcons).toHaveLength(3)
    expect(metricIcons.every(icon => icon.find('svg').exists())).toBe(true)
  })

  it('shows each symbol logo with an initials fallback', async () => {
    const wrapper = mount(StoragePage, {
      props: { bootstrap: { display: { logokit_api_key: 'secret key' } } }
    })
    await flushPromises()

    const logo = wrapper.get('.storage-instrument .symbol-logo')
    expect(logo.attributes('src')).toBe(
      'https://img.logokit.com/ticker/AAPL?token=secret%20key'
    )

    await logo.trigger('error')

    expect(wrapper.find('.storage-instrument .symbol-logo').exists()).toBe(false)
    expect(wrapper.get('.storage-instrument .asset-avatar').text()).toBe('AA')
  })

  it('does not report empty storage while rows are loading', async () => {
    let resolveStorage
    api.mockReturnValueOnce(new Promise(resolve => { resolveStorage = resolve }))

    const wrapper = mount(StoragePage, { props: { bootstrap: {} } })

    expect(wrapper.text()).toContain('Loading stored market data…')
    expect(wrapper.text()).not.toContain('No stored series match this search.')

    resolveStorage(series)
    await flushPromises()

    expect(wrapper.text()).toContain('AAPL')
    expect(wrapper.text()).not.toContain('Loading stored market data…')
  })

  it('asks in-page before deleting selected stored series', async () => {
    const wrapper = mount(StoragePage, { props: { bootstrap: {} } })
    await flushPromises()
    await wrapper.findAll('input[type="checkbox"]')[1].setValue(true)

    await wrapper.get('.toolbar button.danger').trigger('click')

    expect(wrapper.get('[role="alertdialog"]').text()).toContain('Delete 1 stored series?')
    expect(remove).not.toHaveBeenCalled()

    await wrapper.get('.confirm-submit').trigger('click')
    await flushPromises()

    expect(remove).toHaveBeenCalledWith('/api/storage', {
      series: [['AAPL', '1d', 'yahoo']]
    })
  })
})
