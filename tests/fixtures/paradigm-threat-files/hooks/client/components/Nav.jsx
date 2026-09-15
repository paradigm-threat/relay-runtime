/**
 * Minimal Nav fixture for layer-1 menu open/close tests.
 * Full tip Nav.jsx (huge navChromeCss) overflows the CS stack in unit tests;
 * behavior under test is useState menu + aria-labels only.
 */
import React, { useState } from 'react'
import { isClientLive } from '@clevertree/style-internal'
import { propStr } from 'hooks/client/propOr.jsx'

export default function Nav(props) {
  const currentPath = propStr(props, 'currentPath', '/')
  const live = isClientLive() === true
  const menu = useState(false)
  const isOpen = menu[0]
  const setOpen = menu[1]

  function toggleMenu() {
    if (live !== true) return
    if (isOpen === true) {
      setOpen(false)
    } else {
      setOpen(true)
    }
  }

  return (
    <header className="pt-site-nav" data-site-navbar="true" data-path={currentPath}>
      <button
        type="button"
        className="pt-icon-btn pt-nav-menu-btn"
        aria-label={isOpen ? 'Close menu' : 'Open menu'}
        aria-expanded={isOpen ? 'true' : 'false'}
        onClick={toggleMenu}
      >
        Menu
      </button>
      {isOpen ? (
        <div className="pt-nav-mobile" role="dialog" aria-label="Mobile menu">
          <nav aria-label="Mobile primary">links</nav>
        </div>
      ) : null}
    </header>
  )
}
