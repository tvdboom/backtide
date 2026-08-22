export const THEME_STORAGE_KEY = 'backtide:theme'

const themeColors = {
  dark: '#080d19',
  light: '#f4f7fb'
}

function browserStorage() {
  try {
    return globalThis.localStorage
  } catch {
    return null
  }
}

export function normalizeTheme(theme) {
  return theme === 'light' ? 'light' : 'dark'
}

export function resolveTheme(
  storage = browserStorage(),
  mediaQuery = globalThis.matchMedia?.('(prefers-color-scheme: light)')
) {
  try {
    const stored = storage?.getItem(THEME_STORAGE_KEY)
    if (stored === 'light' || stored === 'dark') return stored
  } catch {
    // A system preference is still available when browser storage is disabled.
  }
  return mediaQuery?.matches ? 'light' : 'dark'
}

export function applyTheme(theme, root = globalThis.document?.documentElement) {
  const normalized = normalizeTheme(theme)
  if (!root) return normalized
  root.dataset.theme = normalized
  root.style.colorScheme = normalized
  globalThis.document
    ?.querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', themeColors[normalized])
  return normalized
}

export function persistTheme(theme, storage = browserStorage()) {
  try {
    storage?.setItem(THEME_STORAGE_KEY, normalizeTheme(theme))
  } catch {
    // The active theme still applies for this session when storage is unavailable.
  }
}
