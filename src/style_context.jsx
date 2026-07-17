/**
 * @clevertree/style — hook-owned theme loader (embedded in relay-runtime).
 * Gates children until theme YAML is fetched, validated, and applied to the session.
 */
import React, { useState, useEffect } from 'react'
import { setSessionThemeYaml, looksLikeThemeYaml } from '@clevertree/style-internal'

export default function StyleContext(props) {
  const src = props.src
  const children = props.children
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
      <div style={{ padding: '12px', fontFamily: 'sans-serif' }}>
        <p style={{ margin: '0 0 8px', fontWeight: 'bold' }}>Theme error</p>
        <p style={{ margin: '0 0 4px' }}>{status}</p>
        <p style={{ margin: 0, fontSize: '12px', opacity: 0.7 }}>{src}</p>
      </div>
    )
  }
  if (phase !== 'ready') {
    return (
      <div style={{ padding: '12px', fontFamily: 'sans-serif' }}>
        <p style={{ margin: 0 }}>{status}</p>
      </div>
    )
  }
  return children
}
