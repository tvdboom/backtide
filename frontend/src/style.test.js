import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const styles = readFileSync(new URL('./style.css', import.meta.url), 'utf8')

function declaration(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return styles.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] || ''
}

describe('application typography', () => {
  it('enlarges page content without changing the navigation type scale', () => {
    expect(styles).toMatch(/\.page\s*\{[^}]*font-size: 18px;/)
    expect(styles).toMatch(/\.page \.eyebrow\s*\{[^}]*font-size: 13px;/)
    expect(styles).toMatch(/\.nav-label\s*\{[^}]*font-size: 12px;/)
    expect(styles).toMatch(/\.sidebar-footer a\s*\{[^}]*font-size: 13px;/)
  })
})

describe('download details', () => {
  it('shows provider, exchange, and currency metadata without boxes', () => {
    expect(declaration('.download-provider')).not.toMatch(/background|border/)
    expect(declaration('.download-profile-meta > span')).not.toMatch(/background|border/)
    expect(declaration('.download-profile-meta > span')).toContain('align-items: flex-end')
    expect(declaration('.download-profile-meta > span')).toContain('text-align: right')
  })
})

describe('experiment selectors', () => {
  it('keeps large symbol results inside a fixed-height scrolling menu', () => {
    expect(declaration('.experiment-builder')).toContain('overflow: visible')
    expect(declaration('.search-menu')).toContain('max-height: 600px')
  })

  it('gives market type buttons a distinct hover treatment', () => {
    expect(declaration('.segmented button:hover')).toContain('color: var(--blue-2)')
    expect(declaration('.segmented button:hover')).toContain('background:')
  })

  it('presents intervals as compact selectable pills', () => {
    expect(declaration('.interval-picker-field')).toContain('width: fit-content')
    expect(declaration('.interval-picker-field')).toContain('max-width: 100%')
    expect(declaration('.interval-picker')).toContain('width: fit-content')
    expect(declaration('.interval-picker')).toContain('max-width: 100%')
    expect(declaration('.interval-picker')).toContain('flex-wrap: wrap')
    expect(declaration('.interval-picker button')).toContain('border-radius: 999px')
    expect(declaration('.interval-picker button.selected')).toContain(
      'background: var(--surface-active)'
    )
    expect(declaration('.interval-picker button:disabled')).toContain('opacity: .38')
    expect(declaration('.interval-picker button:disabled')).toContain('cursor: not-allowed')
  })

  it('keeps the experiment name compact beside its icon picker', () => {
    expect(declaration('.experiment-identity-grid')).toContain('2.4fr')
    expect(declaration('.experiment-identity-grid')).toContain('190px')
  })

  it('gives selected market symbols a roomier logo treatment', () => {
    expect(declaration('.download-page')).toContain('1080px')
    expect(declaration('.symbol-select-field .tag-field')).toContain('min-height: 52px')
    expect(declaration('.symbol-select-field .selected-symbol-logo')).toContain('width: 25px')
  })

  it('separates the download asset-type control from the symbols field', () => {
    expect(declaration('.download-page .wide-control')).toContain('margin-bottom: 18px')
  })

  it('keeps the benchmark selector compact on desktop', () => {
    expect(declaration('.benchmark-field')).toContain('width: min(33.333%, 340px)')
    expect(styles).toMatch(/@media \(max-width: 760px\)[\s\S]*\.benchmark-field\s*\{\s*width: 100%;\s*\}/)
  })

  it('uses one interval-row background and separates the provider logo', () => {
    expect(declaration('.download-interval-row')).toContain('background: var(--panel-start)')
    expect(styles).not.toMatch(/\.download-interval-row:nth-child/)
    expect(declaration('.download-provider')).toContain('margin-right: 14px')
    expect(styles).toContain('.download-row-count small { margin-top: 3px; font-size: 15px;')
  })
})

describe('dashboard activity', () => {
  it('separates the primary metric from status and gives price history enough width', () => {
    expect(declaration('.primary-metric-value')).toContain('margin-right: 10px')
    expect(declaration('.market-sparkline')).toContain('width: 150px')
    expect(declaration('.market-sparkline')).toContain('margin-right: 12px')
  })
})

describe('paper trading setup', () => {
  it('matches toggle fields to standard input geometry', () => {
    expect(declaration('.toggle-label')).toContain('width: 100%')
    expect(declaration('.toggle-control')).toContain('width: 100%')
    expect(declaration('.toggle-control')).toContain('min-height: 42px')
    expect(declaration('.toggle-control')).toContain('height: 42px')
    expect(declaration('.toggle-title')).toContain('padding-right: 24px')
    expect(declaration('.toggle-description')).toContain('-webkit-line-clamp: 2')
  })

  it('shows accessible field and action popovers on hover or keyboard focus', () => {
    expect(declaration('.field-info-popover, .action-popover')).toContain('visibility: hidden')
    expect(styles).toContain('.field-info:hover .field-info-popover')
    expect(styles).toContain('.field-info:focus-visible .field-info-popover')
    expect(styles).toContain('.action-help .secondary:focus-visible + .action-popover')
    expect(styles).not.toContain('.action-help:focus-within .action-popover')
  })

  it('gives recent live-session financial values dedicated columns', () => {
    expect(declaration('.live-session-row')).toContain(
      'grid-template-columns: 38px minmax(190px, 1.35fr) minmax(180px, 1.15fr) minmax(120px, .85fr) minmax(110px, .75fr) 80px 17px'
    )
    expect(declaration('.session-financial')).toContain('margin-right: 0')
  })

  it('positions execution reasons in a viewport overlay', () => {
    expect(declaration('.execution-status-tooltip')).toContain('position: fixed')
    expect(declaration('.execution-status-tooltip')).toContain('z-index: 300')
    expect(declaration('.execution-status-tooltip')).toContain('calc(100vw - 24px)')
    expect(styles).toContain(".execution-status-tooltip[data-placement='below']")
  })

  it('keeps every live interval on one full-width row', () => {
    expect(declaration('.live-interval-field')).toContain('grid-column: 1 / -1')
    expect(declaration('.live-interval-field .interval-picker')).toContain('flex-wrap: nowrap')
    expect(declaration('.live-interval-field .interval-picker')).toContain('overflow-x: auto')
  })

  it('keeps the base-currency control compact beside initial cash', () => {
    expect(declaration('.currency-picker-field')).toContain('width: min(100%, 172px)')
    expect(declaration('.live-cash-field input')).toContain('height: 42px')
    expect(declaration('.live-base-currency .currency-select')).toContain('172px')
    expect(declaration('.live-base-currency .currency-trigger')).toContain('min-height: 42px')
    expect(declaration('.live-base-currency .currency-trigger')).toContain('height: 42px')
  })

  it('uses labeled, responsive market-feed columns for OHLCV values', () => {
    expect(declaration('.event-feed')).toContain('container-type: inline-size')
    expect(declaration('.event-log-header, .event-log-row')).toContain(
      'grid-template-columns: repeat(9, minmax(0, 1fr))'
    )
    expect(styles).toContain('grid-template-columns: repeat(6, minmax(0, 1fr))')
    expect(styles).toContain('grid-template-columns: repeat(3, minmax(0, 1fr))')
    expect(declaration('.event-open, .event-high, .event-low, .event-close, .event-volume, .event-fills')).toContain(
      'white-space: nowrap'
    )
    expect(declaration('.event-symbol')).toContain('display: flex')
    expect(declaration('.event-symbol-logo')).toContain('width: 20px')
    expect(styles).toContain('@container (max-width: 900px)')
    expect(styles).toContain('@container (max-width: 620px)')
  })

  it('keeps strategy telemetry compact and the strategy switcher attached to the chart', () => {
    expect(declaration('.live-observability')).toContain('align-items: start')
    expect(declaration('.live-observability .config-summary')).toContain('display: grid')
    expect(declaration('.live-observability .config-summary')).toContain(
      'grid-template-columns: repeat(2, minmax(0, 1fr))'
    )
    expect(declaration('.live-observability .config-summary > div')).toContain(
      'background: var(--surface-muted)'
    )
    expect(declaration('.live-observability .config-summary dd')).toContain('margin: 5px 0 0')
    expect(declaration('.live-observability .risk-metric')).toContain('position: relative')
    expect(declaration('.live-observability .risk-metric')).toContain('padding-right: 42px')
    expect(declaration('.live-observability .risk-metric-help')).toContain('top: 10px')
    expect(declaration('.live-observability .risk-metric-help')).toContain('right: 10px')
    expect(declaration('.live-strategy-switcher')).toContain('border-top: 1px solid var(--line)')
    expect(declaration('.live-strategy-switcher')).toContain('margin: 0')
  })

  it('keeps live session actions aligned and readable at narrow widths', () => {
    expect(declaration('.session-actions')).toContain('justify-content: flex-end')
    expect(declaration('.session-actions button, .session-actions .status-pill'))
      .toContain('white-space: nowrap')
    expect(styles).toContain('.session-actions .status-pill { min-height: 40px')
    expect(styles).toContain(
      '.live-intro .session-actions { width: 100%; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr));'
    )
    expect(styles).toContain('@media (max-width: 480px)')
  })

  it('styles failed live sessions as red hoverable status badges', () => {
    expect(declaration('.status-pill.failed')).toContain('color: var(--red)')
    expect(declaration('.status-pill.failed')).toContain('cursor: help')
    expect(declaration('.session-status-tooltip')).toContain('max-width: min(480px')
    expect(styles).toContain(
      '.status-pill:hover .session-status-tooltip, .status-pill:focus-visible .session-status-tooltip'
    )
  })
})

describe('session history', () => {
  it('keeps column positions fixed while replay rows expand', () => {
    expect(declaration('.session-history-table')).toContain('table-layout: fixed')
    expect(declaration('.session-history-table')).toContain('min-width: 1240px')
    expect(declaration('.session-history-table th, .session-history-table td'))
      .toContain('padding-inline: 10px')
    expect(declaration('.session-history-table th:nth-child(1)')).toContain('width: 14%')
    expect(declaration('.session-history-table th:nth-child(7)')).toContain('width: 300px')
    expect(declaration('.session-history-table td:last-child')).toContain('overflow: hidden')
    expect(declaration('.session-history-actions .compact-button')).toContain('max-width: 100%')
    expect(declaration('.session-history-actions .compact-button')).toContain('padding-inline: 12px')
  })

  it('keeps strategy names inside their column and exposes overflow details', () => {
    expect(declaration('.session-strategy-summary')).toContain('min-width: 0')
    expect(declaration('.session-strategy-summary')).toContain('display: inline-flex')
    expect(declaration('.session-strategy-visible')).toContain('white-space: nowrap')
    expect(declaration('.session-strategy-visible')).toContain('text-overflow: ellipsis')
    expect(declaration('.session-strategy-overflow')).toContain('border-radius: 999px')
    expect(declaration('.session-strategy-tooltip')).toContain('position: fixed')
    expect(styles).toContain('.session-strategy-overflow:focus-visible')
  })
})

describe('experiment result summaries', () => {
  it('keeps the results subtitle on one line while allowing the action to wrap', () => {
    expect(declaration('.results-page-intro')).toContain('flex-wrap: wrap')
    expect(declaration('.results-page-intro > div')).toContain('flex: 1 1 760px')
    expect(declaration('.results-page-intro p')).toContain('white-space: nowrap')
  })

  it('keeps tags subordinate to the title and presents breakdown metrics without tiles', () => {
    expect(declaration('.experiment-result-identity h3')).toContain('font-size: 22px')
    expect(declaration('.experiment-result-summary')).toContain('min-height: 124px')
    expect(declaration('.experiment-card-actions')).toContain('align-self: flex-start')
    expect(declaration('.experiment-result-tags')).toContain('margin-top: 12px')
    expect(declaration('.experiment-result-meta')).toContain('margin-top: 16px')
    expect(declaration('.experiment-result-meta')).toContain('font-size: 14px')
    expect(declaration('.result-tag')).toContain('font-size: 10px')
    expect(declaration('.run-summary-metrics > div')).not.toContain('background')
    expect(declaration('.run-summary-metrics > div + div')).toContain('border-left: 0')
    expect(declaration('.run-breakdown-card')).toContain('background: transparent')
  })

  it('uses Streamlit-style metric rows and a right-side plot options column', () => {
    expect(declaration('.primary-metrics')).toContain('grid-template-columns: .8fr 1.2fr 1fr 1fr')
    expect(declaration('.context-metrics')).toContain('grid-template-columns: .8fr 1.2fr 1fr 1fr')
    expect(declaration('.result-summary-panel')).toContain('padding: 0')
    expect(declaration('.result-overview-metrics')).toContain('margin: 0')
    expect(declaration('.result-overview-metrics')).toContain('background: transparent')
    expect(declaration('.result-overview-row + .result-overview-row')).toContain('border-top: 0')
    expect(declaration('.result-overview-metric + .result-overview-metric')).toContain('border-left: 0')
    expect(declaration('.result-overview-metric > span')).toContain('font-size: 12px')
    expect(declaration('.result-overview-metric > strong')).toContain('font-size: 18px')
    expect(declaration('.result-plot-stage.has-options')).toContain('220px')
    expect(declaration('.result-plot-options')).toContain('flex-direction: column')
    expect(declaration('.result-plot-options')).toContain('border-left: 1px solid var(--line)')
    expect(declaration('.result-plot-tabs button')).toContain('min-height: 54px')
    expect(declaration('.result-plot-description p')).toContain('font-size: 15px')
    expect(declaration('.strategy-plot-tabs')).toContain('repeat(5, minmax(0, 1fr))')
    expect(declaration('.modal.document-modal')).toContain('width: min(1140px, 94vw)')
  })

  it('keeps order history in a bounded table with visible headings', () => {
    expect(declaration('.result-record-table')).toContain('max-height: 560px')
    expect(declaration('.result-record-table')).toContain('overflow: auto')
    expect(declaration('.result-record-table .data-table th')).toContain('position: sticky')
  })
})

describe('library editor', () => {
  it('uses the available screen width for source editing', () => {
    expect(declaration('.modal.library-editor')).toContain('width: min(1200px, 72vw)')
  })

  it('separates mode, fields, and built-in guidance into distinct sections', () => {
    expect(declaration('.library-editor-mode')).toContain('margin-bottom: 24px')
    expect(declaration('.library-editor-fields')).toContain('gap: 20px')
    expect(declaration('.library-editor-callout')).toContain('margin-top: 24px')
  })
})
