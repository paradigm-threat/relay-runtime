/**
 * CleverScript features: ??, ?., while.
 * No template literals - use string concatenation.
 */
import React from 'react'

export default function CleverScriptFeatures() {
    const a = null ?? 'default'
    const b = 0 ?? 'fallback'
    const c = '' ?? 'empty-fallback'

    const obj = { x: 1, nested: { y: 2 } }
    const d = obj?.x
    const e = obj?.nested?.y
    const f = null?.prop ?? 'null'

    let sum = 0
    let i = 0
    while (i < 5) {
        sum = sum + i
        i++
    }

    return (
        <div className="cleverscript-features p-4">
            <h2 className="text-lg font-semibold mb-2">CleverScript Features</h2>
            <div className="mb-2">?? nullish: a={a}, b={b}, c={c}</div>
            <div className="mb-2">?. optional: d={d}, e={e}, f={f}</div>
            <div>while sum(0-4)={sum}</div>
        </div>
    )
}
