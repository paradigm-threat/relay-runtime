/**
 * CleverScript stub — div[data-relay-park-native=fb-chat-app].
 * Optional props via propStr — CleverScript throws on missing keys.
 */
import React from 'react'
import { propStr } from 'hooks/client/propOr.jsx'

export default function ChatApp(props) {
  const p = props || {}
  const channel = propStr(p, 'channel', 'pt')
  return (
    <div
      className="fb-chat-app-host"
      data-relay-park-native="fb-chat-app"
      data-channel={channel}
    />
  )
}
