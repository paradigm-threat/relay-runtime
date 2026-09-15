import React, { useState, useEffect } from 'react'
import { isClientLivePhase } from '@clevertree/style-internal'
import { isCsrHost } from 'hooks/client/hostMode.jsx'
import { dirExpandPaths } from '@relay/catalog'
import { includes } from '@relay/util'
import { propStr } from 'hooks/client/propOr.jsx'
import { loadDirTree, rowPath, rowTitle, siteHref } from 'hooks/client/loadArticles.jsx'
import FeelingLucky from 'hooks/client/components/FeelingLucky.jsx'
import {
  IconChevronDownSm,
  IconChevronRightSm,
  IconFileSm,
} from 'hooks/client/components/Icons.jsx'

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

function pathIsActive(folderPath, currentRel) {
  if (folderPath.length === 0) return false
  if (currentRel === folderPath) return true
  if (currentRel.startsWith(folderPath + '/')) return true
  return false
}

function pathMatchesCurrent(filePath, currentRel) {
  if (!filePath || !currentRel) return false
  if (filePath === currentRel) return true
  if (currentRel + '.md' === filePath) return true
  if (filePath + '.md' === currentRel) return true
  return false
}

function pathListHas(list, path) {
  return includes(list, path) === true
}

function pathListToggle(list, path) {
  if (pathListHas(list, path)) {
    let next = []
    for (let i = 0; i < list.length; i++) {
      if (list[i] !== path) next = next.concat([list[i]])
    }
    return next
  }
  return list.concat([path])
}

function scrollActiveDirFileIntoView() {
  let el = null
  try {
    el = document.querySelector('.pt-dir-file-active')
  } catch (e) {
    return
  }
  if (!el) return
  try {
    if (el.scrollIntoView) {
      el.scrollIntoView({ block: 'nearest', inline: 'nearest' })
    }
  } catch (e2) {}
}

function seedExpandPaths(currentRel) {
  let paths = []
  try {
    paths = dirExpandPaths(currentRel)
  } catch (e) {}
  if (!paths) return []
  return paths
}

function DirFileRow(props) {
  const row = props.row
  const depth = props.depth
  const currentRel = props.currentRel
  const path = rowPath(row)
  if (path.length === 0) return null
  const active = pathMatchesCurrent(path, currentRel)
  let cls = 'pt-dir-file'
  if (active) {
    cls = cls + ' pt-dir-file-active'
  }
  const pad = depth * 16 + 'px'
  return (
    <div className={cls} style={{ paddingLeft: pad }}>
      <span className="pt-dir-file-icon" aria-hidden="true">
        <IconFileSm />
      </span>
      <a href={siteHref('/' + path)} className="pt-dir-file-link" title={path}>
        {rowTitle(row)}
      </a>
    </div>
  )
}

function FolderBranch(props) {
  const node = props.node
  const depth = props.depth
  const keyPrefix = props.keyPrefix
  const expandPaths = props.expandPaths
  const onToggle = props.onToggle
  const currentRel = props.currentRel
  let name = ''
  let path = ''
  let count = 0
  let files = []
  let folders = []
  try {
    if (node && typeof node.name === 'string') name = node.name
  } catch (e) {}
  try {
    if (node && typeof node.path === 'string') path = node.path
  } catch (e2) {}
  try {
    if (node && typeof node.count === 'number') count = node.count
  } catch (e3) {}
  try {
    if (node && node.files) files = node.files
  } catch (e4) {}
  try {
    if (node && node.folders) folders = node.folders
  } catch (e5) {}

  const hasChildren = folders.length > 0 || files.length > 0
  const isExpand = pathListHas(expandPaths, path)
  const openVal = isExpand ? '1' : '0'
  const active = pathIsActive(path, currentRel)
  let rowCls = 'pt-dir-row'
  if (active) {
    rowCls = rowCls + ' pt-dir-row-active'
  }
  const pad = depth * 16 + 'px'
  // Mount children only while open — keeps phase-2 hydrate cheap; click remounts
  // via useState (DOM class toggles alone left empty .pt-dir-kids).
  const fileCap = isExpand === true ? (files.length < 12 ? files.length : 12) : 0
  const hiddenCount = isExpand === true ? files.length - fileCap : 0

  let folderEls = []
  if (isExpand === true) {
    for (let i = 0; i < folders.length; i++) {
      folderEls = folderEls.concat([
        <FolderBranch
          key={keyPrefix + 'd' + i}
          node={folders[i]}
          depth={depth + 1}
          keyPrefix={keyPrefix + 'd' + i + '-'}
          expandPaths={expandPaths}
          onToggle={onToggle}
          currentRel={currentRel}
        />,
      ])
    }
  }

  let fileEls = []
  if (isExpand === true) {
    for (let i = 0; i < fileCap; i++) {
      fileEls = fileEls.concat([
        <DirFileRow
          key={keyPrefix + 'f' + i}
          row={files[i]}
          depth={depth + 1}
          currentRel={currentRel}
        />,
      ])
    }
  }

  let moreEls = []
  if (hiddenCount > 0) {
    moreEls = moreEls.concat([
      <a
        key={keyPrefix + 'all'}
        href={siteHref('/' + path + '/page.md')}
        className="pt-dir-showall"
        style={{ paddingLeft: (depth + 1) * 16 + 22 + 'px' }}
      >
        {'+' + hiddenCount + ' more in folder…'}
      </a>,
    ])
  }

  const kidsCls = isExpand ? 'pt-dir-kids pt-dir-kids-open' : 'pt-dir-kids'
  const openIconCls = isExpand
    ? 'pt-dir-chevron-open pt-dir-chevron-open-on'
    : 'pt-dir-chevron-open'
  const closedIconCls = isExpand
    ? 'pt-dir-chevron-closed pt-dir-chevron-closed-off'
    : 'pt-dir-chevron-closed'

  return (
    <div
      className="pt-dir-node"
      data-pt-dir-node="1"
      data-pt-path={path}
      data-pt-open={openVal}
      aria-expanded={openVal}
    >
      <div className={rowCls} style={{ paddingLeft: pad }}>
        {hasChildren ? (
          <button
            type="button"
            className="pt-dir-chevron"
            aria-label={isExpand ? 'Collapse folder' : 'Expand folder'}
            onClick={() => {
              onToggle(path)
            }}
          >
            <span className={openIconCls} aria-hidden="true">
              <IconChevronDownSm />
            </span>
            <span className={closedIconCls} aria-hidden="true">
              <IconChevronRightSm />
            </span>
          </button>
        ) : (
          <span className="pt-dir-chevron-spacer" aria-hidden="true" />
        )}
        <a href={siteHref('/' + path + '/page.md')} className="pt-dir-folder-link">
          <span className="pt-dir-folder-name">{folderTitle(name)}</span>
          {count > 0 ? <span className="pt-dir-count">{'(' + count + ')'}</span> : null}
        </a>
      </div>
      {hasChildren ? (
        <div className={kidsCls}>
          {folderEls}
          {fileEls}
          {moreEls}
        </div>
      ) : null}
    </div>
  )
}

function dirCssMarker() {
  return (
    <span
      aria-hidden="true"
      hidden={true}
      className="pt-dir-nav lg:pt-dir-nav-lg pt-dir-head pt-dir-title pt-dir-actions pt-dir-action pt-dir-sep pt-dir-tree pt-dir-node pt-dir-row pt-dir-row-active pt-dir-chevron pt-dir-chevron-open pt-dir-chevron-open-on pt-dir-chevron-closed pt-dir-chevron-closed-off pt-dir-chevron-spacer pt-dir-folder-link pt-dir-folder-name pt-dir-count pt-dir-kids pt-dir-kids-open pt-dir-file pt-dir-file-extra pt-dir-file-active pt-dir-file-icon pt-dir-file-link pt-dir-showall pt-dir-showfewer pt-dir-foot pt-feeling-lucky-dir pt-dir-list pt-dir-item pt-dir-link pt-dir-tools pt-dir-tool pt-dir-folder pt-dir-folder-label pt-dir-nested"
      data-pt-dir-css="true"
    />
  )
}

/** Left directory — QUERY catalog → `@relay/catalog.buildDirTree`; expand via useState. */
export default function DirNav(props) {
  const currentRel = stripRel(propStr(props, 'currentPath', ''))
  const live = isClientLivePhase(2) === true

  // Hooks must run every render (SSR placeholder + live). Seed empty until live.
  const expandState = useState([])
  const expandPaths = expandState[0]
  const setExpandPaths = expandState[1]

  useEffect(() => {
    if (live !== true) return
    try {
      setExpandPaths(seedExpandPaths(currentRel))
    } catch (e) {}
  }, [live, currentRel])

  useEffect(() => {
    if (live !== true) return
    if (!expandPaths || expandPaths.length === 0) return
    try {
      scrollActiveDirFileIntoView()
    } catch (eScroll) {}
  }, [live, currentRel, expandPaths])

  if (live !== true) {
    if (isCsrHost()) {
      return null
    }
    return (
      <aside
        className="pt-dir-nav lg:pt-dir-nav-lg"
        aria-label="Site directory"
        data-pt-dirnav-ssr="1"
      >
        <div className="pt-dir-head">
          <h2 className="pt-dir-title">Directory</h2>
        </div>
        <p className="pt-muted">Directory loads after page open…</p>
        {dirCssMarker()}
      </aside>
    )
  }

  let tree = null
  try {
    tree = loadDirTree()
  } catch (eLoad) {}
  let folders = []
  let files = []
  try {
    if (tree && tree.folders) folders = tree.folders
  } catch (e) {}
  try {
    if (tree && tree.files) files = tree.files
  } catch (e2) {}

  function onToggle(path) {
    setExpandPaths(pathListToggle(expandPaths, path))
  }

  function collapseAll() {
    setExpandPaths([])
  }

  function expandToCurrent() {
    setExpandPaths(seedExpandPaths(currentRel))
  }

  let folderEls = []
  for (let i = 0; i < folders.length; i++) {
    folderEls = folderEls.concat([
      <FolderBranch
        key={'r-d' + i}
        node={folders[i]}
        depth={0}
        keyPrefix={'r-d' + i + '-'}
        expandPaths={expandPaths}
        onToggle={onToggle}
        currentRel={currentRel}
      />,
    ])
  }
  let fileEls = []
  for (let j = 0; j < files.length; j++) {
    fileEls = fileEls.concat([
      <DirFileRow
        key={'r-f' + j}
        row={files[j]}
        depth={0}
        currentRel={currentRel}
      />,
    ])
  }

  const empty = folders.length === 0 && files.length === 0

  return (
    <aside className="pt-dir-nav lg:pt-dir-nav-lg" aria-label="Site directory">
      <div className="pt-dir-head">
        <h2 className="pt-dir-title">Directory</h2>
      </div>
      <div className="pt-dir-actions">
        <button type="button" className="pt-dir-action" onClick={collapseAll}>
          Collapse all
        </button>
        <span className="pt-dir-sep">{'|'}</span>
        <button type="button" className="pt-dir-action" onClick={expandToCurrent}>
          Expand to current
        </button>
      </div>
      {empty ? (
        <p className="pt-muted">No indexed articles yet.</p>
      ) : (
        <div className="pt-dir-tree">
          {folderEls}
          {fileEls}
        </div>
      )}
      <div className="pt-dir-foot">
        <FeelingLucky dir={true} />
      </div>
      {dirCssMarker()}
    </aside>
  )
}
