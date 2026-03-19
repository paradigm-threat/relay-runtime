# relay-runtime Implementation Phases

**Goal**: Complete all functionality to render client hooks in `relay-template/hooks/client/*`.

---

## Web platform: WASM owns core rendering

**Policy:** **[POLICY.md](POLICY.md)** — **no** hook-transpiler, **no** transpile-to-JS for hooks, **no** legacy dual stacks. On the web, **relay-runtime** (WASM) plus **themed-styler** (separate library, linked in WASM) perform **all** hook UI semantics. **Browser JavaScript** only **hosts** WASM and **DOM/platform FFI**. Checklists: **[WEB_WASM_HOST.md](WEB_WASM_HOST.md)**, **[NEXT_STEPS.md](NEXT_STEPS.md)**.

---

## Scope: Minimal, relay-template only

We **do not** implement full ES6. We only support what relay-template actually uses. Keep things simple; avoid the complexity of existing JS standards.

### Required (from relay-template)

| Feature | Used in |
|---------|---------|
| **async/await** | get-client, query-client, plugins |
| **import/export** | All modules |
| **Dynamic import** | `await import('./plugin/tmdb.mjs')` |
| **const, let** | All files |
| **function, async function, arrow fn** | All files |
| **Destructuring** | `const { content } = ...`, `const [path, setPath] = useState(...)` |
| **Template literals** | `` `/${id}` ``, `` `https://.../${id}` `` |
| **Optional chaining** | `params?.q`, `layoutComponent?.default` |
| **Nullish coalescing** | `item.id ?? item.tmdb_id` |
| **try/catch** | Error handling |
| **if/else, return** | Control flow |
| **JSX** | Elements, props, children, `{expr}`, `{arr.map(...)}` |
| **Object/array literals** | `{ method: 'OPTIONS' }`, `[path, setPath]` |

### Host APIs (provided by runtime, not language)

- `fetch`, `fetchRelative`
- `useState`, `useEffect`, `useCallback` (React-like hooks)
- `URL`, `URLSearchParams`, `encodeURIComponent`
- `console.log`, `console.warn`, `console.error`
- `MarkdownRenderer`, `registerThemesFromYaml`, `url` (host modules)

### Explicitly out of scope (for now)

- `var`, `class`, `this`, `new` (except host-provided: `new URL`, `new Error`, `new Date`)
- Prototypes, `instanceof`, `typeof` (beyond `typeof x === 'string'`)
- `eval`, `with`, `Proxy`, `Symbol`, `Reflect`
- Generators, `for...of`, `for...in`
- Full regex (`.match()` can be host-provided)
- `Promise` constructor (async/await only; host provides `Promise.all`)

### When unsure

If a feature is not clearly used in relay-template, **ask** before implementing.

---

## Target: relay-template/hooks/client/*

| File | Features Required |
|------|-------------------|
| `get-client.jsx` | useState, useEffect, useCallback, fetch, fetchRelative, dynamic import, MarkdownRenderer, theme, url, Layout, plugins, path routing |
| `query-client.jsx` | async component, fetch(QUERY), dynamic import, JSX, params, navigate, env |
| `components/Layout.jsx` | function component, form, input, select, button, onSubmit, onChange, className |
| `components/MovieResults.jsx` | renderMovieResults, map, key, onClick, onMouseEnter, style, conditional JSX |
| `components/MovieView.jsx` | renderMovieView, template literals, optional chaining |
| `components/CreateView.jsx` | FormField, spanarea, form submit, validation |
| `plugin/tmdb.mjs` | handleGetRequest, handleQuery, fetch, async, URLSearchParams |
| `plugin/yts.mjs` | handleQuery, fetch, async |

---

## Phase 1: Parser Foundation

**Scope**: Parse JS-like + JSX-like source into AST.

### 1.1 Lexer
- Tokens: identifiers, keywords, literals (string, number, null, true, false), operators, punctuation
- Keywords: `function`, `const`, `let`, `var`, `return`, `if`, `else`, `for`, `while`, `await`, `async`, `export`, `default`, `import`, `from`
- String literals: single/double quoted, template literals
- Comments: `//`, `/* */`

### 1.2 Parser
- Expressions: binary, unary, call, member, optional chaining, array/object literals
- Statements: variable decl, return, if/else, for, while, try/catch
- Functions: arrow, async, parameters, rest
- JSX: elements, attributes, children, spread, expressions in `{}`
- Modules: import, export default, export named

### 1.3 Unit Tests (Phase 1)

| Test | Source | Expected |
|------|--------|----------|
| `parse_simple_jsx` | `<div>hello</div>` | AST with element, tag, text child |
| `parse_jsx_props` | `<div className="foo" onClick={fn}>x</div>` | props: className (string), onClick (expr) |
| `parse_jsx_children_expr` | `<div>{items.map(x => <span key={x}>{x}</span>)}</div>` | map call, arrow fn, nested JSX |
| `parse_use_state` | `const [a, setA] = useState(0)` | variable decl, array pattern, call |
| `parse_use_effect` | `useEffect(() => { ... }, [deps])` | call with arrow fn, array arg |
| `parse_async_component` | `export default async function hook({ params }) { ... }` | async fn, export default |
| `parse_dynamic_import` | `await import('./plugin/tmdb.mjs')` | await, import call |
| `parse_fetch` | `await fetch(url, { method: 'QUERY' })` | await, call, object literal |
| `parse_template_literal` | `` `https://.../${id}` `` | template literal with expr |
| `parse_optional_chaining` | `movie?.poster_path` | optional chain |
| `parse_try_catch` | `try { ... } catch (err) { ... }` | try/catch block |
| `parse_export_named` | `export function renderMovieResults(...)` | named export |

---

## Phase 2: IR and Semantics ✅

**Scope**: Lower AST to IR (or bytecode); define hook rules and component model.

### 2.1 IR
- Value types: null, bool, number, string, array, object, function ref
- Instructions: load/store, call, branch, loop
- Component/hook metadata: which functions are components, hook call order

### 2.2 Hook Rules
- Hooks called in same order every render
- useState returns [value, setter]
- useEffect(deps) runs after render
- useCallback(fn, deps) returns memoized fn

### 2.3 Unit Tests (Phase 2)

| Test | Scenario |
|------|----------|
| `ir_simple_component` | Function returning JSX → IR with component node |
| `ir_hook_order` | Multiple useState/useEffect → correct ordering |
| `ir_nested_components` | Layout wrapping children → IR tree |
| `ir_async_await` | async fn with await → IR with suspend/resume |
| `ir_dynamic_import` | import() → IR with module load |
| `ir_jsx_spread` | `<div {...props}>` → spread in IR |

---

## Phase 3: Hooks Runtime ✅

**Scope**: Interpret or execute IR with hooks semantics.

### 3.1 State
- Component instance → fiber-like structure
- Hook slots per component instance
- setState triggers re-render

### 3.2 Unit Tests (Phase 3)

| Test | Scenario |
|------|----------|
| `runtime_use_state` | useState(0) → [0, setter]; setter(1) → re-render with [1, setter] |
| `runtime_use_effect` | useEffect runs after render; deps change triggers re-run |
| `runtime_use_callback` | useCallback(fn, []) returns same fn ref across renders |
| `runtime_conditional_hooks` | Hooks in conditional → error or deterministic behavior |
| `runtime_multiple_instances` | Two components each with useState → independent state |

---

## Phase 4: Host Bindings ✅

**Scope**: Minimal API for DOM/native: createElement, setAttribute, addEventListener, etc.

### 4.1 API Surface
- `createElement(tag, props, children)`
- `setAttribute(id, key, value)`
- `addEventListener(id, event, handler)`
- `fetch(url, options)` — host-provided
- `navigate(path)` — host-provided
- `registerThemesFromYaml`, `MarkdownRenderer`, `url` — host modules

### 4.2 Unit Tests (Phase 4)

| Test | Scenario |
|------|----------|
| `host_create_div` | createElement('div', {}, []) → host creates div |
| `host_style_object` | style={{ color: 'red' }} → host applies style |
| `host_on_click` | onClick={fn} → host registers click handler |
| `host_fetch` | fetch(url) → host returns mock response |
| `host_navigate` | navigate('/search/foo') → host receives path |

---

## Phase 5: DOM / Native Renderer

**Scope**: Render IR to actual DOM (web) or native views (Android).

### 5.0 Virtual tree (platform-agnostic) ✅
- `vdom::VNode` / `value_to_vnode` — no DOM dependency
- `vdom::reconcile` — `diff`, `patches`, `vnodes_equal` for hosts and tests

### 5.1 DOM Renderer

**Web (WASM host):**
- VDOM → **batched** host calls (see `WEB_WASM_HOST.md`): create/update/remove nodes, text, attributes, resolved styles, event wiring by stable id.
- Host JS: **apply only**; no Act/React/large JS renderer on the hook-UI path.
- Integration tests: mock host in Rust/WASM; browser e2e against thin shell.

**Shared:**
- Map element tree to host nodes
- Apply className, style (via themed-styler output), event handlers
- Reconcile on updates (apply `vdom::reconcile::patches` or full replace)

### 5.2 Native Renderer (Android)
- Map to native view hierarchy
- themed-styler integration for styles

### 5.3 Unit Tests (Phase 5)

| Test | Scenario |
|------|----------|
| `render_simple_div` | `<div>hello</div>` → DOM div with text |
| `render_form_submit` | form onSubmit → handler called |
| `render_list_keyed` | map with key → correct reconciliation |
| `render_conditional` | {x && <Y />} → Y when x truthy |
| `render_layout` | Layout with children → nested structure |
| `render_movie_results` | MovieResults fixture → list of items |

---

## Phase 6: Module System and Plugins ✅

**Scope**: Dynamic import, plugin resolution, env injection.

### 6.1 Module Loader
- Resolve `./plugin/tmdb.mjs`, `./components/MovieResults.jsx`
- Inject env, params, navigate into hook context

### 6.2 Unit Tests (Phase 6)

| Test | Scenario |
|------|----------|
| `module_import_plugin` | import('./plugin/tmdb.mjs') → module with handleQuery |
| `module_import_component` | import('./components/MovieResults.jsx') → renderMovieResults |
| `module_env` | env.RELAY_PUBLIC_TMDB_API_KEY available |
| `module_params` | params.q, params.page in hook |
| `module_navigate` | navigate passed to hook and components |

---

## Phase 7: Full Integration

**Scope**: Run `get-client.jsx`, `query-client.jsx`, and all components end-to-end.

### 7.0 Fixture layout (in progress) ✅
- `tests/fixtures/client/components/Layout.jsx` — copy of relay-template Layout (parse test **ignored** until parser handles `!`, optional chain, default params, etc.)
- `tests/fixtures/client/mini_ok.jsx` — minimal hook; **active** tests: parse → run → VNode → reconcile

### 7.1 Integration Tests

| Test | Fixture | Assertions |
|------|---------|------------|
| `e2e_get_client` | get-client.jsx + mock server | Renders Layout, routes work |
| `e2e_query_client` | query-client.jsx + mock TMDB | Search returns results UI |
| `e2e_layout` | Layout.jsx | Form submit, branch select |
| `e2e_movie_results` | MovieResults.jsx | List renders, View button works |
| `e2e_movie_view` | MovieView.jsx | Detail view with poster, backdrop |
| `e2e_create_view` | CreateView.jsx | Form fields, submit |
| `e2e_plugin_tmdb` | tmdb.mjs | handleQuery returns items |
| `e2e_plugin_yts` | yts.mjs | handleQuery returns items |

---

## Test Fixtures Location

Copy fixtures from `relay-template/hooks/client/` for integration tests:

```
relay-runtime/
  tests/
    fixtures/
      client/           # From relay-template/hooks/client
        get-client.jsx
        query-client.jsx
        components/
        plugin/
    parse/
    ir/
    runtime/
    host/
    render/
    integration/
```

---

## Reference scenarios (test inspiration — not the shipped stack)

*Relay ships **relay-runtime** + **themed-styler** only; see [POLICY.md](POLICY.md).*

| Prior art | Scenario to cover in relay-runtime tests | Phase |
|-----------|------------------------------------------|-------|
| React-like hooks | useState, useEffect, useCallback | 3 |
| React-like VDOM | reconciliation, composition | 2, 5 |
| **relay-runtime** | JSX + modules parse | 1, 6 |
| **themed-styler** | style resolution, theme vars | 4, 5 |
| Host APIs | fetch, navigate, createElement (Host trait) | 4 |

---

## Implementation Order

1. **Phase 1** → Parser + lexer + parse tests
2. **Phase 2** → IR lowering + IR tests
3. **Phase 3** → Hooks runtime + runtime tests
4. **Phase 4** → Host bindings + host tests
5. **Phase 5** → DOM/native renderer + render tests (**web track:** WASM host contract + `WEB_WASM_HOST.md` checklist)
6. **Phase 6** → Module system + module tests
7. **Phase 7** → Integration tests with real fixtures
