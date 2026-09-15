/**
 * Directory listing when hub page.md is missing (old CatchAll Categories + Documents).
 * Listing rows from `@relay/catalog.dirIndexListing` over warm loadDirTree.
 * Phase ≥3 fill; SSR shows placeholder (no QUERY).
 */
import React from 'react'
import { isClientLivePhase } from '@clevertree/style-internal'
import {
  autoIndexDirFromPath as hostAutoIndexDirFromPath,
  shouldTryDirectoryIndex as hostShouldTryDirectoryIndex,
  dirTreeNode,
  dirTreeHasKids,
  dirIndexListing,
} from '@relay/catalog'
import { loadDirTree, siteHref } from 'hooks/client/loadArticles.jsx'
import { propStr } from 'hooks/client/propOr.jsx'

function stripRel(raw) {
  let s = typeof raw === 'string' ? raw : ''
  while (s.length > 0 && s.charAt(0) === '/') {
    s = s.slice(1)
  }
  return s
}

export function autoIndexDirFromPath(pathPart) {
  try {
    const d = hostAutoIndexDirFromPath(pathPart)
    return typeof d === 'string' ? d : ''
  } catch (e) {
    return ''
  }
}

export function shouldTryDirectoryIndex(pathPart) {
  try {
    return hostShouldTryDirectoryIndex(pathPart) === true
  } catch (e) {
    return false
  }
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
      const node = dirTreeNode(loadDirTree(), dir)
      if (dirTreeHasKids(node) === true) return dir
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

  let title = 'Directory'
  if (dir.length > 0) {
    const slash = dir.lastIndexOf('/')
    const leaf = slash >= 0 ? dir.slice(slash + 1) : dir
    title = leaf.replaceAll('_', ' ')
    if (title.length === 0) title = 'Directory'
  }

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

  let listing = null
  try {
    listing = dirIndexListing(loadDirTree(), dir, { maxDocs: 48 })
  } catch (e) {
    listing = null
  }

  let listTitle = title
  let categories = []
  let documents = []
  try {
    if (listing && typeof listing.title === 'string' && listing.title.length > 0) {
      listTitle = listing.title
    }
  } catch (eT) {}
  try {
    if (listing && listing.categories) categories = listing.categories
  } catch (eC) {}
  try {
    if (listing && listing.documents) documents = listing.documents
  } catch (eD) {}

  let catEls = []
  for (let i = 0; i < categories.length; i++) {
    let path = ''
    let label = ''
    let count = 0
    try {
      if (categories[i] && typeof categories[i].path === 'string') path = categories[i].path
    } catch (e1) {}
    try {
      if (categories[i] && typeof categories[i].label === 'string') label = categories[i].label
    } catch (e2) {}
    try {
      if (categories[i] && typeof categories[i].count === 'number') count = categories[i].count
    } catch (e3) {}
    if (path.length === 0) continue
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
  for (let j = 0; j < documents.length; j++) {
    let p = ''
    let docTitle = ''
    try {
      if (documents[j] && typeof documents[j].path === 'string') p = documents[j].path
    } catch (e4) {}
    try {
      if (documents[j] && typeof documents[j].title === 'string') docTitle = documents[j].title
    } catch (e5) {}
    if (p.length === 0) continue
    docEls = docEls.concat([
      <li key={'di-d-' + j} className="pt-dir-index-item pt-dir-index-doc">
        <a href={siteHref('/' + p)}>{docTitle}</a>
      </li>,
    ])
  }

  const empty = catEls.length === 0 && docEls.length === 0

  return (
    <div
      className="pt-dir-index"
      data-pt-dir-index="1"
      data-pt-dir-index-live="1"
      aria-label="Auto index"
    >
      <h1 className="pt-dir-index-title">{listTitle}</h1>
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
