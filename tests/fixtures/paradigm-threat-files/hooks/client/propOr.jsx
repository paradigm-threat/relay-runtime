/**
 * Safe optional prop read — CleverScript throws on missing keys.
 */
export function propOr(props, key, fallback) {
  try {
    const v = props[key]
    if (v === null) return fallback
    if (typeof v === 'undefined') return fallback
    if (typeof v === 'string' && v.length === 0) return fallback
    return v
  } catch (e) {
    return fallback
  }
}

export function propStr(props, key, fallback) {
  const v = propOr(props, key, fallback)
  if (typeof v === 'string') return v
  if (typeof v === 'number') return '' + v
  if (typeof v === 'boolean') {
    if (v) return 'true'
    return 'false'
  }
  return fallback
}
