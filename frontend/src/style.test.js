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
  })
})

describe('library editor', () => {
  it('separates mode, fields, and built-in guidance into distinct sections', () => {
    expect(declaration('.library-editor-mode')).toContain('margin-bottom: 24px')
    expect(declaration('.library-editor-fields')).toContain('gap: 20px')
    expect(declaration('.library-editor-callout')).toContain('margin-top: 24px')
  })
})
