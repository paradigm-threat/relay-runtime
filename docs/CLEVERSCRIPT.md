# CleverScript — Language Specification

CleverScript is the JS-like language executed by **relay-runtime** for client hooks. It is intentionally a **small, well-specified subset** designed for JSX rendering—not full JavaScript. This document defines what is supported and what is explicitly out of scope.

**Design principle:** Keep the surface small and predictable. Fail fast on unsupported constructs. Prefer host-provided APIs over runtime emulation. Tip CS is **routing/composition**; shared/heavy logic belongs in Rust `@relay/*` (see `relay-gateway/docs/CS_HOST_HELPERS.md`).

---

## Server hooks (gateway)

When CleverScript runs as **`server.hooks.*`** modules (`.cs`), the relay-gateway provides virtual modules:

- **`@relay/git`** — `readFile`, `readFileBase64`
- **`@relay/search/sqlite`** / **`@relay/search/redb`** — `search(ctx)` for QUERY (Rust FTS / filter); optional `config` path (default tip `search.yaml`); preferred over returning `{ plugin: "sqlite" }`
- On GET / raster resize paths only: **`@relay/image`**, **`@relay/ipfs`**, **`@relay/md`**
- Always (server + client SSR/WASM): **`@relay/util`**, **`@relay/path`**, **`@relay/date`**, **`@relay/catalog`**, **`@relay/ui`** — tip ergonomics (sort/take, basename, ChangeLog date formats, nav-index parse/tree, Link/RadioTabs/cachedGet)

Top-level **`import { … } from '@relay/…'`**. The global **`Relay`** is **`null`**; prefer imports for new scripts.

| Module | Exports (summary) |
|--------|-------------------|
| `@relay/util` | `sortBy`, `take`, `prop`, `omitUndefined`, `slugify`, `uniqueId`, `includes`, `splitSentences`, `takeSentenceChunk` |
| `@relay/path` | `basename`, `dirname`, `extname`, `join`, `split`, `normalizeRel`, `resolveRel`, `parentDir`, `siteHref`, `resolveEmbedSrc`, `resolveAssetUrl`, `rewriteMarkdownAssetUrls`, `portraitSiblingSrc`, `repoRelFromMediaSrc`, `safeImgDomId`, `readHashImgSrc`, `galleryImgHash`, `galleryDisplaySrc`, `portraitSiblingIfPresent` |
| `@relay/date` | `formatTiny` / `formatLong` / `formatFeed` / `sameDay` |
| `@relay/catalog` | `parseNavIndexTxt`, `loadNavIndex`, `extractMarkdownHeadings`, `contentKind`, `buildDirTree`, `publishedFromBlogPath`, `neighborPaths`, `isFeelingLuckyCandidate`, `pickFeelingLuckyPath` |
| `@relay/ui` | `cachedGet`, `Link`, `RadioTabs`, `safeProps` / `omitUndefined` |
| `@relay/tts` | `synthesize({ text, voiceId?, baseUrl?, endpoints? })` → data URL; `AudioPlayer` → `relay-audio-player` |

**`fetch` binary bodies:** pass `{ responseType: 'base64' }` in options; response may include **`base64`** + **`contentType`** (web XHR). Used by `@relay/tts.synthesize` for Piper `POST /tts`.


### ES builtins (stdlib)

| Name | Notes |
|------|--------|
| `undefined` | Alias of `null` |
| `parseInt` / `parseFloat` | Invalid → `null` |
| `Number.from` / `isFinite` / `isInteger` | No callable-with-properties; use `Number.from(x)` not `Number(x)` |
| `Math.min\|max\|abs\|floor\|ceil\|round` | |
| `Date.now` / `Date.parse` / `Date.from` | No `new Date` yet; instances are `{ __type: "Date", __ms }` with UTC getters + `toISOString` |
| `Array#sort` / `#slice` | **Return new arrays** (CS value copy semantics; not ES in-place mutate) |
| `String#replaceAll` | String search only |

---

## 1. Supported — Must Have

### 1.1 Literals

- `null`, `true`, `false`
- Numbers: `0`, `1.5`, `-3`
- Strings: `"hello"`, `'world'`
- **Template literals** `` `…${expr}…` `` — expanded at lex time to string literals and `+` (same meaning as concatenation). Static parts support escapes such as `` \` ``, `\${`, `\\`, `\n`, etc. **MVP caveat:** each `${…}` runs until a matching `}` by **brace depth only**; `{` / `}` inside `"strings"`, `'strings'`, nested `` `…` ``, or comments can confuse the scanner—extract a variable or use `+` for those cases.
- **String instance helpers** (receiver must be a string): `split(sep?)`, `trim()`, `startsWith(prefix)`, `indexOf`, `lastIndexOf`, `slice`, `charAt`, `toLowerCase()`, `toUpperCase()`, `replaceAll(from, to)` — useful in server hooks (e.g. case-insensitive file extensions). `.length` is supported.

### 1.2 Objects

- Object literals with **identifier keys** (and shorthand): `{ a: 1, b }`
- Shallow object spread: `{ ...a, b: 2 }` (later keys win; non-object spreads error except `null`)
- Member access: `obj.key`, `obj["key"]`, `obj?.prop`, `obj?.[i]`
- Simple object destructuring: `const { a, b } = obj` (identifier keys / rename `const { a: x } = obj`; no nested/rest/defaults)
- No computed keys in literals

### 1.3 Arrays

- Array literals: `[]`, `[1, 2, 3]`
- Shallow array spread: `[...a, x, ...b]` (arrays only; length limit enforced)
- Length: `arr.length`
- Index access: `arr[i]` (where `i` is a numeric variable or literal)
- Methods: `map`, `filter`, `find`, `concat`, `slice`, `sort(compareFn?)` (sort/slice return **new** arrays)
- Building arrays: prefer `concat` or spread; **no** `arr.push()` in core

### 1.4 Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `===`, `!==`, `<`, `<=`, `>`, `>=`
- Logical: `&&`, `||`, `!`
- Unary: `!`, `-` (negation), `typeof`, `++`, `--` (prefix and postfix)
- Assignment: `=`, `+=`, `-=`, `*=`, `/=`, `%=`
- Nullish coalescing: `??` — `a ?? b` returns `b` when `a` is `null`
- Optional chaining: `?.` — `obj?.prop`, `obj?.[i]`, `fn?.()` returns `null` when the base/callee is `null`

### 1.5 Control Flow

- `if` / `else if` / `else`
- `for (let i = 0; i < n; i++)` — classic for loops
- `for (const x of arr)` — **arrays only** (no iterators)
- `while (cond) { ... }`
- `switch` / `case` / `default` — exclusive cases (no fall-through); trailing `break` optional
- `break` / `continue` in loops (and `break` in switch)
- `try` / `catch` / `finally` — catch binds the error as a string; no `throw` syntax

### 1.6 Functions

- Arrow functions: `() => x`, `(a, b) => a + b`, `(a = 1) => a`, `(...rest) => rest`
- `function` / `async function` declarations
- Default parameters: missing or `null` argument uses default (`??`-style)
- Rest parameters: last param `...rest` binds remaining args as an array
- `return` statements

### 1.7 Imports / exports

- `import x from 'source'` — default imports from host modules
- Named / namespace imports as supported by the parser
- `export default function …`
- `export function name(…)` — named export installed in the module env
- Host provides: theme, styler, relayRuntime (getThemes, postMessage, getPendingMessages), etc.
- **Client WASM / Android markdown:** `import { MarkdownRenderer } from '@clevertree/markdown'` — virtual module (not npm): named export **`MarkdownRenderer`** renders markdown to VDOM via **md2ast** built into relay-runtime. **`navigate`** is accepted for backward compatibility only; markdown links default to normal **`href`** navigation (same-origin absolute URLs are not intercepted automatically). For SPA-style routing per repo, pass **`anchorProps`** (shallow-merge object onto each parsed `<a>`) and/or **`anchorOnClick`** (`onClick` applied after merge; wins over `anchorProps.onClick`). Custom tags (e.g. `<AutoIndex />`) require **`overrides={{ AutoIndex: Comp }}`** — override keys become md2ast allowed tags and callables replace those elements. The implementation avoids document-level delegated capture—only handlers you attach. **`@clevertree/hook-transpiler`** is an alias for the same module. IR and behavior: **`relay-gateway/docs/RELAY_MARKDOWN_IR.md`**.
- **Client WASM / Android / desktop video:** `import { VideoPlayer } from '@clevertree/video'` — virtual module: **`VideoPlayer(props)`** returns a native **`relay-video-player`** VNode. Hosts render platform playback (HTML5 `<video>`, ExoPlayer, or desktop external player) with a consistent CleverScript control surface:
  - **Media:** `src` (required), optional `poster`, `subtitles` (array of `{ src, label?, lang?, default? }` — serialized to `subtitlesJson` for hosts; `.srt` converted to WebVTT on web).
  - **Native UI:** `controls` (default `true`), `autoPlay`, `muted`, `loop`, `volume`, `playbackRate`.
  - **External control (CleverScript):** `playing` (bool), `currentTime` (seconds), `command` + `commandSeq` + optional `commandArg` — `1` play, `2` pause, `3` seek to `commandArg`.
  - **Events:** `onPlay`, `onPause`, `onEnded`, `onTimeUpdate` (throttled ~250ms on web), `onError`, `onLoadedMetadata`, `onSeeked`.
  - **Desktop (Linux X11):** embeds system **mpv** in the laid-out player rect (native OSC controls + CleverScript props). Requires `mpv` on PATH. Without mpv, shows a placeholder; click opens the system player (`xdg-open` / `open`).

### 1.8 React Hooks

- `useState(initial)` — supported
- `useEffect`, `useCallback`, `useRef` — provided by the React virtual module (semantics are a subset of React)
- Hooks must be called in a **static order** (no hooks in conditionals or loops)

### 1.9 JSX

- Elements: `<div>`, `<span>`, etc.
- Fragments: `<>...</>`
- Children: `{expr}` expressions
- Props: `className`, `key`, hyphenated attrs (`data-*`, `aria-*`), etc.
- No JSX spread: `<Foo {...props}` — pass props explicitly
- **Templating with variables in JSX** — `{'Hello ' + name}` or `{part1}{part2}`; or use template literals for multi-line / markdown (see §1.1).

### 1.10 Optional type annotations

- Optional annotations on variables/params: `number`, `string`, `boolean`, `object`, `array`, `null` (validated at runtime when present)

---

## 2. Explicitly Out of Scope

### 2.1 Optional: template literals vs JSX

Template literals are supported (§1.1). For attribute values and short text, JSX with `+` is often clearer.

### 2.2 Parser / Language Surface

| Construct | Status |
|-----------|--------|
| `==`, `!=` (loose equality) | Not supported — use `===`, `!==` only |
| Template literals `` `…${x}…` `` | Lowered to `+` at lex; MVP: `${…}` uses naive `{`/`}` depth (see §1.1) |
| Complex / nested destructuring | Not supported — simple object/array patterns only |
| `void`, `delete` | Not supported |
| Nested object destructuring / object rest | Not supported |

### 2.3 Control Flow

| Construct | Status |
|-----------|--------|
| `for...in` | Not supported |
| `do...while` | Not supported — use `while` or `if/else` |
| `throw` | Not supported |

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
| `arguments` object | Not supported |
| Real async / Promises | `async`/`await` are sync passthrough |

### 2.6 Arrays

| Construct | Status |
|-----------|--------|
| `arr.push()`, `arr.pop()`, etc. | Not supported — use `concat` or spread |
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
| **Loop iterations** | 100,000 | `for`, `for...of`, and `while` abort with error if exceeded (possible infinite loop) |
| **Recursion depth** | 256 | Function/component call depth; aborts to prevent stack overflow |
| **Array length** | 50,000 | Array literals, spreads, `concat`, `map`, `filter`, `find`, rest params reject larger arrays |

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

## 5.1 Tip / park hook checklist (hard-won)

When writing CleverScript for parked tips (SSR + hydrate):

1. **Braces always** — `while` / `for` / `try` bodies must use `{ … }`. Single-statement bodies parse-error with a braces hint.
2. **No reserved bindings** — `from`, `of`, `as`, `import`, etc. cannot be variable names (`const from = …` fails with a reserved-keyword message). Prefer `rootPrefix`, `needle`, …
3. **Exports** — `export default`, `export function`, **`export const`**, **`export let`**. Top-level `const`/`let` in a module are evaluated when the module loads (with function decls hoisted first).
4. **Lone quote literals are OK in the runtime** — `'"'` and `"'"` round-trip; prefer them over ad-hoc concat when you need a quote char. Prefer longer safe forms (`"x'y".charAt(1)`) only if a host/tooling path mangles lone quotes.
5. **Prefer literal `replaceAll` over host+path concat** for URL rewriting — concat has historically mangled tip paths under SSR.
6. **Markdown custom tags** — pass `overrides={{ AutoIndex: Comp, … }}` to `MarkdownRenderer` so md2ast allows the tag and CleverScript renders the component. Without overrides, `<AutoIndex />` leaks as text. Nested `function` decls inside the component body are supported; module-level components are fine too.
7. **Do not preprocess large markdown with fragile replaces** to fake MDX — use overrides instead.
8. **JSX attribute expressions must evaluate** — failed `{expr}` props are errors (not silently dropped), so a bad `overrides={…}` surfaces instead of rendering without custom tags.
9. **Avoid unary minus after `return`** — use `return 0 - 1` or `return (-1)`, not `return -1`.
10. **Shared module state** — prefer `export const api = createApi()` where methods close over locals inside `createApi()` (works). Module-level `const` read from *nested* `function` bodies is unreliable; do not rely on that.
11. **Park SSR tiers (normative tip doc)** — **P0** article + TOC and **P1** static chrome on SSR; **P2 catalog** (DirNav / ChangeLog / article lists) and **P3** interactive **forbidden** on SSR — placeholders until `isClientLive()`. Do **not** SSR catalog rows “with a better cache”: tip-OID HTML cache misses would reintroduce multi-second cold renders; same-Host QUERY during SSR deadlocks the gateway worker. Canon: park `hooks/CLEVERSCRIPT_TIP.md`.
12. **Catalog warm on hydrate** — `@relay/catalog.loadNavIndex` uses request-local `tip_cached_get('/nav-index.txt')` + `tip_cached_nav_index_rows` (one parse per tip blob text per RunState session). Tip modules may module-warm sidecars (e.g. `changelog-thumbs.json`). Prefer that over per-component re-parse or SSR QUERY.
13. **`html[data-pt-hydrated]`** — gateway SSR embed emits `"0"`; web glue (`syncHtmlHydratedAttr`) sets `"1"` when promoting `set_client_live(true)`. Tips style inactive chrome with `[data-pt-hydrated="0"] .pt-client-only` (keep nav links clickable). Naming stays `data-pt-*` alongside `data-pt-theme`.

---

## 6. Summary Table

| Category | Supported | Not Supported |
|----------|-----------|---------------|
| Equality | `===`, `!==` | `==`, `!=` |
| Optional | `?.`, `??`, `fn?.()` | — |
| Spread | `{...}`, `[...]` (shallow) | JSX `{...props}` |
| Unary | `!`, `-`, `typeof`, `++`/`--` (prefix/postfix) | `void`, `delete` |
| Loops | `for (;;)`, `for...of` (arrays), `while`, `break`/`continue` | `for...in`, `do...while` |
| Switch | exclusive `switch`/`case` | fall-through |
| Objects | identifier keys, destructuring, shallow spread | computed keys, prototype, `this` |
| Arrays | `length`, `[i]`, `concat`/`map`/`filter`/`find`, spread | `push`, `pop`, mutable methods |
| Modules | `export default`, `export function`, `export const`, `export let` | `arguments` |
| JSX | elements, fragments `<>`, `{expr}` | spread `{...props}` |
| Markdown | `MarkdownRenderer` + `anchorProps` / `anchorOnClick` / **`overrides`** | full MDX expressions |

---

## Related

- [POLICY.md](POLICY.md) — no legacy, no transpiler
- [PLAN.md](PLAN.md) — strategy and language philosophy
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md) — web embedding
- Park tip tiers / catalog SSR anti-pattern: tip `hooks/CLEVERSCRIPT_TIP.md`
- Gateway SSR HTML cache: `relay-gateway/docs/RELAY_POLICY_AND_PERFORMANCE.md`

*Last updated: 2026-08-02*
