/**
 * Flex layout demo — flexDirection, gap, row, column.
 * CleverScript-compatible.
 */
import React from 'react'

function Chip(props) {
    const label = props.label
    return (
        <div className="px-3 py-1 rounded bg-primary text-white text-sm">
            {label}
        </div>
    )
}

export default function FlexDemo() {
    return (
        <div className="flex flex-col gap-4">
            <div>
                <p className="text-sm text-muted mb-2">flex-row gap-2 (horizontal)</p>
                <div className="flex flex-row gap-2">
                    <Chip label="One" />
                    <Chip label="Two" />
                    <Chip label="Three" />
                </div>
            </div>
            <div>
                <p className="text-sm text-muted mb-2">flex-col gap-2 (vertical)</p>
                <div className="flex flex-col gap-2">
                    <div className="p-2 rounded border border-themed">Item A</div>
                    <div className="p-2 rounded border border-themed">Item B</div>
                    <div className="p-2 rounded border border-themed">Item C</div>
                </div>
            </div>
        </div>
    )
}
