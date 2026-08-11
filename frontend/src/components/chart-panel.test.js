// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ChartPanel from './chart-panel.vue'

const plotly = vi.hoisted(() => ({
  react: vi.fn().mockResolvedValue(undefined),
  purge: vi.fn()
}))

vi.mock('plotly.js-dist-min', () => ({ default: plotly }))

describe('ChartPanel', () => {
  beforeEach(() => vi.clearAllMocks())

  it('draws a figure that already exists when the component mounts', async () => {
    const figure = { data: [{ x: [1], y: [2] }], layout: { title: 'Returns', width: 900 } }

    const wrapper = mount(ChartPanel, { props: { figure } })
    await vi.waitFor(() => expect(plotly.react).toHaveBeenCalledOnce())

    expect(plotly.react.mock.calls[0][1]).toEqual(figure.data)
    expect(plotly.react.mock.calls[0][2].legend.bgcolor).toBe('rgba(0, 0, 0, 0)')
    expect(plotly.react.mock.calls[0][2].width).toBeUndefined()
    wrapper.unmount()
  })

  it('redraws with the active theme colors when the theme changes', async () => {
    const figure = { data: [{ x: [1], y: [2] }], layout: {} }
    const wrapper = mount(ChartPanel, { props: { figure } })
    await vi.waitFor(() => expect(plotly.react).toHaveBeenCalledOnce())

    document.documentElement.style.setProperty('--chart-text', '#334155')
    document.documentElement.style.setProperty('--chart-grid', '#dce4ef')
    document.documentElement.dataset.theme = 'light'
    await vi.waitFor(() => expect(plotly.react).toHaveBeenCalledTimes(2))

    const layout = plotly.react.mock.calls[1][2]
    expect(layout.font.color).toBe('#334155')
    expect(layout.xaxis.gridcolor).toBe('#dce4ef')
    wrapper.unmount()
  })
})
