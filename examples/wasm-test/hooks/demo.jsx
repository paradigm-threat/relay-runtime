/**
 * Simple counter demo - CleverScript-compatible.
 * Works on web and native (relay-runtime).
 */
import React, { useState } from 'react'

export default function Demo() {
    const [count, setCount] = useState(0)
    return (
        <div className="app p-4">
            <h1 className="heading text-xl font-bold mb-4">Relay Demo</h1>
            <p className="mb-4">Count: {count}</p>
            <button
                className="px-3 py-1 bg-primary text-white rounded"
                onClick={() => setCount(count + 1)}
            >
                Increment
            </button>
        </div>
    )
}
