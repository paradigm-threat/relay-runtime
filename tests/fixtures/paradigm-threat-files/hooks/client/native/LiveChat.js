/**
 * CleverScript stub — div[data-relay-park-native=fb-live-chat].
 * Quoted object keys / spreads are not CleverScript-safe; use JSX attrs.
 * Optional props via propStr — CleverScript throws on missing keys.
 */
import React from 'react'
import { propStr } from 'hooks/client/propOr.jsx'

export default function LiveChat(props) {
  const p = props || {}
  const channel = propStr(p, 'channel', '')
  const path = propStr(p, 'path', '/')
  const mode = propStr(p, 'mode', 'embed')
  const prefix = propStr(p, 'prefix', '')
  let origin = propStr(p, 'origin', '')
  if (origin.length === 0) {
    origin = propStr(p, 'chatOrigin', '')
  }
  return (
    <div
      className="fb-live-chat-host"
      data-relay-park-native="fb-live-chat"
      data-path={path}
      data-mode={mode}
      data-channel={channel}
      data-prefix={prefix}
      data-chat-origin={origin}
    />
  )
}
