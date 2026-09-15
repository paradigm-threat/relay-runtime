/**
 * Host mode helpers — CSR relay web client vs parked SSR/hydrate.
 */
import manifest from '@hooks'

function str(v) {
  if (v === null) return ''
  if (typeof v === 'boolean') {
    if (v) return 'true'
    return 'false'
  }
  if (typeof v === 'number') return '' + v
  if (typeof v === 'string') return v
  return ''
}

export function relayHostMode() {
  try {
    const m = str(manifest.relayHostMode)
    if (m.length > 0) return m
  } catch (e) {}
  // Gateway SSR does not inject relayHostMode. Default SSR so P0 article HTML
  // is emitted; only the web/Android CSR hosts set relayHostMode=csr.
  return 'ssr'
}

export function relaySsrArticlePresent() {
  try {
    if (manifest.relaySsrArticlePresent === true) return true
  } catch (e) {}
  return false
}

export function isCsrHost() {
  return relayHostMode() === 'csr'
}

/** True when catalog/article fetches should run before isClientLive (CSR web client). */
export function shouldFetchBeforeLive() {
  if (isCsrHost()) return true
  return relaySsrArticlePresent() !== true
}
