// CleverScript-compatible PeerContent: placeholder for peer hook content
// Remote hook loading not yet implemented on Android
import React, { useState } from 'react'

export default function PeerContent(props) {
    const host = props.host
    const [status] = useState('placeholder')

    return (
        <div className="flex-1 overflow-hidden flex items-center justify-center p-8">
            <div className="text-center">
                <p className="text-base font-semibold mb-2">Peer: {host}</p>
                <p className="text-sm text-gray-600">Remote peer content loading is not yet implemented.</p>
                <p className="text-xs text-gray-500 mt-4">Status: {status}</p>
            </div>
        </div>
    )
}
