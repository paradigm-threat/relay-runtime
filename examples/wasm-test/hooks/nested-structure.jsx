/**
 * Nested structure — deep nesting, flex layout.
 * CleverScript-compatible.
 */
import React from 'react'

export default function NestedStructure() {
    return (
        <div className="p-4">
            <span className="text-2xl">Nested Structure</span>
            <div className="flex flex-row p-2 rounded bg-surface">
                <div className="p-3 rounded m-1 bg-primary flex-1">
                    <span>Box 1</span>
                </div>
                <div className="p-3 rounded m-1 bg-secondary flex-1">
                    <span>Box 2</span>
                </div>
                <div className="p-3 rounded m-1 bg-primary flex-1">
                    <span>Box 3</span>
                </div>
            </div>
        </div>
    )
}
