/**
 * Simple counter demo - CleverScript-compatible.
 * Works on web and native (relay-runtime).
 */
import React, { useState } from 'react'

export default function Demo() {
    const [count, setCount] = useState(0)
    return (
        <div className="app p-4">
            <h2 className="text-lg font-bold mb-2">Counter</h2>
            <p className="mb-4">Count: {count}</p>
            <button
                className="px-4 py-2 bg-primary text-white rounded text-base"
                onClick={() => setCount(count + 1)}
            >
                Increment
            </button>
        </div>
    )
}
