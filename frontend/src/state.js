export function flattenFills(updates, limit = 50) {
  return updates.flatMap(update => update.fills || []).reverse().slice(0, limit)
}

export function cloneApiState(value) {
  return JSON.parse(JSON.stringify(value))
}

const experimentOptionValues = {
  instrument_type: {
    Stocks: 'stocks',
    ETF: 'etf',
    Forex: 'forex',
    Crypto: 'crypto'
  },
  interval: {
    '1m': 'OneMinute',
    '5m': 'FiveMinutes',
    '15m': 'FifteenMinutes',
    '30m': 'ThirtyMinutes',
    '1h': 'OneHour',
    '4h': 'FourHours',
    '1d': 'OneDay',
    '1w': 'OneWeek'
  },
  commission_type: {
    'Percentage (%)': 'Percentage',
    'Fixed amount': 'Fixed',
    'Percentage + Fixed': 'PercentagePlusFixed'
  },
  conversion_period: {
    day: 'Day',
    week: 'Week',
    month: 'Month',
    year: 'Year'
  }
}

export function experimentOptionValue(group, label) {
  return experimentOptionValues[group]?.[label] ?? label
}

export function consumeExperimentDraft(storage) {
  const key = 'backtide:experiment-config'
  const value = storage.getItem(key)
  storage.removeItem(key)
  if (!value) return null
  try {
    return JSON.parse(value)
  } catch {
    return null
  }
}

export function instrumentLogoUrl(symbol, instrumentType, apiKey) {
  if (!symbol || !apiKey) return ''
  const type = String(instrumentType).toLowerCase()
  const parts = String(symbol).split('-')
  const crypto = type.includes('crypto')
  const forex = type.includes('forex')
  const domain = crypto ? 'crypto' : 'ticker'
  const value = crypto
    ? parts[0]
    : forex && parts.length === 2
      ? `${parts[0]}${parts[1]}:CUR`
      : symbol
  return `https://img.logokit.com/${domain}/${encodeURIComponent(value)}?token=${encodeURIComponent(apiKey)}`
}

export function paperEquitySeries(updates) {
  return updates.map(update => ({
    timestamp: update.market?.received_ts || update.market?.close_ts || 0,
    equity: update.snapshot?.equity ?? null
  }))
}

export function formatResultMetric(raw, percent = false) {
  const numeric = Number(raw)
  if (!Number.isFinite(numeric)) return '—'
  if (percent) return `${(numeric * (Math.abs(numeric) <= 2 ? 100 : 1)).toFixed(2)}%`
  return numeric.toLocaleString(undefined, { maximumFractionDigits: 2 })
}

export function resolvePage(hash, validPages) {
  const candidate = String(hash || '').replace(/^#/, '')
  return validPages.includes(candidate) ? candidate : 'home'
}

export function symbolsForAnalysis(symbols, plot) {
  return ['candlestick', 'seasonality'].includes(plot) ? symbols.slice(0, 1) : symbols
}
