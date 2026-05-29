// CleverScript-compatible Settings: theme selector, debug info
import React, { useState, useEffect } from 'react'

export default function Settings() {
    const [currentTheme, setCurrentTheme] = useState('dark')
    const [debugInfo, setDebugInfo] = useState(null)
    const [reactLibrary, setReactLibrary] = useState('react')

    useEffect(() => {
        const savedLibrary = localStorage.getItem('relay:react-library')
        if (savedLibrary) {
            setReactLibrary(savedLibrary)
        }
        setDebugInfo({
            runtime: 'relay-runtime',
            platform: 'native',
        })
    }, [])

    const handleThemeChange = (newTheme) => {
        setCurrentTheme(newTheme)
    }

    const handleReactLibraryChange = (newLibrary) => {
        setReactLibrary(newLibrary)
        localStorage.setItem('relay:react-library', newLibrary)
    }

    const themes = [
        { key: 'light', name: 'Light' },
        { key: 'dark', name: 'Dark' },
        { key: 'system', name: 'System' },
    ]

    return (
        <div className="p-6 max-w-4xl">
            <h1 className="text-2xl font-bold mb-6">Relay Settings</h1>
            <p className="text-sm text-gray-600 mb-6">Configuration for hooks and themes.</p>

            <div className="mb-8">
                <h2 className="text-xl font-semibold mb-4">Theme</h2>
                <div className="border rounded-lg p-4">
                    <div className="mb-4">
                        <label className="block text-sm font-medium mb-2">Appearance</label>
                        <div className="flex gap-2 flex-wrap">
                            {themes.map((theme) => (
                                <button
                                    key={theme.key}
                                    onClick={() => handleThemeChange(theme.key)}
                                    className={'px-4 py-2 rounded ' + (currentTheme === theme.key ? 'bg-blue-500 text-white' : 'bg-gray-200')}
                                >
                                    {theme.name}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>
            </div>

            <div className="mb-8">
                <h2 className="text-xl font-semibold mb-4">Debug</h2>
                <div className="border rounded-lg p-4 bg-gray-50">
                    {debugInfo ? (
                        <div className="space-y-2 text-xs font-mono">
                            <div>Runtime: {debugInfo.runtime}</div>
                            <div>Platform: {debugInfo.platform}</div>
                        </div>
                    ) : (
                        <p className="text-sm text-gray-500">Loading...</p>
                    )}
                </div>
            </div>
        </div>
    )
}
