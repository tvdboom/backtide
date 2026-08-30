// @vitest-environment jsdom
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import InstrumentPreview from './instrument-preview.vue'

const { query } = vi.hoisted(() => ({ query: vi.fn() }))

vi.mock('../api', () => ({ query }))

describe('instrument-preview', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('merges catalog facts with a thirty-close stored trend', async () => {
    query.mockResolvedValue({
      exchange: 'XNAS',
      interval: '1d',
      provider: 'yahoo',
      quote: 'USD',
      sparkline: Array.from({ length: 30 }, (_, index) => 100 + index)
    })
    const anchor = document.createElement('button')
    anchor.getBoundingClientRect = () => ({ left: 10, right: 210, top: 40 })
    document.body.append(anchor)

    const wrapper = mount(InstrumentPreview, {
      props: {
        visible: true,
        symbol: 'AAPL',
        anchor,
        details: { name: 'Apple Inc.', instrument_type: 'stocks' }
      }
    })
    await flushPromises()

    const preview = document.body.querySelector('.instrument-preview')
    expect(preview.textContent).toContain('Apple Inc.')
    expect(preview.textContent).toContain('Latest 30 stored closes')
    expect(preview.textContent).toContain('XNAS')
    expect(preview.textContent).toContain('USD')
    expect(preview.querySelector('img[src="https://flagcdn.com/us.svg"]')).not.toBeNull()
    expect(preview.querySelector('img[src="/providers/yahoo.png"]').getAttribute('alt'))
      .toBe('Yahoo provider')
    expect(preview.textContent).not.toContain('Source')
    expect(preview.querySelector('p')).toBeNull()
    expect(preview.querySelector('polyline').getAttribute('points')).not.toBe('')
    expect(query).toHaveBeenCalledWith('/api/instrument-overview', {
      symbol: 'AAPL', instrument_type: 'stocks', provider: undefined
    })
    wrapper.unmount()
  })
})
