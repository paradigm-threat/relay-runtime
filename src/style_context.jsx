import React, { useState, useEffect } from 'react'
import { setSessionThemeYaml, looksLikeThemeYaml } from '@clevertree/style-internal'

export default function StyleContext(props) {
  const src = props.src
  const children = props.children
  let fullScreen = true
  try {
    if (props.fullScreen === false) fullScreen = false
  } catch (e) {}
  const [phase, setPhase] = useState('loading')
  const [status, setStatus] = useState('Loading theme…')

  useEffect(() => {
    if (phase !== 'loading') return
    if (!src) {
      setPhase('error')
      setStatus('StyleContext: src is required')
      return
    }
    const res = fetch(src)
    if (!res || res.ok !== true || !res.text) {
      let st = 0
      if (res && res.status) st = res.status
      setPhase('error')
      setStatus('Failed to fetch theme (HTTP ' + st + ')')
      return
    }
    const text = res.text
    if (!looksLikeThemeYaml(text)) {
      setPhase('error')
      setStatus('Invalid theme YAML at ' + src)
      return
    }
    setStatus('Applying theme…')
    setSessionThemeYaml(text)
    setPhase('ready')
  })

  if (phase === 'error') {
    return (
      <div className="relay-theme-error" style={{ padding: '12px', fontFamily: 'sans-serif' }}>
        <p style={{ margin: '0 0 8px', fontWeight: 'bold' }}>Theme error</p>
        <p style={{ margin: '0 0 4px' }}>{status}</p>
        <p style={{ margin: 0, fontSize: '12px', opacity: 0.7 }}>{src}</p>
      </div>
    )
  }
  if (phase !== 'ready') {
    if (!fullScreen) {
      return null
    }
    return (
      <div className="relay-theme-loading relay-boot-shell" style={{
        position: 'fixed', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
        background: '#0f172a', color: '#e2e8f0', fontFamily: 'ui-sans-serif, system-ui, sans-serif',
        zIndex: 9998,
      }}>
        <div style={{ textAlign: 'center' }}>
          <div className="relay-boot-spinner" style={{
            width: '2rem', height: '2rem', margin: '0 auto 1rem', borderRadius: '9999px',
            border: '3px solid rgba(148,163,184,0.35)', borderTopColor: '#38bdf8',
            animation: 'relay-boot-spin 0.8s linear infinite',
          }} />
          <p style={{ margin: 0 }}>{status}</p>
        </div>
      </div>
    )
  }
  return children
}
