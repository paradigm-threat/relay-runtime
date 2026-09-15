import React from 'react'
import { isClientLivePhase } from '@clevertree/style-internal'
import { pickFeelingLuckyPath, siteHref } from 'hooks/client/loadArticles.jsx'
import { IconHome } from 'hooks/client/components/Icons.jsx'
import { propOr } from 'hooks/client/propOr.jsx'

/**
 * Random article from QUERY `order=random` (files park only).
 * Prefer native navigate() host binding; fall back to <a href>.
 * Phase ≥4 — do not run the random QUERY during phase-2 DirNav (same idle
 * slice as catalog parse; freezes before deferred fill).
 * `dir={true}` is the Directory-column control (old-site “I'm Feeling Lucky”).
 */
export default function FeelingLucky(props) {
  const live = isClientLivePhase(4) === true
  const dir = propOr(props, 'dir', false) === true
  let cls = 'pt-btn pt-feeling-lucky pt-client-only'
  if (dir) {
    cls = cls + ' pt-feeling-lucky-dir'
  }

  return (
    <a
      className={cls}
      href="#"
      title={live ? 'Feeling Lucky' : 'Feeling Lucky available after page loads'}
      aria-disabled={live ? 'false' : 'true'}
      onClick={() => {
        if (!live) return
        const next = pickFeelingLuckyPath()
        if (next.length === 0) return
        const dest = siteHref('/' + next)
        try {
          navigate(dest)
        } catch (e) {
          try {
            const g = globalThis
            if (g && g.location) {
              g.location.href = dest
            }
          } catch (e2) {}
        }
      }}
    >
      {dir ? <IconHome /> : null}
      {dir ? "I'm Feeling Lucky" : 'Feeling Lucky'}
    </a>
  )
}
