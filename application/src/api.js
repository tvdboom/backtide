export class ApiError extends Error {
  constructor(message, status) {
    super(message)
    this.status = status
  }
}

export async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: options.body ? { 'Content-Type': 'application/json' } : {},
    ...options,
    body: options.body ? JSON.stringify(options.body) : undefined
  })
  const data = await response.json().catch(() => ({}))
  if (!response.ok) throw new ApiError(data.error || `Request failed (${response.status})`, response.status)
  return data
}

export const post = (path, body = {}) => api(path, { method: 'POST', body })
export const put = (path, body = {}) => api(path, { method: 'PUT', body })
export const remove = (path, body) => api(path, { method: 'DELETE', body })

export function query(path, params = {}) {
  const search = new URLSearchParams()
  Object.entries(params).forEach(([key, value]) => {
    if (Array.isArray(value)) value.forEach(item => search.append(key, item))
    else if (value !== '' && value !== null && value !== undefined) search.set(key, value)
  })
  const suffix = search.size ? `?${search}` : ''
  return api(`${path}${suffix}`)
}
