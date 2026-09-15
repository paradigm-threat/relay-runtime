/**
 * Apex park does not host Next-only app shells. Rewrite those hrefs so
 * markdown and in-park clicks land on the archive or the live Relay apps.
 *
 * old.paradigmthreat.net: /submit /mcp /api/mcp /reply /search/{topic} /chat/{channel}
 * timeline.paradigmthreat.net: /timeline…
 * chat.paradigmthreat.net: /chat (join)
 * Bare /search stays on park.
 */
const ARCHIVE = 'https://old.paradigmthreat.net'
const TIMELINE = 'https://timeline.paradigmthreat.net'
const CHAT = 'https://chat.paradigmthreat.net'
const APEX = 'https://paradigmthreat.net'
const APEX_WWW = 'https://www.paradigmthreat.net'

function stripApex(href) {
  if (href === APEX || href === APEX + '/') return '/'
  if (href.indexOf(APEX + '/') === 0) return href.slice(APEX.length)
  if (href === APEX_WWW || href === APEX_WWW + '/') return '/'
  if (href.indexOf(APEX_WWW + '/') === 0) return href.slice(APEX_WWW.length)
  return href
}

function pathBase(path) {
  let end = path.length
  const q = path.indexOf('?')
  const hash = path.indexOf('#')
  if (q >= 0 && q < end) end = q
  if (hash >= 0 && hash < end) end = hash
  return path.slice(0, end)
}

export function rewriteLegacyAppHref(href) {
  if (typeof href !== 'string' || href.length === 0) return href
  if (href.indexOf(ARCHIVE) === 0) return href
  if (href.indexOf(TIMELINE) === 0) return href
  if (href.indexOf(CHAT) === 0) return href

  const isApex =
    href === APEX ||
    href === APEX + '/' ||
    href.indexOf(APEX + '/') === 0 ||
    href === APEX_WWW ||
    href === APEX_WWW + '/' ||
    href.indexOf(APEX_WWW + '/') === 0
  const isRoot = href.charAt(0) === '/' && href.indexOf('//') !== 0
  if (!isApex && !isRoot) return href

  const path = isApex ? stripApex(href) : href
  const base = pathBase(path)
  const rest = path.slice(base.length)

  if (base === '/submit' || base.indexOf('/submit/') === 0) return ARCHIVE + path
  if (base === '/mcp' || base.indexOf('/mcp/') === 0) return ARCHIVE + path
  if (base === '/api/mcp' || base.indexOf('/api/mcp/') === 0) return ARCHIVE + path
  if (base === '/reply' || base.indexOf('/reply/') === 0) return ARCHIVE + path
  if (base.indexOf('/search/') === 0) return ARCHIVE + path
  if (base === '/chat') {
    if (rest.length === 0) return CHAT + '/'
    return CHAT + '/' + rest
  }
  if (base.indexOf('/chat/') === 0) return ARCHIVE + path
  if (base === '/timeline' || base.indexOf('/timeline/') === 0) return TIMELINE + path
  return href
}

function pathOnly(pathname) {
  const raw = typeof pathname === 'string' ? pathname : '/'
  let p = pathBase(raw)
  if (p.length === 0) {
    return '/'
  }
  if (p.charAt(0) !== '/') {
    p = '/' + p
  }
  if (p.length > 1 && p.charAt(p.length - 1) === '/') {
    p = p.slice(0, p.length - 1)
  }
  return p
}

function lastSegment(path) {
  let last = 0
  for (let i = 0; i < path.length; i++) {
    if (path.charAt(i) === '/') {
      last = i + 1
    }
  }
  return path.slice(last)
}

function segmentCount(path) {
  let n = 0
  let inSeg = false
  for (let i = 0; i < path.length; i++) {
    if (path.charAt(i) === '/') {
      inSeg = false
    } else if (!inSeg) {
      n = n + 1
      inSeg = true
    }
  }
  return n
}

function endsWithFold(s, suffix) {
  const lower = s.toLowerCase()
  const suf = suffix.toLowerCase()
  if (lower.length < suf.length) {
    return false
  }
  return lower.slice(lower.length - suf.length) === suf
}

function isMdLeaf(seg) {
  if (endsWithFold(seg, '.mdx')) {
    return true
  }
  if (endsWithFold(seg, '.md')) {
    return true
  }
  return false
}

function hasNonMdExtension(seg) {
  if (isMdLeaf(seg)) {
    return false
  }
  for (let i = 0; i < seg.length; i++) {
    if (seg.charAt(i) === '.') {
      return true
    }
  }
  return false
}

function startsWithPrefix(path, prefix) {
  if (path === prefix) {
    return true
  }
  if (path.indexOf(prefix + '/') === 0) {
    return true
  }
  return false
}

function isAppShellPath(path) {
  if (startsWithPrefix(path, '/search')) {
    return true
  }
  if (startsWithPrefix(path, '/timeline')) {
    return true
  }
  if (startsWithPrefix(path, '/chat')) {
    return true
  }
  if (startsWithPrefix(path, '/submit')) {
    return true
  }
  if (startsWithPrefix(path, '/mcp')) {
    return true
  }
  if (startsWithPrefix(path, '/api')) {
    return true
  }
  if (startsWithPrefix(path, '/reply')) {
    return true
  }
  if (startsWithPrefix(path, '/site')) {
    return true
  }
  if (startsWithPrefix(path, '/_next')) {
    return true
  }
  if (startsWithPrefix(path, '/export')) {
    return true
  }
  if (startsWithPrefix(path, '/files')) {
    return true
  }
  if (startsWithPrefix(path, '/hooks')) {
    return true
  }
  if (startsWithPrefix(path, '/docs')) {
    return true
  }
  return false
}

/** True for files-corpus article URLs (pretty or `.md`), not app shells or home. */
export function isMdCorpusSubpath(pathname) {
  const path = pathOnly(pathname)
  if (path === '/') {
    return false
  }
  if (path === '/page.md' || path === '/index.md' || path === '/page.mdx') {
    return false
  }
  if (isAppShellPath(path)) {
    return false
  }
  const seg = lastSegment(path)
  if (hasNonMdExtension(seg)) {
    return false
  }
  if (isMdLeaf(seg)) {
    return true
  }
  if (segmentCount(path) >= 2) {
    return true
  }
  return false
}

/** Pretty path the archive catch-all understands (no `.md`). */
export function archivePrettyPath(pathname) {
  if (!isMdCorpusSubpath(pathname)) {
    return '/'
  }
  let path = pathOnly(pathname)
  if (endsWithFold(path, '.mdx')) {
    path = path.slice(0, path.length - 4)
  } else if (endsWithFold(path, '.md')) {
    path = path.slice(0, path.length - 3)
  }
  if (path.length === 0) {
    return '/'
  }
  return path
}

/** Archive header banner: keep markdown corpus subpaths, else archive home. */
export function archiveBannerHref(pathname) {
  const pretty = archivePrettyPath(pathname)
  if (pretty === '/') {
    return ARCHIVE + '/'
  }
  return ARCHIVE + pretty
}
