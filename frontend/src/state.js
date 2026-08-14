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

export function formatIntervalLabel(value) {
  const labels = {
    OneMinute: '1m', FiveMinutes: '5m', FifteenMinutes: '15m', ThirtyMinutes: '30m',
    OneHour: '1h', FourHours: '4h', OneDay: '1d', OneWeek: '1w'
  }
  const label = labels[value] || String(value || '—')
  return label.replace(/^(\d+)\s*M$/, '$1m')
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

const benchmarkByCurrency = {
  AUD: 'VAS.AX', BRL: 'BOVA11.SA', CAD: 'XIC.TO', CHF: 'CSSPX.SW',
  CNY: '510300.SS', DKK: 'DKIGI.CO', EUR: 'EXW1.DE', GBP: 'ISF.L',
  HKD: '2800.HK', IDR: 'XIIT.JK', ILS: 'TA35.TA', INR: 'NIFTYBEES.NS',
  JPY: '1306.T', KRW: '069500.KS', MXN: 'NAFTRAC.MX', MYR: 'FBMKLCI-EA.KL',
  NOK: 'OBXEDNB.OL', NZD: 'FNZ.NZ', PLN: 'ETFW20L.WA', SEK: 'XACT-OMXS30.ST',
  SGD: 'ES3.SI', THB: 'TDEX.BK', TRY: 'DJIST.IS', TWD: '0050.TW',
  USD: 'SPY', ZAR: 'STX40.JO'
}

export function defaultExperimentBenchmark(baseCurrency, instrumentType, availableSymbols = []) {
  const type = String(instrumentType || '').toLowerCase()
  const currency = String(baseCurrency || 'USD').toUpperCase()
  if (type.includes('forex')) return null
  if (type.includes('crypto')) {
    const preferred = `BTC-${currency}`
    return availableSymbols.some(symbol => String(symbol).toUpperCase() === preferred)
      ? preferred
      : 'BTC-USD'
  }
  return benchmarkByCurrency[currency] || 'SPY'
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
  return numeric.toLocaleString('en', { maximumFractionDigits: 2 })
}

export function formatDaySpan(value) {
  const days = Math.max(0, Math.floor(Number(value) || 0))
  const years = Math.floor(days / 365)
  const remainingDays = days % 365
  return [
    years ? `${years} ${years === 1 ? 'year' : 'years'}` : '',
    remainingDays || !years ? `${remainingDays} ${remainingDays === 1 ? 'day' : 'days'}` : ''
  ]
    .filter(Boolean)
    .join(' ')
}

const DEFAULT_DATE_FORMAT = 'YYYY-MM-DD'
const DEFAULT_TIME_FORMAT = 'HH:mm'
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']

function pad(value) { return String(value).padStart(2, '0') }

function configuredDateFormat(display = {}) {
  const candidate = String(display?.date_format || '').trim()
  return /Y{2,4}/.test(candidate) && /M/.test(candidate) && /D/.test(candidate)
    ? candidate
    : DEFAULT_DATE_FORMAT
}

function configuredTimeFormat(display = {}) {
  const direct = String(display?.time_format || '').trim()
  if (direct) return direct
  const dateFormat = String(display?.date_format || '').trim()
  const dateTimeFormat = String(display?.datetime_format || '').trim()
  if (dateFormat && dateTimeFormat.startsWith(dateFormat)) {
    return dateTimeFormat.slice(dateFormat.length).trim() || DEFAULT_TIME_FORMAT
  }
  return DEFAULT_TIME_FORMAT
}

function configuredTimezone(display = {}) {
  const value = String(display?.timezone || '').trim()
  return value && !['none', 'null', 'local'].includes(value.toLowerCase()) ? value : undefined
}

function parsedDate(value) {
  if (value instanceof Date) return Number.isNaN(value.getTime()) ? null : value
  if (value === null || value === undefined || value === '') return null
  if (typeof value === 'number' || /^\d+(?:\.\d+)?$/.test(String(value))) {
    const numeric = Number(value)
    if (!Number.isFinite(numeric)) return null
    const date = new Date(numeric * (Math.abs(numeric) < 1e12 ? 1000 : 1))
    return Number.isNaN(date.getTime()) ? null : date
  }
  const dateOnly = String(value).match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (dateOnly) return new Date(Date.UTC(Number(dateOnly[1]), Number(dateOnly[2]) - 1, Number(dateOnly[3]), 12))
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}

function dateParts(value, display = {}) {
  const plainDate = typeof value === 'string' ? value.match(/^(\d{4})-(\d{2})-(\d{2})$/) : null
  if (plainDate) {
    const year = Number(plainDate[1])
    const month = Number(plainDate[2])
    const day = Number(plainDate[3])
    return { year, month, day, weekday: new Date(Date.UTC(year, month - 1, day)).getUTCDay(), hour: 0, minute: 0, second: 0, millisecond: 0 }
  }
  const date = parsedDate(value)
  if (!date) return null
  const timezone = configuredTimezone(display)
  if (timezone) {
    try {
      const values = Object.fromEntries(new Intl.DateTimeFormat('en-US', {
        timeZone: timezone,
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        weekday: 'long',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hourCycle: 'h23'
      }).formatToParts(date).map(part => [part.type, part.value]))
      const weekday = WEEKDAYS.indexOf(values.weekday)
      return {
        year: Number(values.year), month: Number(values.month), day: Number(values.day),
        weekday, hour: Number(values.hour), minute: Number(values.minute),
        second: Number(values.second), millisecond: date.getMilliseconds()
      }
    } catch {
      // Fall through to the local timezone when a configured timezone is invalid.
    }
  }
  return {
    year: date.getFullYear(), month: date.getMonth() + 1, day: date.getDate(),
    weekday: date.getDay(), hour: date.getHours(), minute: date.getMinutes(),
    second: date.getSeconds(), millisecond: date.getMilliseconds()
  }
}

function renderDate(parts, format) {
  const replacements = {
    YYYY: String(parts.year), YY: String(parts.year).slice(-2),
    MMMM: MONTHS[parts.month - 1], MMM: MONTHS[parts.month - 1].slice(0, 3),
    MM: pad(parts.month), M: String(parts.month), DD: pad(parts.day), D: String(parts.day),
    dddd: WEEKDAYS[parts.weekday], ddd: WEEKDAYS[parts.weekday].slice(0, 3), dd: WEEKDAYS[parts.weekday].slice(0, 3)
  }
  return format.replace(/YYYY|MMMM|MMM|dddd|ddd|YY|MM|DD|dd|M|D/g, token => replacements[token])
}

function renderTime(parts, format) {
  const hour12 = parts.hour % 12 || 12
  const replacements = {
    SSS: String(parts.millisecond || 0).padStart(3, '0'),
    HH: pad(parts.hour), H: String(parts.hour), hh: pad(hour12), h: String(hour12),
    MM: pad(parts.minute), M: String(parts.minute), mm: pad(parts.minute), m: String(parts.minute),
    ss: pad(parts.second), s: String(parts.second), A: parts.hour < 12 ? 'AM' : 'PM', a: parts.hour < 12 ? 'am' : 'pm'
  }
  return format.replace(/SSS|HH|hh|MM|mm|ss|H|h|M|m|s|A|a/g, token => replacements[token])
}

export function formatConfiguredDate(value, display = {}, fallback = '—') {
  const parts = dateParts(value, display)
  return parts ? renderDate(parts, configuredDateFormat(display)) : fallback
}

export function formatConfiguredTime(value, display = {}, fallback = '—') {
  const parts = dateParts(value, display)
  return parts ? renderTime(parts, configuredTimeFormat(display)) : fallback
}

export function formatConfiguredTimeWithSeconds(value, display = {}, fallback = '—') {
  const parts = dateParts(value, display)
  if (!parts) return fallback
  const format = configuredTimeFormat(display)
  const meridiem = format.match(/(\s*[Aa])$/)?.[1] || ''
  let base = meridiem ? format.slice(0, -meridiem.length) : format
  if (!/s/.test(base)) base = `${base}:ss`
  if (parts.millisecond && !/S/.test(base)) base = `${base}.SSS`
  return renderTime(parts, `${base}${meridiem}`)
}

export function formatConfiguredDateTime(value, display = {}, fallback = '—') {
  const parts = dateParts(value, display)
  if (!parts) return fallback
  return `${renderDate(parts, configuredDateFormat(display))} ${renderTime(parts, configuredTimeFormat(display))}`
}

export function formatConfiguredCurrency(
  value,
  currency = 'USD',
  display = {},
  maximumFractionDigits = 2
) {
  const parsed = Number(value)
  const amount = Number.isFinite(parsed) ? parsed : 0
  const code = currency || 'USD'
  const symbol = new Intl.NumberFormat('en', {
    style: 'currency',
    currency: code,
    currencyDisplay: 'narrowSymbol'
  }).formatToParts(0).find(part => part.type === 'currency')?.value || code
  const sign = amount < 0 ? '-' : ''
  const number = Math.abs(amount).toLocaleString('en', {
    minimumFractionDigits: 2,
    maximumFractionDigits
  })
  return display?.currency_prefix === false
    ? `${sign}${number} ${symbol}`
    : `${sign}${symbol}${number}`
}

export function configuredPlotlyDateTimeFormat(display = {}) {
  const dateTokens = {
    YYYY: '%Y', YY: '%y', MMMM: '%B', MMM: '%b', MM: '%m', M: '%-m',
    DD: '%d', D: '%-d', dddd: '%A', ddd: '%a', dd: '%a'
  }
  const timeTokens = {
    HH: '%H', H: '%-H', hh: '%I', h: '%-I', MM: '%M', M: '%-M',
    mm: '%M', m: '%-M', ss: '%S', s: '%-S', A: '%p', a: '%p'
  }
  const dateFormat = configuredDateFormat(display).replace(
    /YYYY|MMMM|MMM|dddd|ddd|YY|MM|DD|dd|M|D/g,
    token => dateTokens[token]
  )
  const timeFormat = configuredTimeFormat(display).replace(
    /HH|hh|MM|mm|ss|H|h|M|m|s|A|a/g,
    token => timeTokens[token]
  )
  return `${dateFormat} ${timeFormat}`
}

export function resolvePage(hash, validPages) {
  const candidate = String(hash || '').replace(/^#/, '')
  return validPages.includes(candidate) ? candidate : 'home'
}

export function symbolsForAnalysis(symbols, plot) {
  return ['candlestick', 'seasonality'].includes(plot) ? symbols.slice(0, 1) : symbols
}
