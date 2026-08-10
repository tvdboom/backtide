import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError, api, query } from './api'

describe('API client', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('serializes a JSON command and returns its response', async () => {
    const fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ id: 'job-1' })
    })
    vi.stubGlobal('fetch', fetch)

    const result = await api('/api/downloads', {
      method: 'POST',
      body: { symbols: ['AAPL'] }
    })

    expect(result).toEqual({ id: 'job-1' })
    expect(fetch).toHaveBeenCalledWith('/api/downloads', expect.objectContaining({
      method: 'POST',
      body: JSON.stringify({ symbols: ['AAPL'] })
    }))
  })

  it('surfaces the safe API error message', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      status: 422,
      json: () => Promise.resolve({ error: 'Select at least one symbol.' })
    }))

    await expect(api('/api/bars')).rejects.toEqual(
      new ApiError('Select at least one symbol.', 422)
    )
  })

  it('repeats array query parameters and omits empty values', async () => {
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve([]) })
    vi.stubGlobal('fetch', fetch)

    await query('/api/bars', { symbol: ['AAPL', 'MSFT'], interval: '1d', provider: '' })

    expect(fetch.mock.calls[0][0]).toBe('/api/bars?symbol=AAPL&symbol=MSFT&interval=1d')
  })
})
