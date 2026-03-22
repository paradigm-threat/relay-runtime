/**
 * Simple tabbed app — no lazy/Suspense, no .map (CleverScript uses for-loops).
 * Safe for relay-runtime Android.
 */
import React, { useState } from 'react'

export default function AppSimple() {
  const [activeTab, setActiveTab] = useState('home')

  return (
    <div className="p-4">
      <h2>Relay Demo</h2>
      <div>
        <button
          className={activeTab === 'home' ? 'font-semibold' : ''}
          onClick={() => setActiveTab('home')}
        >
          Home
        </button>
        <button
          className={activeTab === 'settings' ? 'font-semibold' : ''}
          onClick={() => setActiveTab('settings')}
        >
          Settings
        </button>
      </div>
      {activeTab === 'home' ? (
        <p>Home — add peers, probe nodes.</p>
      ) : (
        <p>Settings — theme, debug info.</p>
      )}
    </div>
  )
}
