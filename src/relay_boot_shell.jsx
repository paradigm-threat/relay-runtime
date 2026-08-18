/**
 * @relay/ui — full-viewport boot overlay while relay-runtime loads hooks/theme.
 * Reads status from hooks manifest (relayBootPhase, relayBootMessage, …).
 */
import React from 'react'
import manifest from '@hooks'
import { isClientLive } from '@clevertree/style-internal'

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

function bootField(name, fallback) {
  try {
    if (manifest && manifest[name] !== null && typeof manifest[name] !== 'undefined') {
      const s = str(manifest[name])
      if (s.length > 0) return s
    }
  } catch (e) {}
  return fallback
}

export default function RelayBootShell(props) {
  const children = props.children
  let brand = bootField('relayBootBrand', '')
  try {
    if (props.brand !== null && typeof props.brand !== 'undefined') {
      const b = str(props.brand)
      if (b.length > 0) brand = b
    }
  } catch (e) {}
  let phase = bootField('relayBootPhase', 'boot')
  let message = bootField('relayBootMessage', 'Loading…')
  let detail = bootField('relayBootDetail', '')
  const progress = bootField('relayBootProgress', '')
  const live = isClientLive() === true

  if (phase === 'idle') {
    phase = 'boot'
  }

  const errored = phase === 'error'
  // Once the host marks the client live, drop the overlay even if the last
  // manifest snapshot still says phase=render.
  const booting = !live && !errored

  if (!booting && !errored) {
    return children
  }

  if (errored) {
    return (
      <div
        className="relay-boot-shell relay-boot-shell-error"
        role="alert"
        style={{
          position: 'fixed', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: '#0f172a', color: '#fecaca', fontFamily: 'ui-sans-serif, system-ui, sans-serif',
          zIndex: 9999, padding: '1.5rem',
        }}
      >
        <div style={{ maxWidth: '32rem', width: '100%' }}>
          <p style={{ margin: '0 0 0.5rem', fontWeight: 700, fontSize: '1.125rem' }}>Could not load site</p>
          <p style={{ margin: '0 0 0.75rem' }}>{message.length > 0 ? message : 'Boot failed'}</p>
          {detail.length > 0 ? (
            <pre style={{
              margin: 0, padding: '0.75rem', background: 'rgba(0,0,0,0.35)', borderRadius: '0.375rem',
              overflow: 'auto', fontSize: '0.8125rem', whiteSpace: 'pre-wrap',
            }}>{detail}</pre>
          ) : null}
        </div>
      </div>
    )
  }

  return (
    <div className="relay-boot-root">
      <div
        className="relay-boot-shell"
        aria-busy="true"
        aria-live="polite"
        style={{
          position: 'fixed', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: '#0f172a', color: '#e2e8f0', fontFamily: 'ui-sans-serif, system-ui, sans-serif',
          zIndex: 9999,
        }}
      >
        <div style={{ textAlign: 'center', padding: '1.5rem' }}>
          {brand.length > 0 ? (
            <p style={{ margin: '0 0 1rem', fontSize: '1.5rem', fontWeight: 700 }}>{brand}</p>
          ) : null}
          <div
            className="relay-boot-spinner"
            aria-hidden="true"
            style={{
              width: '2.5rem', height: '2.5rem', margin: '0 auto 1rem', borderRadius: '9999px',
              border: '3px solid rgba(148,163,184,0.35)', borderTopColor: '#38bdf8',
            }}
          />
          <p className="relay-boot-status" style={{ margin: 0 }}>{message}</p>
          {progress.length > 0 ? (
            <p className="relay-boot-progress" style={{ margin: '0.5rem 0 0', opacity: 0.75, fontSize: '0.875rem' }}>
              {progress}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  )
}
