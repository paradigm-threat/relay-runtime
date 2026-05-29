import React, { useState } from 'react'

/** Minimal app — no lazy/Suspense, guaranteed visible UI for Android debug. */
export default function MinimalApp() {
  const [count, setCount] = useState(0)
  return (
    <div className="p-4">
      <h2>Relay Android Demo</h2>
      <span>Count: {count}</span>
      <button onClick={() => setCount(count + 1)}>Increment</button>
    </div>
  )
}
