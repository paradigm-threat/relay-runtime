// CleverScript-compatible Home: peers list, add/remove, no fetch/setInterval
import React, { useState, useEffect } from 'react'

const STORAGE_KEY_PEERS = 'relay:peers'
const DEFAULT_PEERS = 'https://node-dfw1.relaynet.online;https://node-dfw2.relaynet.online;http://localhost:8080'

export default function Home(props) {
    const openPeerTab = props.openPeerTab
    const [peers, setPeers] = useState([])

    const loadPeers = () => {
        try {
            const stored = localStorage.getItem(STORAGE_KEY_PEERS)
            if (stored) {
                const peerList = JSON.parse(stored)
                if (Array.isArray(peerList) && peerList.length > 0) {
                    setPeers(peerList.map(host => ({ host: host, isProbing: false, probes: [] })))
                    return
                }
            }
            const envPeers = DEFAULT_PEERS
            const peerList = envPeers.split(';').map(p => p.trim()).filter(p => p.length > 0)
            setPeers(peerList.map(host => ({ host: host, isProbing: false, probes: [] })))
        } catch (e) {
            // fallback to default
            const peerList = DEFAULT_PEERS.split(';').map(p => p.trim()).filter(p => p.length > 0)
            setPeers(peerList.map(host => ({ host: host, isProbing: false, probes: [] })))
        }
    }

    const handleRemovePeer = (e, host) => {
        if (e && e.stopPropagation) e.stopPropagation()
        const updatedPeers = peers.filter(p => p.host !== host)
        setPeers(updatedPeers)
        localStorage.setItem(STORAGE_KEY_PEERS, JSON.stringify(updatedPeers.map(p => p.host)))
    }

    useEffect(() => {
        loadPeers()
    }, [])

    return (
        <div className="flex flex-col h-full">
            <div className="p-4 border-b">
                <div className="flex flex-col gap-3 mb-4">
                    <h2 className="m-0 text-xl font-semibold">Relay Peers</h2>
                    <p className="text-sm text-gray-600 m-0">Tap a peer to open, or tap ✕ to remove.</p>
                </div>
            </div>
            <div className="flex-1 overflow-y-auto flex flex-col gap-1 p-2">
                {peers.length === 0 ? (
                    <div className="flex items-center justify-center h-full p-8 text-center">
                        <p className="m-0">No peers configured. Add one using the form above.</p>
                    </div>
                ) : (
                    peers.map((peer) => (
                        <div
                            key={peer.host}
                            onClick={() => openPeerTab(peer.host)}
                            className="p-4 bg-white border rounded-lg cursor-pointer"
                        >
                            <div className="flex justify-between items-center gap-4 mb-2">
                                <div className="flex gap-2 flex-1">
                                    <span className="font-semibold text-base">{peer.host}</span>
                                </div>
                                <div className="flex gap-2">
                                    <button
                                        onClick={(ev) => handleRemovePeer(ev, peer.host)}
                                        className="px-2 py-1 bg-red-500 text-white rounded text-xs font-medium"
                                        title="Remove peer"
                                    >
                                        ✕
                                    </button>
                                </div>
                            </div>
                            <button
                                className="w-full px-2 py-2 mt-2 bg-blue-500 text-white border-none rounded cursor-pointer text-sm font-medium"
                                onClick={() => openPeerTab(peer.host)}
                            >
                                Open
                            </button>
                        </div>
                    ))
                )}
            </div>
        </div>
    )
}
