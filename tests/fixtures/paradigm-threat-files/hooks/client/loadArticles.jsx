import manifest from '@hooks'
import { pickFeelingLuckyPath as hostPickLucky, buildDirTree } from '@relay/catalog'
import { siteHref as hostSiteHref } from '@relay/path'
import { take } from '@relay/util'
import { parseJson, queryIndexBody } from 'hooks/client/jsonUtil.jsx'

function clientBase() {
  try {
    if (typeof manifest.relayClientBaseUrl === 'string') {
      let b = manifest.relayClientBaseUrl
      while (b.length > 0 && b.charAt(b.length - 1) === '/') {
        b = b.slice(0, b.length - 1)
      }
      return b
    }
  } catch (e) {}
  return ''
}

/** Match SSR HTML href rewriting (`asset_base_url` + relative path). */
export function siteHref(path) {
  return hostSiteHref(clientBase(), path)
}

export function rowPath(row) {
  if (!row) return ''
  try {
    if (row.path && typeof row.path === 'string') return row.path
  } catch (e) {}
  return ''
}

export function rowTitle(row) {
  if (!row) return 'Untitled'
  try {
    if (row._title && typeof row._title === 'string') return row._title
  } catch (e) {}
  try {
    if (row.title && typeof row.title === 'string') return row.title
  } catch (e) {}
  const p = rowPath(row)
  if (p.length > 0) return p
  return 'Untitled'
}

export function rowUpdated(row) {
  if (!row) return ''
  try {
    if (row._updated && typeof row._updated === 'string') return row._updated
  } catch (e) {}
  return ''
}

export function rowKind(row) {
  if (!row) return ''
  try {
    if (row.content_kind && typeof row.content_kind === 'string') return row.content_kind
  } catch (e) {}
  return ''
}

export function rowPublished(row) {
  if (!row) return ''
  try {
    if (row._originalPublishedAt && typeof row._originalPublishedAt === 'string') {
      return row._originalPublishedAt
    }
  } catch (e) {}
  return ''
}

export function rowImageSrc(row) {
  if (!row) return ''
  try {
    if (row.image_src && typeof row.image_src === 'string') return row.image_src
  } catch (e) {}
  return ''
}

/**
 * Park ChangeLog `<img src>` — same-Host relative + optional `?w=160` (Vercel parity).
 * Keeps `/site/*` chrome assets untouched; skips width for SVG.
 */
export function changelogThumbSrc(src) {
  if (!src || typeof src !== 'string') return ''
  let s = src.trim()
  if (s.length === 0) return ''
  if (s.indexOf('http://') === 0 || s.indexOf('https://') === 0) return s
  if (s.charAt(0) !== '/') s = '/' + s
  const q = s.indexOf('?')
  let pathOnly = s
  if (q >= 0) pathOnly = s.slice(0, q)
  if (pathOnly.indexOf('/site/') === 0) return pathOnly
  if (q >= 0) return s
  let ext = ''
  let i = pathOnly.length - 1
  while (i >= 0) {
    const c = pathOnly.charAt(i)
    if (c === '.') {
      ext = pathOnly.slice(i + 1)
      break
    }
    if (c === '/') break
    i = i - 1
  }
  if (ext === 'svg' || ext === 'SVG') return pathOnly
  return pathOnly + '?w=160'
}

const KIND_ARTICLE_INV_BLOG = ['article', 'investigation', 'blog']
const DIR_QUERY = { content_kind: { $ne: 'exclude' } }

let dirRowsWarm = null
let dirTreeWarm = null
let articlesRowsWarm = null
let investigationsRowsWarm = null
let blogRowsWarm = null
let recentRowsWarm = null
let luckyRowsWarm = null
let homeRowsWarm = null

function rowsFromQueryJson(data) {
  // Return the host array as-is. CleverScript `concat` in a 600-row loop is
  // O(n²) and freezes the main thread for seconds during phase-2 DirNav.
  if (!data || !data.results) return []
  return data.results
}

/** Minimal CSV line split (handles quoted commas). CleverScript-safe. */
function splitCsvLine(line) {
  let fields = []
  let cur = ''
  let inQ = false
  for (let i = 0; i < line.length; i++) {
    const ch = line.charAt(i)
    if (inQ) {
      if (ch === '"') {
        if (i + 1 < line.length && line.charAt(i + 1) === '"') {
          cur = cur + '"'
          i = i + 1
        } else {
          inQ = false
        }
      } else {
        cur = cur + ch
      }
    } else if (ch === '"') {
      inQ = true
    } else if (ch === ',') {
      fields = fields.concat([cur])
      cur = ''
    } else {
      cur = cur + ch
    }
  }
  fields = fields.concat([cur])
  return fields
}

/** Minimal CSV QUERY fallback when Accept:json is dropped by the host fetch. */
function rowsFromQueryCsv(text) {
  if (!text || text.length < 8) return []
  const lines = text.split('\n')
  if (lines.length < 2) return []
  const cols = splitCsvLine(lines[0])
  let pathIdx = 0 - 1
  let kindIdx = 0 - 1
  let titleIdx = 0 - 1
  for (let c = 0; c < cols.length; c++) {
    const name = cols[c]
    if (name === 'path') pathIdx = c
    if (name === 'content_kind') kindIdx = c
    if (name === 'title') titleIdx = c
  }
  if (pathIdx < 0) return []
  let rows = []
  for (let li = 1; li < lines.length; li++) {
    const line = lines[li]
    if (!line || line.length === 0) continue
    const parts = splitCsvLine(line)
    if (parts.length <= pathIdx) continue
    const path = parts[pathIdx]
    if (!path || path.length === 0) continue
    let kind = ''
    if (kindIdx >= 0 && kindIdx < parts.length) kind = parts[kindIdx]
    if (kind === 'exclude') continue
    let title = path
    if (titleIdx >= 0 && titleIdx < parts.length && parts[titleIdx].length > 0) {
      title = parts[titleIdx]
    }
    rows = rows.concat([
      {
        path: path,
        title: title,
        content_kind: kind,
      },
    ])
  }
  return rows
}

function fetchIndexQuery(query, order, limit) {
  let resp = null
  try {
    // CleverScript object keys must be identifiers (not string literals).
    // Lowercase `accept` — some hosts only forward canonicalized header maps.
    resp = fetch('/', {
      method: 'QUERY',
      headers: {
        accept: 'application/json',
      },
      body: queryIndexBody(query, limit, order),
    })
  } catch (e) {
    return []
  }
  if (!resp || resp.ok !== true) return []
  let text = resp.text ? resp.text : ''
  if (text.length === 0) return []
  const trimmed = text.charAt(0) === '\uFEFF' ? text.slice(1) : text
  if (trimmed.charAt(0) === '{' || trimmed.charAt(0) === '[') {
    const data = parseJson(text)
    return rowsFromQueryJson(data)
  }
  // Accept dropped → text/csv. Still build a directory from path/title columns.
  return rowsFromQueryCsv(trimmed)
}

function takeRows(rows, limit) {
  if (!rows) return []
  if (typeof limit !== 'number' || limit <= 0) return rows
  return take(rows, limit)
}

/**
 * Path-ordered catalog rows (`content_kind ≠ exclude`). Shared by DirNav, neighbors, AutoIndex.
 * Warm only successful non-empty results so a failed first tick can retry.
 */
export function loadDirCatalogRows() {
  if (dirRowsWarm !== null && dirRowsWarm.length > 0) return dirRowsWarm
  const rows = fetchIndexQuery(DIR_QUERY, 'path', 10000)
  if (rows.length > 0) {
    dirRowsWarm = rows
  }
  return rows
}

/** Warm `@relay/catalog.buildDirTree` once per session (phase-2+ DirNav). */
export function loadDirTree() {
  if (dirTreeWarm !== null) return dirTreeWarm
  const rows = loadDirCatalogRows()
  if (!(rows && rows.length > 0)) return null
  try {
    dirTreeWarm = buildDirTree(rows)
  } catch (e) {
    dirTreeWarm = null
  }
  return dirTreeWarm
}

/**
 * Recency QUERY for Index / Home lists. Server order is `-_updated`.
 */
export function loadArticleRows(limit) {
  let lim = 48
  if (typeof limit === 'number' && limit > 0) lim = limit
  if (homeRowsWarm === null) {
    homeRowsWarm = fetchIndexQuery(
      { content_kind: KIND_ARTICLE_INV_BLOG },
      '-_updated',
      80,
    )
  }
  return takeRows(homeRowsWarm, lim)
}

/** ChangeLog tab QUERY: kind filter + `-_updated` + limit 8. */
export function loadCatalogKind(kind) {
  if (kind === 'article') {
    if (articlesRowsWarm === null) {
      articlesRowsWarm = fetchIndexQuery({ content_kind: 'article' }, '-_updated', 8)
    }
    return articlesRowsWarm
  }
  if (kind === 'investigation') {
    if (investigationsRowsWarm === null) {
      investigationsRowsWarm = fetchIndexQuery(
        { content_kind: 'investigation' },
        '-_updated',
        8,
      )
    }
    return investigationsRowsWarm
  }
  if (kind === 'blog') {
    if (blogRowsWarm === null) {
      blogRowsWarm = fetchIndexQuery({ content_kind: 'blog' }, '-_updated', 8)
    }
    return blogRowsWarm
  }
  if (kind === 'recent') {
    if (recentRowsWarm === null) {
      recentRowsWarm = fetchIndexQuery(
        { content_kind: KIND_ARTICLE_INV_BLOG },
        '-_updated',
        8,
      )
    }
    return recentRowsWarm
  }
  return []
}

/** Feeling Lucky: `order=random` candidate set (one QUERY per session). */
export function loadLuckyRows() {
  if (luckyRowsWarm !== null) return luckyRowsWarm
  luckyRowsWarm = fetchIndexQuery(
    { content_kind: KIND_ARTICLE_INV_BLOG },
    'random',
    40,
  )
  return luckyRowsWarm
}

export function featuredArticlePath(rows) {
  for (let i = 0; i < rows.length; i++) {
    const p = rowPath(rows[i])
    if (p.indexOf('blog/') === 0) return p
  }
  for (let j = 0; j < rows.length; j++) {
    const p2 = rowPath(rows[j])
    if (p2 !== 'README.md' && p2.length > 0) return p2
  }
  return ''
}

/**
 * Pick a rotating article path from the warmed random QUERY set.
 */
export function pickFeelingLuckyPath() {
  const rows = loadLuckyRows()
  let seed = 0
  try {
    const g = globalThis
    if (g && g.location && typeof g.location.pathname === 'string') {
      seed = g.location.pathname.length
    }
    if (g && g.performance && typeof g.performance.now === 'function') {
      const n = g.performance.now()
      if (typeof n === 'number') seed = seed + n
    }
  } catch (e) {
    seed = 0
  }
  return hostPickLucky(rows, seed)
}
