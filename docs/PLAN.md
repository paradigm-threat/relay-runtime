# Client hook language: bridge strategy vs. from-scratch language

This doc analyzes the **previous** “emulate JavaScript everywhere” strategy and the chosen direction: **relay-runtime** — a JavaScript-like, React-hooks-like language with **its own Rust implementation** (native + WASM). The same idea applies to **Songwalker Core** (music language that works everywhere with one compiler/runtime).

**Relay today:** follow **[POLICY.md](POLICY.md)** — **no legacy** execution paths, **no** hook transpile-to-JS, **no** hook-transpiler on the product stack; **themed-styler** stays a separate library. **[NEXT_STEPS.md](NEXT_STEPS.md)** lists concrete work.

---

## 1. Prior strategy (historical context — not maintained for Relay hooks)

### 1.1 Architecture (as built)

| Layer | Role |
|-------|------|
| **jscbridge** | Kotlin/Java → JNI → C++ → **JavaScriptCore C API**. Provides a single JS runtime on Android (and eventually others). Registers native modules, CommonJS shim, console bridge. |
| **act** | **React-like runtime**: hooks (`useState`, `useEffect`, …), VDOM, reconciler. Two renderers: **act/dom** (browser) and **act/android** (native via bridge). Runs **inside** the JSC (or browser) environment; depends on a real JS engine. |
| **hook-transpiler** | **Rust** (native + WASM): parses JSX and ES6 module syntax; transpiles JSX → `__hook_jsx_runtime.jsx(…)` and ES6 → CommonJS. Output is still **JavaScript** that runs in JSC or the browser. |
| **themed-styler** | **Rust** (native + WASM): CSS-like styling and theming; dp/sp → pixels, variables, selectors. Used from JS (via WASM or native bindings) to compute style objects. |
| **relay-client-android** | Loads **client hooks** (JSX/JS) from the server; uses **JSC** + **bridge.js** (loader, `__hook_import`, Act/React, `__hook_jsx_runtime`); renders via **Act** → native views. |
| **relay-client-web** | **Only path:** load hook **source** as bytes → **relay-runtime** (WASM) parses + runs + builds VDOM; **themed-styler** (library, WASM or native) for styles; **browser JavaScript is glue only** (load WASM, DOM FFI, fetch, timers). **No** transpiler, **no** Act/React for hook UI. |

End-to-end: **author once** in a JS/JSX + hooks style → **relay-runtime** parses and executes **directly** (native or WASM) → **host** applies DOM/native views. On **web**, the host is a **thin JS shim**; on **Android**, native bindings—same core, different FFI. See [WEB_WASM_HOST.md](WEB_WASM_HOST.md) and [POLICY.md](POLICY.md).

### 1.2 What makes this a huge task

- **JSC bridge**: Embedding JavaScriptCore, exposing a safe Kotlin/Java API, and keeping object/function boundaries correct is non-trivial. You are effectively maintaining a small "React Native–like" JS host.
- **ACT as a React copy**: To feel like React and support the same mental model (hooks, composition, reconciliation), ACT reimplements a large slice of React's behavior and contracts. Every divergence (lifecycle, batching, Suspense, etc.) is a compatibility and maintenance cost.
- **ES6/JS surface**: As soon as you say "same as JavaScript," you inherit **prototypes**, **this**, **async/await**, **Proxy**, **Symbol**, **iterators**, and the rest. Matching ES6 (or even a "safe" subset) across engines and bridge boundaries is a long-term game of whack-a-mole.
- **Client hook as "script"**: The **client hook** is the desired abstraction: one script that describes components, styles, and behavior in a JS/React-like way and runs everywhere. Today that script **is** JavaScript (or transpiled JS), so the only way to run it on Android is to ship a full JS engine and bridge. The complexity is in making "one language" (JS) run in two very different environments (browser + native) with one UI abstraction (Act).

So: **rebuilding everything to match existing ES6 + React** is a huge challenge and will remain a hassle even when it works. The question is whether there is a **different way** to get the same outcome (one authoring language, one mental model, all platforms).

---

## 2. Goal (unchanged)

- **Universal rendering interface**: one "client hook" that can script **components**, **styles**, and behavior.
- **JavaScript-like and React-hooks-like** authoring experience (familiar syntax and semantics).
- **All operating systems**: web, Android, iOS, desktop, etc., without relying on a single vendor's JS engine or a heavy bridge.

The shift is: **that interface does not have to be implemented by a real JavaScript engine**. It only has to **look and feel** like JS + hooks to the author.

---

## 3. Alternative: a language that looks like JS + hooks but has its own engines

### 3.1 Core idea

- Define a **small, well-specified language** (**CleverScript**) that is **intentionally close to JavaScript** (syntax, expressions, control flow) and **identical in spirit to React hooks** (components are functions; `useState`, `useEffect`, etc. exist and behave the same way). See [CLEVERSCRIPT.md](CLEVERSCRIPT.md) for the supported subset.
- This language **is not JavaScript**: no requirement to execute in V8/JSC/SpiderMonkey. You implement **your own parser, IR, and runtime(s)** (interpreted, JIT, or AOT-compiled).
- **Multiple runtimes**:
  - **Native** (e.g. Rust/C++): Android, iOS, desktop — no JS engine.
  - **Web**: **WASM** — relay-runtime in the browser, talking to the DOM via a **tiny FFI** ([WEB_WASM_HOST.md](WEB_WASM_HOST.md)). **Relay does not** target “compile hooks to JS for execution.”
- **Keep it really simple**: no `eval`, no full `Proxy`, no need to support every ES6 feature. Only what's needed for the **client hook** use case: components, hooks, conditionals, loops, simple data structures, and a small set of host bindings (styles, layout, events, maybe fetch).

This is the same philosophy as **Songwalker Core**: a **domain-specific language** (music) with **one grammar and one semantics**, implemented in **Rust**, with **one compiler** and **multiple targets** (WASM for browser, native for CLI). You don't emulate an existing runtime; you define the language and implement it.

### 3.2 Why it could beat JavaScript (for this domain)

- **No legacy**: no `this`, no prototype chain, no "optional" semicolons and ASI. Parser and IR stay small and predictable.
- **AOT / static shape**: you know exactly what "components" and "hooks" exist; you can optimize layout, state, and updates for that model instead of optimizing general-purpose JS.
- **Controlled surface**: the only "API" the script sees is what you expose (e.g. `View`, `Text`, `useState`, `useEffect`, `StyleSheet`). No `window`, no `document` (unless you explicitly expose a minimal DOM facade). Smaller surface → easier to secure, reason about, and optimize.
- **Single implementation**: one parser, one type-checker (if you want), **native + WASM** targets for Relay. No maintaining parity with a moving ES6+ target in a second execution engine.

### 3.3 Full integration with the web (Relay)

**Relay uses WASM only for hook execution:** Parser + interpreter (**relay-runtime**) and **themed-styler** (separate crate) run in WASM (or native on Android). They own **all rendering semantics** (tree, hooks, styles). The browser host provides **platform capabilities** only: instantiate module, memory, **DOM mutations** via imports, `fetch`, `requestAnimationFrame`, etc.—**glue**, not a second renderer (**no** Act/React on the hook path). See [WEB_WASM_HOST.md](WEB_WASM_HOST.md).

*Historical note:* A hypothetical “compile to minimal JS” target (old “Option B”) is **out of scope for Relay**; see [POLICY.md](POLICY.md).

### 3.4 Relation to Songwalker Core

Songwalker is a **music language**: custom syntax and semantics, one compiler (Rust), one core runtime (DSP, scheduler). It targets **browser (WASM)** and **native (CLI)**. It does **not** try to be JavaScript; it only has to be good at expressing music and running the same program everywhere.

The same idea for **client hooks**: a **UI/component language** that is **JS-like and hooks-identical in design**, but is **defined and implemented by you**. One grammar, **native + WASM** backends, **no** JSC bridge and **no** “run real JS” for hooks. Web interop is **WASM ↔ small JS host FFI** only ([WEB_WASM_HOST.md](WEB_WASM_HOST.md)).

---

## 4. Repos: Relay hook stack (no legacy)

| Repo | Role |
|------|------|
| **relay-runtime** | **Source of truth** for hook language: parser, IR, runtime, VDOM, host trait; **native + WASM**. |
| **themed-styler** | **Separate library** — themes / style resolution; dependency of relay-runtime (and optionally hosts). |
| **relay-client-android** | Host app; **relay-runtime native** + themed-styler — **remove** JSC/act/bridge hook pipeline when adopting this stack ([POLICY.md](POLICY.md)). |
| **relay-client-web** | Host app; **minimal JS shell** + **relay-runtime WASM** + themed-styler — **remove** transpiler/Act/React hook execution paths. |
| **relay-server** | **Keep**. Serves hook source; unchanged contract. |
| **jscbridge** | **Not part of** Relay hook execution going forward. |
| **act** | **Not part of** Relay hook execution going forward. |
| **hook-transpiler** | **Not part of** Relay — **deprecated** for this product; relay-runtime parser replaces it (**no** transpile-to-JS step). |

**relay-runtime** (this repo) is **greenfield**: no legacy modules. See **[NEXT_STEPS.md](NEXT_STEPS.md)** for sibling-repo teardown tasks.

---

## 5. References

- **[POLICY.md](POLICY.md)** — no legacy, no hook transpile, themed-styler separate.
- **[NEXT_STEPS.md](NEXT_STEPS.md)** — ordered implementation and client teardown.
- **[WEB_WASM_HOST.md](WEB_WASM_HOST.md)** — web: WASM core; host JS glue only.
- **themed-styler**, **relay-client-android**, **relay-client-web**, **relay-server**, **Songwalker Core**.
