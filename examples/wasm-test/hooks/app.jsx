// Main application hook - entry point for runtime-transpiled JSX
// CleverScript-compatible. Works on web and native (relay-runtime).
import React, { useState, useEffect, lazy, Suspense } from 'react'
import StyleContext from '@clevertree/style'

// Lazy load page components — paths must match asset keys in module map (e.g. hooks/home.jsx)
const Home = lazy(() => import('hooks/home.jsx'))
const Settings = lazy(() => import('hooks/settings.jsx'))
const PeerContent = lazy(() => import('hooks/peerContent.jsx'))
const Demo = lazy(() => import('hooks/demo.jsx'))
const UseStateDemo = lazy(() => import('hooks/use-state-demo.jsx'))
const StyleKeywords = lazy(() => import('hooks/style-keywords.jsx'))
const CleverScriptFeatures = lazy(() => import('hooks/cleverscript-features.jsx'))
const SimpleJsx = lazy(() => import('hooks/simple-jsx.jsx'))
const NestedStructure = lazy(() => import('hooks/nested-structure.jsx'))
const StringKeywords = lazy(() => import('hooks/string-keywords.jsx'))

const STORAGE_KEY_ACTIVE_TAB = 'relay:activeTab'

function LoadingFallback() {
    return (
        <div className="flex items-center justify-center h-full w-full">
            <div className="text-center">
                <p className="text-base">Loading...</p>
            </div>
        </div>
    )
}

export default function App() {
    return (
        <StyleContext src="theme.yaml">
            <AppMain />
        </StyleContext>
    )
}

function AppMain() {
    const getInitialState = () => {
        try {
            const storedTab = localStorage.getItem(STORAGE_KEY_ACTIVE_TAB)
            if (storedTab) {
                const parsed = JSON.parse(storedTab)
                const tabId = parsed.tabId
                const storedTabs = parsed.tabs
                if (storedTabs && Array.isArray(storedTabs) && storedTabs.length > 0) {
                    return { activeTab: tabId || 'home', tabs: storedTabs }
                }
            }
        } catch (e) {
            // fallback
        }
        return {
            activeTab: 'home',
            tabs: [
                { id: 'home', title: 'Home', isHome: true },
                { id: 'demo', title: 'Demo' },
                { id: 'styles', title: 'Styles' },
                { id: 'simple', title: 'Simple' },
                { id: 'nested', title: 'Nested' },
                { id: 'string', title: 'String' },
                { id: 'cleverscript', title: 'CleverScript' },
                { id: 'settings', title: 'Settings' }
            ]
        }
    }

    const initialState = getInitialState()
    const [activeTab, setActiveTab] = useState(initialState.activeTab)
    const [tabs, setTabs] = useState(initialState.tabs)

    useEffect(() => {
        try {
            localStorage.setItem(STORAGE_KEY_ACTIVE_TAB, JSON.stringify({ tabId: activeTab, tabs }))
        } catch (e) {
            // ignore
        }
    }, [activeTab, tabs])

    const openPeerTab = (host) => {
        const existingTab = tabs.find(t => t.host === host)
        if (existingTab) {
            setActiveTab(existingTab.id)
            return
        }
        const parts = host.split('//')
        const afterScheme = parts.length > 1 ? parts[1] : host
        const firstPart = afterScheme.split('/')[0]
        const title = firstPart || host
        const newTab = {
            id: 'peer-' + Date.now(),
            title,
            host,
            isPeer: true
        }
        const updatedTabs = tabs.concat([newTab])
        setTabs(updatedTabs)
        setActiveTab(newTab.id)
    }

    const closeTab = (e, tabId) => {
        if (e && e.stopPropagation) e.stopPropagation()
        const tabToClose = tabs.find(t => t.id === tabId)
        if (!tabToClose || tabToClose.isHome || tabId === 'settings') return

        const updatedTabs = tabs.filter(t => t.id !== tabId)
        setTabs(updatedTabs)

        if (activeTab === tabId) {
            setActiveTab('home')
        }
    }

    const handleTabClick = (tabId) => {
        setActiveTab(tabId)
    }

    const renderContent = () => {
        const currentTab = tabs.find(t => t.id === activeTab)

        if (currentTab && currentTab.isPeer) {
            return <PeerContent host={currentTab.host} />
        }

        if (activeTab === 'home') return <Home openPeerTab={openPeerTab} />
        if (activeTab === 'demo') return <Demo />
        if (activeTab === 'styles') return <StyleKeywords />
        if (activeTab === 'simple') return <SimpleJsx />
        if (activeTab === 'nested') return <NestedStructure />
        if (activeTab === 'string') return <StringKeywords />
        if (activeTab === 'cleverscript') return <CleverScriptFeatures />
        if (activeTab === 'settings') return <Settings />

        return (
            <div className="flex items-center justify-center h-full w-full">
                <div className="text-center">
                    <h2 className="mb-2 text-2xl font-semibold">No content</h2>
                    <p className="text-base">Select a tab</p>
                </div>
            </div>
        )
    }

    return (
        <div className="flex flex-col w-screen h-screen bg-primary theme">
            {/* Tab Bar */}
            <div className="border-b overflow-x-auto overflow-y-hidden">
                <div className="flex gap-1 p-0 min-h-11 items-center">
                    <div className="flex gap-1">
                        {tabs.map((tab) => {
                            const isActive = tab.id === activeTab
                            return (
                                <div
                                    key={tab.id}
                                    className={'flex items-center gap-2 px-4 py-2 border border-b-2 rounded-t-lg cursor-pointer transition-all flex-shrink-0 min-w-32 max-w-60 ' + (isActive
                                        ? 'border-b-blue-500 font-semibold bg-surface'
                                        : 'border-b-transparent')}
                                    onClick={() => handleTabClick(tab.id)}
                                >
                                    <span
                                        className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-sm"
                                        title={tab.title}
                                    >
                                        {tab.title}
                                    </span>
                                    {tab.isPeer && (
                                        <button
                                            onClick={(ev) => closeTab(ev, tab.id)}
                                            className="ml-1 text-gray-400 hover:text-red-600 text-xs"
                                            title="Close tab"
                                        >
                                            ✕
                                        </button>
                                    )}
                                </div>
                            )
                        })}
                    </div>
                </div>
            </div>

            {/* Main Content Area */}
            <div className="flex flex-1 overflow-hidden">
                <main className="flex-1 flex flex-col overflow-hidden">
                    <Suspense fallback={<LoadingFallback />}>
                        {renderContent()}
                    </Suspense>
                </main>
            </div>
        </div>
    )
}
