/**
 * Directory listing when hub page.md is missing (old CatchAll Categories + Documents).
 * Uses warm loadDirTree (DirNav QUERY) — O(folder children), not a fat index.json.
 * Phase ≥2; SSR shows placeholder (no QUERY).
 */
import React from 'react'
import { isClientLivePhase } from '@clevertree/style-internal'
import { loadDirTree, rowPath, rowTitle, siteHref } from 'hooks/client/loadArticles.jsx'
import { propStr } from 'hooks/client/propOr.jsx'

function stripRel(raw) {
  let s = typeof raw === 'string' ? raw : ''
  while (s.length > 0 && s.charAt(0) === '/') {
    s = s.slice(1)
  }
  return s
}

function folderTitle(name) {
  if (typeof name !== 'string') return ''
  return name.replaceAll('_', ' ')
}

function skipFileName(name) {
  if (typeof name !== 'string') return true
  if (name === 'page.md' || name === 'page.mdx' || name === 'README.md') return true
  if (name.indexOf('.auto.md') >= 0) return true
  return false
}

/** Walk buildDirTree root to folder path ('' = site root folders/files). */
export function findDirTreeNode(tree, dirRel) {
  const dir = stripRel(dirRel)
  if (!tree) return null
  if (dir.length === 0) return tree
  const parts = dir.split('/')
  let node = tree
  for (let i = 0; i < parts.length; i++) {
    const want = parts[i]
    if (want.length === 0) continue
    let folders = []
    try {
      if (node && node.folders) folders = node.folders
    } catch (e) {}
    let next = null
    for (let j = 0; j < folders.length; j++) {
      let n = ''
      try {
        if (folders[j] && typeof folders[j].name === 'string') n = folders[j].name
      } catch (e2) {}
      if (n === want) {
        next = folders[j]
        break
      }
    }
    if (!next) return null
    node = next
  }
  return node
}

export function dirTreeHasKids(node) {
  if (!node) return false
  let nf = 0
  let nd = 0
  try {
    if (node.files) nf = node.files.length
  } catch (e) {}
  try {
    if (node.folders) nd = node.folders.length
  } catch (e2) {}
  return nf + nd > 0
}

/**
 * Directory to auto-index from a failed article path.
 * `events/false_flag/page.md` → `events/false_flag`
 * `events/false_flag.md` → `events/false_flag` when that folder exists in tree
 */
export function autoIndexDirFromPath(pathPart) {
  let p = stripRel(pathPart)
  if (p.length === 0) return ''
  if (p === 'page.md' || p === 'index.md' || p === 'README.md') return ''
  if (p.length > 8 && p.slice(p.length - 8) === '/page.md') {
    return p.slice(0, p.length - 8)
  }
  if (p.length > 3 && p.slice(p.length - 3) === '.md') {
    return p.slice(0, p.length - 3)
  }
  if (p.indexOf('.') < 0) return p
  return ''
}

export function shouldTryDirectoryIndex(pathPart) {
  // Root page.md miss is a real error — not a directory index.
  if (stripRel(pathPart) === 'page.md') return false
  const d = autoIndexDirFromPath(pathPart)
  return d.length > 0
}

/**
 * After MD miss: use warm tree when phase ≥2; optimistic placeholder when cold + hub path.
 */
export function resolveDirectoryIndexDir(pathPart) {
  if (!shouldTryDirectoryIndex(pathPart)) return ''
  const dir = autoIndexDirFromPath(pathPart)
  if (dir.length === 0) return ''
  if (isClientLivePhase(2) === true) {
    try {
      const tree = loadDirTree()
      const node = findDirTreeNode(tree, dir)
      if (dirTreeHasKids(node)) return dir
      return ''
    } catch (e) {
      return ''
    }
  }
  // SSR / pre-catalog: optimistic hub shell for …/page.md (gateway Tree → 200).
  const raw = stripRel(pathPart)
  if (raw.length > 8 && raw.slice(raw.length - 8) === '/page.md') return dir
  return ''
}

export default function DirectoryIndex(props) {
  const dir = stripRel(propStr(props, 'path', ''))
  // Phase 3 = feeds / AutoIndex (see HYDRATE_PHASES). Tree warms at phase 2.
  const liveCatalog = isClientLivePhase(3) === true

  let title = folderTitle(dir)
  const slash = dir.lastIndexOf('/')
  if (slash >= 0) {
    title = folderTitle(dir.slice(slash + 1))
  }
  if (title.length === 0) title = 'Directory'

  if (liveCatalog !== true) {
    return (
      <div
        className="pt-dir-index"
        data-pt-dir-index="1"
        data-pt-autoindex-ssr="1"
        aria-label="Auto index"
      >
        <h1 className="pt-dir-index-title">{title}</h1>
        <p className="pt-muted">Folder index loads after page open…</p>
      </div>
    )
  }

  let node = null
  try {
    node = findDirTreeNode(loadDirTree(), dir)
  } catch (e) {
    node = null
  }

  let folders = []
  let files = []
  try {
    if (node && node.folders) folders = node.folders
  } catch (e2) {}
  try {
    if (node && node.files) files = node.files
  } catch (e3) {}

  let catEls = []
  for (let i = 0; i < folders.length; i++) {
    let name = ''
    let path = ''
    let count = 0
    try {
      if (folders[i] && typeof folders[i].name === 'string') name = folders[i].name
    } catch (e4) {}
    try {
      if (folders[i] && typeof folders[i].path === 'string') path = folders[i].path
    } catch (e5) {}
    try {
      if (folders[i] && typeof folders[i].count === 'number') count = folders[i].count
    } catch (e6) {}
    if (path.length === 0) continue
    const label = folderTitle(name)
    const countLabel = count > 0 ? ' (' + count + ')' : ''
    catEls = catEls.concat([
      <li key={'di-c-' + i} className="pt-dir-index-item pt-dir-index-cat">
        <a href={siteHref('/' + path + '/page.md')}>
          {label}
          {countLabel}
        </a>
      </li>,
    ])
  }

  let docEls = []
  let docN = 0
  for (let j = 0; j < files.length; j++) {
    const p = rowPath(files[j])
    if (p.length === 0) continue
    const slashF = p.lastIndexOf('/')
    const base = slashF >= 0 ? p.slice(slashF + 1) : p
    if (skipFileName(base)) continue
    if (docN >= 48) break
    docEls = docEls.concat([
      <li key={'di-d-' + j} className="pt-dir-index-item pt-dir-index-doc">
        <a href={siteHref('/' + p)}>{rowTitle(files[j])}</a>
      </li>,
    ])
    docN = docN + 1
  }

  const empty = catEls.length === 0 && docEls.length === 0

  return (
    <div
      className="pt-dir-index"
      data-pt-dir-index="1"
      data-pt-dir-index-live="1"
      aria-label="Auto index"
    >
      <h1 className="pt-dir-index-title">{title}</h1>
      {empty ? (
        <p className="pt-muted">No indexed articles in this folder.</p>
      ) : (
        <div className="pt-dir-index-body">
          {catEls.length > 0 ? (
            <section className="pt-dir-index-section" aria-label="Categories">
              <h2 className="pt-dir-index-heading">Categories</h2>
              <ul className="pt-dir-index-list">{catEls}</ul>
            </section>
          ) : null}
          {docEls.length > 0 ? (
            <section className="pt-dir-index-section" aria-label="Documents">
              <h2 className="pt-dir-index-heading">Documents</h2>
              <ul className="pt-dir-index-list">{docEls}</ul>
            </section>
          ) : null}
        </div>
      )}
    </div>
  )
}
