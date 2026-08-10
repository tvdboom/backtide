import { describe, expect, it } from 'vitest'
import {
  cloneApiState,
  consumeExperimentDraft,
  experimentOptionValue,
  flattenFills,
  formatResultMetric,
  instrumentLogoUrl,
  paperEquitySeries,
  resolvePage,
  symbolsForAnalysis
} from './state'

describe('live session transformations', () => {
  it('flattens newest fills first and applies a bound', () => {
    const updates = [
      { fills: [{ order: { symbol: 'BTC-USD' } }] },
      { fills: [{ order: { symbol: 'ETH-USD' } }, { order: { symbol: 'SOL-USD' } }] }
    ]

    expect(flattenFills(updates, 2).map(fill => fill.order.symbol)).toEqual(['SOL-USD', 'ETH-USD'])
  })

  it('prefers receive timestamps for the paper equity series', () => {
    const series = paperEquitySeries([
      { market: { received_ts: 20, close_ts: 10 }, snapshot: { equity: 101000 } },
      { market: { close_ts: 30 }, snapshot: { equity: 102000 } }
    ])

    expect(series).toEqual([{ timestamp: 20, equity: 101000 }, { timestamp: 30, equity: 102000 }])
  })
})

describe('result and route state', () => {
  it('clones API defaults without retaining nested reactive references', () => {
    const defaults = { data: { symbols: ['BTC-USD'] } }
    const cloned = cloneApiState(defaults)

    cloned.data.symbols.push('ETH-USD')
    expect(defaults.data.symbols).toEqual(['BTC-USD'])
  })

  it('maps display labels to serialized experiment enum values', () => {
    expect(experimentOptionValue('instrument_type', 'Stocks')).toBe('stocks')
    expect(experimentOptionValue('interval', '1d')).toBe('OneDay')
    expect(experimentOptionValue('commission_type', 'Fixed amount')).toBe('Fixed')
    expect(experimentOptionValue('conversion_period', 'month')).toBe('Month')
  })

  it('consumes a saved experiment draft once', () => {
    const values = new Map([['backtide:experiment-config', '{"general":{"name":"Copy"}}']])
    const storage = {
      getItem: key => values.get(key) || null,
      removeItem: key => values.delete(key)
    }

    expect(consumeExperimentDraft(storage)).toEqual({ general: { name: 'Copy' } })
    expect(consumeExperimentDraft(storage)).toBeNull()
  })

  it('formats fractional and already-percent result metrics', () => {
    expect(formatResultMetric(0.125, true)).toBe('12.50%')
    expect(formatResultMetric(12.5, true)).toBe('12.50%')
    expect(formatResultMetric(undefined, true)).toBe('—')
  })

  it('falls back to home for an unknown route', () => {
    expect(resolvePage('#live', ['home', 'live'])).toBe('live')
    expect(resolvePage('#missing', ['home', 'live'])).toBe('home')
  })

  it('limits single-series analysis plots without changing multi-series plots', () => {
    const symbols = ['BTC-EUR', 'BTC-USDT']

    expect(symbolsForAnalysis(symbols, 'candlestick')).toEqual(['BTC-EUR'])
    expect(symbolsForAnalysis(symbols, 'seasonality')).toEqual(['BTC-EUR'])
    expect(symbolsForAnalysis(symbols, 'correlation')).toEqual(symbols)
  })

  it('builds legacy LogoKit URLs for equities, forex, and crypto instruments', () => {
    expect(instrumentLogoUrl('AAPL', 'Stocks', 'secret key')).toBe(
      'https://img.logokit.com/ticker/AAPL?token=secret%20key'
    )
    expect(instrumentLogoUrl('BTC-USD', 'Cryptocurrencies', 'token')).toBe(
      'https://img.logokit.com/crypto/BTC?token=token'
    )
    expect(instrumentLogoUrl('EUR-USD', 'Forex', 'token')).toBe(
      'https://img.logokit.com/ticker/EURUSD%3ACUR?token=token'
    )
    expect(instrumentLogoUrl('AAPL', 'Stocks', '')).toBe('')
  })
})
