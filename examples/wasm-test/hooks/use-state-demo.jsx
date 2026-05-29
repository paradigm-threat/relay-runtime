/**
 * useState demo — displays count; Increment button.
 * CleverScript-compatible.
 */
import React, { useState } from 'react'

export default function UseStateDemo() {
    const [count, setCount] = useState(0)
    return (
        <div className="p-4 bg-surface rounded">
            <h2 className="text-lg font-semibold mb-2">useState Demo</h2>
            <span className="mr-2">Count: {count}</span>
            <button
                className="px-3 py-1 bg-primary text-white rounded"
                onClick={() => setCount(count + 1)}
            >
                Increment
            </button>
        </div>
    )
}
