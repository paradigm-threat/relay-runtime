// @use-jsx
/**
 * Paradigm Threat — files corpus GET hook. Fetches repo-relative .md via Host.fetch; markdown links use anchorOnClick + relayControlled (runtime).
 * Note: CleverScript has no global `String()` — coerce with '' + value.
 */
import React, { useState } from 'react'
import { MarkdownRenderer } from '@clevertree/markdown'
import Layout from 'hooks/client/components/Layout.jsx'
import manifest from '@hooks'

/** Coerce to string for `.trim` / `.indexOf`; truthy non-strings (e.g. numbers) must not pass through. */
function str(v) {
  if (v === null) return ''
  if (typeof v === 'boolean') {
    if (v) return 'true'
    return 'false'
  }
  if (typeof v === 'number') return '' + v
  if (typeof v === 'string') return v
  return ''
}

function normalizeRelPath(s) {
  let t = str(s).trim()
  while (t.length > 0 && t.charAt(0) === '/') {
    t = t.slice(1)
  }
  if (t.length === 0) return 'README.md'
  return t
}

function resolveMdHref(href, currentPath, slash, baseUrl) {
  const h = str(href)
  if (h.length === 0) return currentPath
  if (h.indexOf('mailto:') === 0 || h.indexOf('tel:') === 0) return currentPath
  if (h.charAt(0) === '#') return currentPath
  if (h.indexOf('://') >= 0) {
    if (baseUrl && h.indexOf(str(baseUrl)) === 0) {
      const baseLen = str(baseUrl).length
      let rest = h.slice(baseLen)
      while (rest.length > 0 && rest.charAt(0) === '/') {
        rest = rest.slice(1)
      }
      return normalizeRelPath(rest)
    }
    return normalizeRelPath(h)
  }
  let rel = h
  if (rel.indexOf('./') === 0) rel = rel.slice(2)
  let dir = str(currentPath)
  const lio = dir.lastIndexOf(slash)
  if (lio >= 0) dir = dir.slice(0, lio)
  else dir = ''
  const parts = rel.split(slash)
  const segs = []
  if (dir.length > 0) {
    const dparts = dir.split(slash)
    for (let i = 0; i < dparts.length; i++) {
      if (dparts[i].length > 0) segs.push(dparts[i])
    }
  }
  for (let j = 0; j < parts.length; j++) {
    const p = parts[j]
    if (p.length === 0 || p === '.') {
      /* skip path segment */
    } else if (p === '..') {
      if (segs.length > 0) segs.pop()
    } else {
      segs.push(p)
    }
  }
  let out = ''
  for (let k = 0; k < segs.length; k++) {
    if (k > 0) out = out + slash
    out = out + segs[k]
  }
  return normalizeRelPath(out)
}

function parentDir(relPath, slash) {
  const p = str(relPath)
  const i = p.lastIndexOf(slash)
  if (i <= 0) return 'README.md'
  const up = p.slice(0, i)
  if (up.length === 0) return 'README.md'
  return normalizeRelPath(up)
}

export default function ParadigmFilesRelayRoot() {
  const rp = useState('README.md')
  const routePath = rp[0]
  const setRoute = rp[1]

  const slash = manifest.relayPathSlash ? manifest.relayPathSlash : '/'

  function navigate(to) {
    setRoute(normalizeRelPath(to))
  }

  function onMdLinkClick(href) {
    const resolved = resolveMdHref(href, routePath, slash, manifest.relayClientBaseUrl)
    navigate(resolved)
  }

  let pathPart = normalizeRelPath(routePath)
  if (!pathPart || pathPart.length === 0) {
    pathPart = 'README.md'
  }
  let markdownContent = ''
  let errMsg = ''
  /* Avoid str(): CleverScript bug drops Some member reads when passed through str() before fetch path. */
  let baseStr =
    typeof manifest.relayClientBaseUrl === 'string' ? manifest.relayClientBaseUrl : str(manifest.relayClientBaseUrl)
  while (baseStr.length > 0 && baseStr.charAt(baseStr.length - 1) === '/') {
    baseStr = baseStr.slice(0, baseStr.length - 1)
  }
  if (!manifest.relayClientBaseUrl || baseStr.length === 0) {
    errMsg = 'Missing relayClientBaseUrl — open this hook with a remote host preset (renderHook host URL).'
  } else {
    const url = baseStr + slash + pathPart
    const resp = fetch(url, { method: 'GET' })
    if (resp && resp.ok === true) {
      markdownContent = resp.text ? resp.text : ''
    } else {
      let st = 0
      if (resp && resp.status) {
        st = resp.status
      }
      errMsg = 'Could not load file (HTTP ' + st + ')'
    }
  }

  const body = errMsg ? '# Error\n\n' + errMsg : markdownContent

  return (
    <div className="min-h-screen flex flex-col bg-[var(--bg-primary,#fafafa)] text-[var(--text-primary,#111)]">
      <header className="border-b border-[var(--border-color,#e5e7eb)] px-4 py-3 shadow-sm flex-shrink-0">
        <h1 className="text-xl font-semibold tracking-tight m-0">Paradigm Threat</h1>
        <p className="text-sm opacity-70 m-0 mt-1">Files — markdown corpus</p>
      </header>
      <main className="flex-1 flex flex-col overflow-hidden">
        <Layout
          filePath={routePath}
          onUp={() => navigate(parentDir(routePath, slash))}
          onHome={() => navigate('README.md')}
        >
          <MarkdownRenderer content={body} navigate={navigate} anchorOnClick={onMdLinkClick} />
        </Layout>
      </main>
    </div>
  )
}
