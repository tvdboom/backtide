// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest'
import {
  THEME_STORAGE_KEY,
  applyTheme,
  normalizeTheme,
  persistTheme,
  resolveTheme
} from './theme'

describe('application theme', () => {
  beforeEach(() => {
    localStorage.clear()
    document.documentElement.removeAttribute('data-theme')
    document.documentElement.removeAttribute('style')
  })

  it('prefers a saved theme over the system preference', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'dark')

    expect(resolveTheme(localStorage, { matches: true })).toBe('dark')
  })

  it('uses the system preference when no theme has been saved', () => {
    expect(resolveTheme(localStorage, { matches: true })).toBe('light')
    expect(resolveTheme(localStorage, { matches: false })).toBe('dark')
  })

  it('applies and persists a normalized theme', () => {
    expect(normalizeTheme('unexpected')).toBe('dark')

    applyTheme('light')
    persistTheme('light', localStorage)

    expect(document.documentElement.dataset.theme).toBe('light')
    expect(document.documentElement.style.colorScheme).toBe('light')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('light')
  })
})
