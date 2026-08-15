# Debugging Plan — relay-runtime

Roadmap for improving debugging, error diagnostics, and developer experience. Implemented in tiers by effort and value.

---

## Current State

| Area | What Exists | Gaps |
|------|-------------|------|
| **Parse errors** | `ParseErrorDetail`: file, line, column, `source_line` | ✓ Good |
| **Runtime errors** | String only: `RuntimeError::Runtime("Undefined: x")` | No location, no call stack |
| **AST** | No spans/offsets on any node | Cannot map runtime errors back to source |
| **Verbose mode** | `set_verbose()` logs component name | No step-level or expression-level tracing |
| **Host** | `RunState` holds `source` | No debug/breakpoint API |
| **DebugMenu (web)** | Copy HTML | No runtime inspection |

---

## Tier 1: Rich Error Context (implemented)

**Effort:** Low | **Value:** High

| Item | Description | Status |
|------|-------------|--------|
| **Call stack on error** | Capture call stack (component/function frames) during execution; attach to runtime errors | ✅ |
| **Locals snapshot on error** | Snapshot current scope (env bindings) when error occurs; include in error payload for host UI | ✅ |

**Output format:** Error JSON gains `call_stack` and `locals`:

```json
{
  "type": "runtime",
  "message": "Undefined: x",
  "call_stack": ["App", "../Foo.jsx", "anonymous"],
  "locals": { "count": 1, "items": "[Array]", "setCount": "[Setter]" }
}
```

---

## Tier 2: AST Spans (enables source locations)

**Effort:** Medium–high | **Value:** High

| Item | Description |
|------|-------------|
| **Source spans on AST** | Add `span: Span { start, end }` (or `Option<Span>`) to `Expr` and `Statement`. Parser captures `stream.peek_start()` when building each node. |
| **Runtime error with location** | New variant `RuntimeError::RuntimeAt { message, loc: SourceLoc }`. When returning `Err`, pass the failing node's span to `line_col_for_offset(source, span.start)`. |
| **Error JSON** | Include `file`, `line`, `column`, `source_line` for all runtime errors. |

**Blocks:** Requires parser and AST refactor across many files.

---

## Tier 3: `debugger` Statement

**Effort:** Medium | **Value:** High for interactive debugging

| Item | Description |
|------|-------------|
| **Lex** | Add `Token::Keyword(Debugger)` for `debugger` |
| **Parse** | `Statement::Debugger` |
| **Execute** | In `eval_stmt`, when `Statement::Debugger`: build context `{ loc, call_stack, locals }`, call host `on_breakpoint(context)`. Host can block (e.g. modal) until user clicks "Continue". |
| **Host contract** | `on_breakpoint?(context: BreakpointContext): void \| Promise<void>` |

**Platform notes:**
- **Web:** Host shows DevTools-style panel or modal; awaits user "Continue".
- **Android:** Native dialog or overlay blocks until user taps.

---

## Tier 4: Step-Through Debugger

**Effort:** High | **Value:** Advanced debugging

| Item | Description |
|------|-------------|
| **before_stmt callback** | Host provides `before_stmt(span, kind)`. Runtime calls before each `eval_stmt`. |
| **Step control** | Host drives: step, step-over, step-in, continue. Runtime checks "paused" flag. |
| **Performance** | Only active when debug mode enabled; guard with feature flag. |

---

## Tier 5: Console / Logging API

**Effort:** Low | **Value:** Medium

| Item | Description |
|------|-------------|
| **Host log** | `log(level: "log"|"warn"|"error", args: Value[])` on Host trait. |
| **Built-in** | Parse `console.log(...)` or `log(...)`; dispatch to host. |
| **CLEVERSCRIPT** | Document in language spec. |

---

## Implementation Status

| Tier | Status |
|------|--------|
| Tier 1 | ✅ Implemented |
| Tier 2 | Planned |
| Tier 3 | Planned |
| Tier 4 | Planned |
| Tier 5 | Planned |

---

## Tests

- `tests/debug_context.rs` — Tier 1: call stack and locals on runtime errors

## Related

- [CLEVERSCRIPT.md](CLEVERSCRIPT.md) — language spec
- [error.rs](../src/error.rs) — error types, `RuntimeErrorDetail`
- [runtime.rs](../src/runtime.rs) — interpreter, `call_stack`, `err()`, `enrich_err()`
