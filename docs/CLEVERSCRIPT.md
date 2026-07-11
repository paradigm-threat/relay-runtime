# CleverScript — Language Specification

CleverScript is the JS-like language executed by **relay-runtime** for client hooks. It is intentionally a **small, well-specified subset** designed for JSX rendering—not full JavaScript. This document defines what is supported and what is explicitly out of scope.

**Design principle:** Keep the surface small and predictable. Fail fast on unsupported constructs. Prefer host-provided APIs over runtime emulation.

---

## Server hooks (gateway)

When CleverScript runs as **`server.hooks.*`** modules (`.cs`), the relay-gateway provides virtual modules:

- **`@relay/git`** — `readFile`, `readFileBase64`
- **`@relay/search/sqlite`** / **`@relay/search/redb`** — `search(ctx)` for QUERY (Rust FTS / filter); optional `config` path (default tip `search.yaml`); preferred over returning `{ plugin: "sqlite" }`
- On GET / raster resize paths only: **`@relay/image`**, **`@relay/ipfs`**, **`@relay/md`**

Use top-level **`import { … } from '@relay/…'`**. The global **`Relay`** is **`null`**; prefer imports for new scripts.

---

## 1. Supported — Must Have

### 1.1 Literals

- `null`, `true`, `false`
- Numbers: `0`, `1.5`, `-3`
- Strings: `"hello"`, `'world'`
- **Template literals** `` `…${expr}…` `` — expanded at lex time to string literals and `+` (same meaning as concatenation). Static parts support escapes such as `` \` ``, `\${`, `\\`, `\n`, etc. **MVP caveat:** each `${…}` runs until a matching `}` by **brace depth only**; `{` / `}` inside `"strings"`, `'strings'`, nested `` `…` ``, or comments can confuse the scanner—extract a variable or use `+` for those cases.
- **String instance helpers** (receiver must be a string): `split(sep?)`, `trim()`, `startsWith(prefix)`, `toLowerCase()`, `toUpperCase()` — useful in server hooks (e.g. case-insensitive file extensions).

### 1.2 Objects

- Object literals with **string keys only**: `{ a: 1, b: "x" }`
- Member access: `obj.key`, `obj["key"]`, `obj?.prop`, `obj?.[i]`
- No computed keys in literals

### 1.3 Arrays

- Array literals: `[]`, `[1, 2, 3]`
- Length: `arr.length`
- Index access: `arr[i]` (where `i` is a numeric variable or literal)
- Building arrays: use `concat` or host-provided helpers (e.g. `arrayConcat(a, b)`); **no** `arr.push()` in core

### 1.4 Operators

- Arithmetic: `+`, `-`, `*`, `/`
- Comparison: `===`, `!==`, `<`, `<=`, `>`, `>=`
- Logical: `&&`, `||`, `!`
- Unary: `!`, `-` (negation), `++`, `--` (postfix)
- Nullish coalescing: `??` — `a ?? b` returns `b` when `a` is `null`
- Optional chaining: `?.` — `obj?.prop`, `obj?.[i]` returns `null` when `obj` is `null`

### 1.5 Control Flow

- `if` / `else if` / `else`
- `for (let i = 0; i < n; i++)` — classic for loops
- `while (cond) { ... }`

### 1.6 Functions

- Arrow functions: `() => x`, `(a, b) => a + b`
- `async` for top-level component when needed
- `return` statements

### 1.7 Imports

- `import x from 'source'` — default imports from host modules
- Host provides: theme, styler, relayRuntime (getThemes, postMessage, getPendingMessages), etc.
- **Client WASM / Android markdown:** `import { MarkdownRenderer } from '@clevertree/markdown'` — virtual module (not npm): named export **`MarkdownRenderer`** renders markdown to VDOM via **md2ast** built into relay-runtime. **`navigate`** is accepted for backward compatibility only; markdown links default to normal **`href`** navigation (same-origin absolute URLs are not intercepted automatically). For SPA-style routing per repo, pass **`anchorProps`** (shallow-merge object onto each parsed `<a>`) and/or **`anchorOnClick`** (`onClick` applied after merge; wins over `anchorProps.onClick`). The implementation avoids document-level delegated capture—only handlers you attach. **`@clevertree/hook-transpiler`** is an alias for the same module. IR and behavior: **`relay-gateway/docs/RELAY_MARKDOWN_IR.md`**.
- **Client WASM / Android / desktop video:** `import { VideoPlayer } from '@clevertree/video'` — virtual module: **`VideoPlayer(props)`** returns a native **`relay-video-player`** VNode. Hosts render platform playback (HTML5 `<video>`, ExoPlayer, or desktop external player) with a consistent CleverScript control surface:
  - **Media:** `src` (required), optional `poster`, `subtitles` (array of `{ src, label?, lang?, default? }` — serialized to `subtitlesJson` for hosts; `.srt` converted to WebVTT on web).
  - **Native UI:** `controls` (default `true`), `autoPlay`, `muted`, `loop`, `volume`, `playbackRate`.
  - **External control (CleverScript):** `playing` (bool), `currentTime` (seconds), `command` + `commandSeq` + optional `commandArg` — `1` play, `2` pause, `3` seek to `commandArg`.
  - **Events:** `onPlay`, `onPause`, `onEnded`, `onTimeUpdate` (throttled ~250ms on web), `onError`, `onLoadedMetadata`, `onSeeked`.
  - **Desktop (Linux X11):** embeds system **mpv** in the laid-out player rect (native OSC controls + CleverScript props). Requires `mpv` on PATH. Without mpv, shows a placeholder; click opens the system player (`xdg-open` / `open`).

### 1.8 React Hooks

- `useState(initial)` — supported
- Hooks must be called in a **static order** (no hooks in conditionals or loops)

### 1.9 JSX

- Elements: `<div>`, `<span>`, etc.
- Fragments: `<>...</>` or `<Fragment>...</>`
- Children: `{expr}` expressions
- Props: `className`, `key`, `data-*`, etc.
- No JSX spread: `<Foo {...props}` — pass props explicitly
- **Templating with variables in JSX** — `{'Hello ' + name}` or `{part1}{part2}`; or use template literals for multi-line / markdown (see §1.1).

---

## 2. Explicitly Out of Scope

### 2.1 Optional: template literals vs JSX

Template literals are supported (§1.1). For attribute values and short text, JSX with `+` is often clearer:

| Style | Example |
|-------|---------|
| Template | `` `Hello ${name}` `` |
| Concat | `'Hello ' + name` |
| JSX | `{'Hello ' + name}` |

---

### 2.2 Parser / Language Surface

| Construct | Status |
|-----------|--------|
| `==`, `!=` (loose equality) | Not supported — use `===`, `!==` only |
| Template literals `` `…${x}…` `` | Lowered to `+` at lex; MVP: `${…}` uses naive `{`/`}` depth (see §1.1) |
| Object/array spread `{...a}`, `[...a]` | Not supported |
| Complex destructuring | Not supported — simple `const {a} = obj` only |
| `typeof`, `void`, `delete` | Not supported |

### 2.3 Control Flow

| Construct | Status |
|-----------|--------|
| `for...of`, `for...in` | Not supported |
| `do...while`, `switch` | Not supported — use `while` or `if/else` |
| `try/catch` | Not supported |

### 2.4 Objects

| Construct | Status |
|-----------|--------|
| Computed keys `[expr]` in object literals | Not supported |
| Prototype chain, `__proto__` | Not supported |
| `this` | Not supported |

### 2.5 Functions and Modules

| Construct | Status |
|-----------|--------|
| Dynamic `import()` in hot path | Avoid — complicates hook analysis |
| Named exports | Single default export per module |
| `arguments` object | Not supported |

### 2.6 Arrays

| Construct | Status |
|-----------|--------|
| `arr.push()`, `arr.pop()`, etc. | Not supported — use `concat` or host helpers |
| Mutable array methods | Not supported — prefer immutable patterns |

### 2.7 JSX

| Construct | Status |
|-----------|--------|
| JSX spread `<Foo {...props}` | Not supported |
| Complex expressions in attributes | Limit to literals, identifiers, `obj.key` |

---

## 3. Host Boundary

- **Heavy APIs from host:** theme, styler, relayRuntime, fetch, etc.
- **JSON in, JSON out:** theme/config as JSON; vnodes as JSON
- **Fail fast:** unsupported syntax or runtime error → clear error, no silent fallback

### 3.1 Host messaging (postMessage)

Hooks can interact with the host via `window.postMessage` and `addEventListener("message", ...)`:

- **`relayRuntime.postMessage(payload)`** — sends a message to the host. Payload is wrapped as `{ type: "relay-runtime", payload }`. Host receives via `window.addEventListener("message", (e) => { ... })`.
- **`relayRuntime.getPendingMessages()`** — returns an array of messages pushed from the host (e.g. from parent iframe, other tabs). The bootstrap adds a message listener that forwards incoming messages (excluding relay-runtime’s own) into this queue.

---

## 4. Hooks and Analysis

- **Static hook count:** analysis must know exact number of hooks per component
- **No hooks in conditionals or loops:** hooks at top level only
- **Dynamic imports:** resolve before hook execution so analysis sees all hooks
- **`arr.map()` with JSX:** callbacks returning JSX **must** include a `key` prop on the returned element (e.g. `<Item key={item.id} />`). Analysis fails otherwise.

---

## 4.1 Runtime Limits and Safety

To prevent hangs and memory exhaustion, the runtime enforces:

| Limit | Value | Behavior |
|-------|-------|----------|
| **Loop iterations** | 100,000 | `for` and `while` abort with error if exceeded (possible infinite loop) |
| **Recursion depth** | 256 | Function/component call depth; aborts to prevent stack overflow |
| **Array length** | 50,000 | Array literals, `concat`, `map`, `filter`, `find` reject larger arrays |

---

## 4.2 Per-Component Error Boundary

When a component throws during render, the runtime **does not** crash the whole tree. Instead, it renders a fallback UI for that component:

- A `div` with `className="relay-error-boundary"`
- `data-error-boundary` = component name
- `data-error-message` = error message
- Inline error styling (red border, light red background)
- Text: `[ComponentName] error message`

Siblings and parent components continue to render. This improves reliability compared to a single global crash.

---

## 5. Documentation and Testing

- **Supported subset doc:** this document
- **Parse-first validation:** reject unsupported syntax at parse time with clear errors
- **Golden/snapshot tests:** parse + run for representative hooks to catch regressions

---

## 6. Summary Table

| Category | Supported | Not Supported |
|----------|-----------|---------------|
| Equality | `===`, `!==` | `==`, `!=` |
| Optional | `?.`, `??` | — |
| Spread | — | `{...}`, `[...]` |
| Unary | `!`, `-`, `++`, `--` (postfix) | `typeof`, `void` |
| Loops | `for (;;)`, `while` | `for...of`, `for...in`, `do...while` |
| Objects | string keys, `obj.key` | computed keys, prototype, `this` |
| Arrays | `length`, `[i]`, `concat` | `push`, `pop`, mutable methods |
| JSX | elements, fragments, `{expr}` (templating), template literals for strings | spread `{...props}` |

---

## Related

- [POLICY.md](POLICY.md) — no legacy, no transpiler
- [PLAN.md](PLAN.md) — strategy and language philosophy
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md) — web embedding
