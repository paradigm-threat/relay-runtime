import React, { useState } from 'react'

/**
 * useState demo — static display (relay-runtime supports useState)
 * Note: onClick not wired in WASM output; button is for structure validation.
 */
export default function UseStateDemo() {
  const [count, setCount] = useState(0)
  return (
    <div className="p-4 bg-surface rounded">
      <h2 className="text-lg font-semibold mb-2">useState Demo</h2>
      <span className="mr-2">Count: 0</span>
      <button className="px-3 py-1 bg-primary text-white rounded">
        Increment
      </button>
    </div>
  )
}
