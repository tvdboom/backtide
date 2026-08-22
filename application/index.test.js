import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { JSDOM } from 'jsdom'

describe('Application document metadata', () => {
  it('uses the Backtide logo as the browser tab icon', () => {
    const documentUrl = new URL('./index.html', import.meta.url)
    const document = new JSDOM(readFileSync(documentUrl, 'utf8')).window.document
    const icon = document.querySelector('link[rel="icon"]')

    expect(icon?.getAttribute('type')).toBe('image/png')
    expect(icon?.getAttribute('href')).toBe('/backtide-logo.png')
    expect(existsSync(fileURLToPath(new URL('./public/backtide-logo.png', import.meta.url)))).toBe(true)
  })
})
