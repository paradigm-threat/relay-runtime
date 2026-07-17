// Scrollable page with all demos — CleverScript-compatible.
import React from 'react'
import StyleContext from '@clevertree/style'
import Home from 'hooks/home.jsx'
import Settings from 'hooks/settings.jsx'
import Demo from 'hooks/demo.jsx'
import UseStateDemo from 'hooks/use-state-demo.jsx'
import StyleKeywords from 'hooks/style-keywords.jsx'
import CleverScriptFeatures from 'hooks/cleverscript-features.jsx'
import SimpleJsx from 'hooks/simple-jsx.jsx'
import NestedStructure from 'hooks/nested-structure.jsx'
import StringKeywords from 'hooks/string-keywords.jsx'
import FlexDemo from 'hooks/flex-demo.jsx'

function Section(props) {
    const title = props.title
    const children = props.children
    return (
        <div className="test-section p-4 mb-4 border rounded bg-surface">
            <div className="test-header font-bold mb-2">{title}</div>
            <div className="test-output">{children}</div>
        </div>
    )
}

function noop() {}
export default function App() {
    return (
        <StyleContext src="theme.yaml">
            <AppScroll />
        </StyleContext>
    )
}

function AppScroll() {
    return (
        <div className="flex flex-col bg min-h-screen">
            <div className="p-4 border-b bg-primary rounded-lg">
                <h1 className="text-white font-bold mb-2">Relay Demo</h1>
                <p className="text-sm text-white m-0">All demos in one scrollable page</p>
            </div>
            <div className="p-4 bg flex-1">
                <Section title="Demo (Increment)"><Demo /></Section>
                <Section title="useState Demo"><UseStateDemo /></Section>
                <Section title="Style Keywords"><StyleKeywords /></Section>
                <Section title="Flex Layout"><FlexDemo /></Section>
                <Section title="Simple JSX"><SimpleJsx /></Section>
                <Section title="Nested Structure"><NestedStructure /></Section>
                <Section title="String Keywords"><StringKeywords /></Section>
                <Section title="CleverScript Features"><CleverScriptFeatures /></Section>
                <Section title="Home (Peers)"><Home openPeerTab={noop} /></Section>
                <Section title="Settings"><Settings /></Section>
            </div>
        </div>
    )
}
