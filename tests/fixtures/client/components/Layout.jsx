/**
 * Repo UI Layout component (JSX)
 * - Path input + Go button
 * - Branch dropdown selector
 * - Search form to /search/[query]
 */

export default function Layout(props) {
  const { path = '/', onNavigate = () => { }, options = {}, children = null } = props || {}
  const branches = (options && options.branches) || ['main']
  const currentBranch = (options && options.repository && options.repository.currentBranch) || branches[0] || 'main'

  function onSubmitPath(ev) {
    ev.preventDefault()
    const form = ev.currentTarget
    const input = form.elements.namedItem('path')
    const value = String((input && input.value) || '')
    if (!onNavigate) return
    const normalized = value.startsWith('/') ? value : `/${value}`
    onNavigate(normalized)
  }

  function onSubmitSearch(ev) {
    ev.preventDefault()
    const form = ev.currentTarget
    const input = form.elements.namedItem('q')
    const q = String((input && input.value) || '')
    if (!q.trim() || !onNavigate) return
    // Reset to page 1 when starting a new search
    onNavigate(`/search/${encodeURIComponent(q.trim())}?page=1`)
  }

  function onChangeBranch(ev) {
    const next = (ev && ev.target && ev.target.value) || 'main'
    // Note: branch selection would require additional context; for now it's informational
    onNavigate(path || '/')
  }

  return (
    <div className="flex flex-col h-full" id="layout">
      <div className="flex flex-col gap-3 p-0 border-b border-[var(--border)] flex-shrink-0">
        <form className="flex gap-2 p-2" onSubmit={onSubmitPath}>
          <input type="text" name="path" defaultValue={path} placeholder="Enter path... (/ or /README.md)" className="flex-1 px-2 py-2 border border-gray-300 rounded font-mono text-sm" />
          <button type="submit" className="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white border-none rounded cursor-pointer text-sm font-medium">Go</button>
        </form>
        <div className="flex gap-4 p-2 items-center">
          {branches && branches.length > 0 ? (
            <label className="flex items-center gap-2 text-sm">
              <span>Branch:</span>
              <select value={currentBranch} onChange={onChangeBranch} className="px-2 py-1 border border-[var(--border)] rounded text-sm bg-[var(--bg-surface)] text-[var(--text)] cursor-pointer">
                {branches.map((b) => (
                  <option value={b} key={b}>{b}</option>
                ))}
              </select>
            </label>
          ) : null}
          <form className="flex items-center gap-2 ml-auto" onSubmit={onSubmitSearch}>
            <input type="search" name="q" placeholder="Search…" className="px-2 py-1 border border-gray-300 rounded text-sm" />
            <button type="submit" className="px-3 py-1 bg-gray-700 hover:bg-gray-800 text-white rounded text-sm">Search</button>
            <button
              type="button"
              onClick={() => {
                onNavigate('/')
              }}
              className="px-3 py-1 bg-gray-600 hover:bg-gray-700 text-white rounded text-sm"
            >
              ← Back
            </button>
          </form>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-2">{children}</div>
    </div>
  )
}
