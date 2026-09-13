# Performance feature flags & strategy catalog

**Performance strategy knobs are allowed.** Keep alternate implementations behind named flags/defaults so we can A/B or roll back without deleting code.

This is **not** a license for migration dual stacks (old JS engine vs CleverScript). See [POLICY.md](POLICY.md) carve-out.

## Principles

- Preferred strategy = **default on** (or default code path).
- Alternate strategies stay in tree, gated — do not delete when a better path wins.
- Flags are **behavior toggles**, not legacy product stacks.
- Tip-local strategies use tip constants / hydrate phases; engine strategies use `RELAY_PERF_*` env (native SSR) or host-injected options (WASM later).

Env shape: `RELAY_PERF_<NAME>=0|1|true|false` (unset = documented default).  
SSR thread detection: cancel flag installed via [`ssr_cancel`](../src/ssr_cancel.rs) (gateway `relay-ssr` workers).

---

## Session lessons (root causes)

| Lesson | Mechanism | Preferred fix |
|--------|-----------|---------------|
| Virtual deferred tails | Empty SSR island → client CS walk of 15–40KB tails | Full tip / filled deferred SSR; soft-hydrate keeps HTML; high `HUGE_ARTICLE_LEAD_CHARS` |
| O(n²) CS arrays | No `Array.push`; `concat` in large loops copies each time | Return host arrays as-is; host builders; never loop-`concat` at scale |
| Wrong Accept | Default `text/html` on tip GET → HTML body treated as MD | `Accept: text/markdown`; reject bodies starting with `<` |
| Ungated catalog | DirNav fan-out, FeelingLucky QUERY, dead-link CS anchors early | Phase gates: body ≥3, DirNav expand-path, Lucky/dead-links ≥4 |
| Nested MD / cachedGet | Override-nested `MarkdownRenderer`/`cachedGet` aborts vnode | Lift reads outside MD walk; warm before render |
| Home deferred empty SSR | Nested tip read failed; empty `data-pt-deferred-ssr` | ArticleView `cachedGet` fill; render filled island |
| Long MD SSR cost | md2ast→Value HashMaps→(overrides)→VNode→HTML; `ms_hook` | Rust path on SSR; skip CS overrides; later fast path |
| CS override cliff | Per-node `invoke_handler_value` | Minutes on dense `a`/`h*`/`img`; SSR-light stubs only |
| SSR cache identity | Tip **commit** OID (+ path, hook, web-assets), not per-md blob | Any tip push cold-misses all paths |
| Dead links | Offline daily probe; display lookup only | No probe at render; phase ≥4 CS anchors |
| Gateway starvation | Hung SSR without cancel → accept-queue death | Coalesce + timeout + `ssr_cancel` |

---

## Tip strategies

Park tip hygiene: paradigm-threat-files `hooks/CLEVERSCRIPT_TIP.md`.

| ID | Preferred default | Alternate (keep named) | Status |
|----|-------------------|------------------------|--------|
| `md-full-ssr` | Full tip / SSR-filled deferred + soft-hydrate keep | Virtual lead + empty island (`ARTICLE_TAIL_SRC`) | Preferred shipped |
| `md-cs-overrides` | Rust md on SSR + phase &lt;4; CS `a`/`h`/`img` ≥4 | CS overrides during SSR | Preferred shipped |
| `md-deferred-chunk` | Chunked deferred fill (1.6–2.8KB) | Full CS walk of fragment | Preferred shipped |
| `dirnav-expand-path` | Files only on expand path; warm `loadDirTree` | Full tree+files on phase 2 | Preferred shipped |
| `feeling-lucky-phase4` | Phase ≥4; pick path on click | Random QUERY on early idle | Preferred shipped |
| `accept-markdown` | Tip GET with `accept: text/markdown` | Browser default Accept | Preferred shipped |
| `dead-urls-phase4` | Map load live; mark anchors ≥4; SSR skip | Mark on SSR | Preferred shipped |
| `ssr-omit-anchor-onclick-tip` | Tip omits `anchorOnClick` when `!live` | Always pass click wrappers | Tip follow-up |
| `ssr-deferred-light-overrides` | Stub overrides on home deferred SSR island | Live AutoIndex/ChangeLog/Embed in SSR walk | Tip follow-up |

---

## Gateway strategies

See also [`RELAY_POLICY_AND_PERFORMANCE.md`](../../relay-gateway/docs/RELAY_POLICY_AND_PERFORMANCE.md).

| ID | Preferred default | Alternate | Env / code |
|----|-------------------|-----------|------------|
| `ssr-html-cache` | On | Always cold | `RELAY_DISABLE_SSR_HTML_CACHE=1` |
| `ssr-coalesce` | On | No coalesce | `ssr_limits` |
| `ssr-timeout-cancel` | 12s + cancel | `RELAY_SSR_TIMEOUT_SECS=0` | Existing |
| `ssr-max-concurrent` | 2 | Higher | `RELAY_SSR_MAX_CONCURRENT` |
| `ssr-cache-per-blob` | (future alternate) | Commit-OID key (today) | Not built — catalog only |
| `localhost-http-origin` | http for loopback SSR public origin | https rewrite | Gateway SSR origin |

---

## Runtime flags (`RELAY_PERF_*`)

Implemented in [`src/perf_flags.rs`](../src/perf_flags.rs).

| Env / getter | Default | Wired? | Intent |
|--------------|---------|--------|--------|
| `RELAY_PERF_CS_CANCEL_IN_LOOPS` / `cs_cancel_in_loops` | **true** | Yes | `for`/`while` check `ssr_is_cancelled` |
| `RELAY_PERF_MD_CANCEL_BEFORE_PARSE` / `md_cancel_before_parse` | **true** | Yes | Bail before `md2ast_parse` |
| `RELAY_PERF_MD_SSR_SKIP_ANCHOR_ONCLICK` / `md_ssr_skip_anchor_onclick` | **true** (SSR thread) | Yes | Skip NativeFn per link on SSR |
| `RELAY_PERF_MD_STUB_OVERRIDES_NOOP` / `md_stub_overrides_noop` | **true** | Getter only | Empty stubs skip `invoke_handler_value` (wire later) |
| `RELAY_PERF_MD_SSR_FAST_PATH` / `md_ssr_fast_path` | **false** | Declare | md2ast→VNode/HTML skip Value HashMaps |
| `RELAY_PERF_CACHED_GET_PREFER_MARKDOWN` / `cached_get_prefer_markdown` | **true** | Getter (+ fetch path when applicable) | Client Accept markdown; reject `<` |
| `RELAY_PERF_MD_NESTED_LOUD_FAIL` / `md_nested_loud_fail` | **true** | Declare | Nested MarkdownRenderer fails loud vs empty vnode |
| `RELAY_PERF_MD_FRAGMENT_CACHE` / `md_fragment_cache` | **false** | Declare | Cache fragment by tip blob OID |
| `RELAY_PERF_ARRAY_HOST_BUILDER` / `array_host_builder` | **false** | Declare | Future `@relay` amortized array build |
| `RELAY_PERF_FUSE_SSR_SERIALIZE` / `fuse_ssr_serialize` | **false** | Declare | Fuse Value→VNode→HTML walks on SSR |

---

## Validation gate

1. **Local first** — `park-local.sh`, cold/warm SSR curl, hydrate, Accept markdown, `cargo test`; flag off/on smoke. Do not deploy until green.
2. **Live after** — tip/gateway SHA on fleet; per-node curl home + Crichton; MCP browser hydrate.

---

## Related

- [POLICY.md](POLICY.md) — carve-out vs migration flags
- [CLEVERSCRIPT.md](CLEVERSCRIPT.md) — arrays: no `push`; avoid O(n²) `concat` loops
- [TERMINOLOGY.md](TERMINOLOGY.md) — CS is orchestration; hot work in Rust
- Gateway: `ssr_limits.rs`, `ssr_html_cache.rs`, `ssr_cancel` install on SSR workers
