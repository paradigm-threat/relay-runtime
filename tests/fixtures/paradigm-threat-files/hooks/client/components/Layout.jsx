/**
 * Repo shell: path label + Up / Home + scrollable content.
 * (Text input omitted: WASM handlers do not receive DOM event payloads for onChange.)
 */
export default function Layout(props) {
  const path = props.filePath
  const onUp = props.onUp
  const onHome = props.onHome
  const children = props.children

  return (
    <div className="flex flex-col h-full" id="layout">
      <div className="flex gap-2 p-2 items-center border-b border-[var(--border-color,#e5e7eb)] font-mono text-sm flex-shrink-0 flex-wrap">
        <span className="opacity-70">File:</span>
        <span className="min-w-0 break-all flex-1">{path}</span>
        <button
          type="button"
          className="px-3 py-1 rounded border border-[var(--border-color,#ccc)] bg-[var(--bg-secondary,#f3f4f6)]"
          onClick={onUp}
        >
          Up
        </button>
        <button
          type="button"
          className="px-3 py-1 rounded border border-[var(--border-color,#ccc)] bg-[var(--bg-secondary,#f3f4f6)]"
          onClick={onHome}
        >
          Home
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-2">{children}</div>
    </div>
  )
}
