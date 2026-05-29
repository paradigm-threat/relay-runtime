/**
 * Style keywords — className with multiple classes.
 * CleverScript-compatible.
 */
import React from 'react'

export default function StyleKeywords() {
    return (
        <div className="w-full h-auto p-4 gap-4 bg-surface">
            <div className="bg-primary p-4 w-full rounded">
                <h1 className="text-white">Style Keywords Test</h1>
                <p className="text-white text-sm opacity-80">Verifying className mapping</p>
            </div>
            <div className="flex flex-col gap-4">
                <div className="p-4 bg-white rounded border border-themed h-fit">
                    <span className="font-bold">h-fit container</span>
                    <p className="text-sm text-muted">Content wraps tightly</p>
                </div>
                <div className="p-4 bg-white rounded border border-themed w-fit">
                    <span className="font-bold">w-fit</span>
                </div>
            </div>
        </div>
    )
}
