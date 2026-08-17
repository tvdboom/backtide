import { describe, expect, it } from 'vitest'
import {
  cloneApiState,
  configuredPlotlyDateTimeFormat,
  consumeExperimentDraft,
  consumeResultsOverviewRequest,
  defaultExperimentBenchmark,
  experimentOptionValue,
  flattenFills,
  formatConfiguredDate,
  formatConfiguredCurrency,
  formatConfiguredDateTime,
  formatConfiguredTime,
  formatConfiguredTimeWithSeconds,
  formatDaySpan,
  formatIntervalLabel,
  formatResultMetric,
  instrumentLogoUrl,
  paperEquitySeries,
  requestResultsOverview,
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

  it('requests the results overview once and clears a queued detail target', () => {
    const values = new Map([['backtide:result-id', 'old-experiment']])
    const storage = {
      getItem: key => values.get(key) || null,
      setItem: (key, value) => values.set(key, value),
      removeItem: key => values.delete(key)
    }

    requestResultsOverview(storage)

    expect(values.has('backtide:result-id')).toBe(false)
    expect(consumeResultsOverviewRequest(storage)).toBe(true)
    expect(consumeResultsOverviewRequest(storage)).toBe(false)
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
      'https://img.logokit.com/ticker/EURUSD:CUR?token=token'
    )
    expect(instrumentLogoUrl('AAPL', 'Stocks', '')).toBe('')
  })

  it('uses lowercase m for minute intervals', () => {
    expect(formatIntervalLabel('FifteenMinutes')).toBe('15m')
    expect(formatIntervalLabel('15M')).toBe('15m')
    expect(formatIntervalLabel('15 M')).toBe('15m')
  })

  it('formats long day spans as years and remaining days', () => {
    expect(formatDaySpan(9000)).toBe('24 years 240 days')
    expect(formatDaySpan(364)).toBe('364 days')
    expect(formatDaySpan(365)).toBe('1 year')
    expect(formatDaySpan(366)).toBe('1 year 1 day')
  })

  it('formats dates from the backend display configuration with an ISO fallback', () => {
    expect(formatConfiguredDate('2026-08-11', { date_format: 'DD-MM-YYYY' })).toBe('11-08-2026')
    expect(formatConfiguredDate('2026-08-11', { date_format: 'MM/DD/YYYY' })).toBe('08/11/2026')
    expect(formatConfiguredDate('2026-08-11', {})).toBe('2026-08-11')
    expect(formatConfiguredDate('2026-08-11', { date_format: 'invalid' })).toBe('2026-08-11')
  })

  it('formats timestamps with the configured time pattern and timezone', () => {
    const timestamp = Date.UTC(2026, 7, 11, 19, 5, 7) / 1000
    const display = {
      date_format: 'DD-MM-YYYY',
      datetime_format: 'DD-MM-YYYY HH:MM',
      timezone: 'UTC'
    }

    expect(formatConfiguredDateTime(timestamp, display)).toBe('11-08-2026 19:05')
    expect(formatConfiguredTime(timestamp, { time_format: 'hh:mm a', timezone: 'UTC' })).toBe('07:05 pm')
    expect(formatConfiguredTimeWithSeconds(timestamp, display)).toBe('19:05:07')
    expect(formatConfiguredTimeWithSeconds(
      timestamp,
      { time_format: 'hh:mm a', timezone: 'UTC' }
    )).toBe('07:05:07 pm')
    expect(formatConfiguredTimeWithSeconds(
      '2026-08-11T19:05:07.123Z',
      display
    )).toBe('19:05:07.123')
    expect(configuredPlotlyDateTimeFormat(display)).toBe('%d-%m-%Y %H:%M')
  })

  it('places the configured currency symbol before or after monetary values', () => {
    expect(formatConfiguredCurrency(-15.7786, 'EUR', {}, 4)).toBe('-€15.7786')
    expect(formatConfiguredCurrency(-15.7786, 'EUR', { currency_prefix: false }, 4))
      .toBe('-15.7786 €')
  })

  it('restores the original benchmark defaults for each experiment context', () => {
    expect(defaultExperimentBenchmark('USD', 'Stocks')).toBe('SPY')
    expect(defaultExperimentBenchmark('EUR', 'ETF')).toBe('EXW1.DE')
    expect(defaultExperimentBenchmark('EUR', 'Crypto', ['BTC-EUR'])).toBe('BTC-EUR')
    expect(defaultExperimentBenchmark('EUR', 'Crypto', ['ETH-EUR'])).toBe('BTC-USD')
    expect(defaultExperimentBenchmark('USD', 'Forex')).toBeNull()
  })
})
