# Progressive hydrate phases (web · Android · iOS)

Uniform readiness ladder so **no host locks the UI thread** rebuilding the full tip tree on first paint. CleverScript is not Fiber — each tip `setState` remounts the shell — so **hosts** advance phases on idle and tips **gate** work with `isClientLivePhase(n)`.

| Phase | Name | Host action | Tip may enable |
|------:|------|-------------|----------------|
| **0** | cold / SSR | HTML or static shell only | P0 article + P1 absolute `<a>` chrome (no JS) |
| **1** | shell | Hydrate handlers; `set_client_live_phase(1)` | Theme / font / hamburger (`isClientLive()` / phase ≥ 1) |
| **2** | catalog | Idle `doRender` | DirNav, FeelingLucky, neighbors |
| **3** | feeds | Idle `doRender` | ChangeLog, AutoIndex, DynamicIndex / home lists |
| **4** | interactive | Idle `doRender` | TTS dock, gallery open, BackToTop, Search QUERY UI |

## APIs

| Layer | API |
|-------|-----|
| CleverScript (`@clevertree/style-internal`) | `isClientLive()`, `getClientLivePhase()`, `isClientLivePhase(n)` |
| WASM | `set_client_live_phase`, `get_client_live_phase`, `set_client_live` (⇒ phase ≥ 1) |
| Android JNI | `nativeSetClientLivePhase`, `nativeGetClientLivePhase`, `nativeSetClientLive` |
| Web DOM | `html[data-pt-hydrated]`, `html[data-pt-live-phase]` |

`isClientLive()` remains **phase ≥ 1** for backward compatibility.

`set_client_live_phase` is **monotonic** (clamped 0..=4): once past a phase, only `0` / `set_client_live(false)` / `clear_client_live` may reset. That prevents stale idle callbacks from demoting readiness and remounting heavy tip work.

## Host scheduler (normative)

1. Boot with phase **0**; first hook render matches SSR (placeholders for P2+).
2. **Soft-hydrate** existing SSR DOM (best-effort child walk). Mismatch **must not** fail the tree.
3. Promote phase **1**; enable `.pt-client-only` chrome **without wiping** P0 HTML.
4. **Yield for interactive** before first WASM (`yieldForInteractive`: rAF×2 + idle ≤120ms) so absolute header `<a>` clicks are not queued behind `run_hook`.
5. Advance **2 → 3 → 4**, one phase per idle slice. On parked SSR, **only** swap `[data-pt-*-ssr]` / known islands (`aria-label` Directory, Change log, Auto index) — **never** `applyPatches` against soft-hydrated DOM (vnode≠DOM skew corrupts the tree).
6. **Parked SSR (`data-relay-ssr=1`): never `container.innerHTML = ''`.** Full wipe is CSR-only.
7. Never run phases 2–4 synchronously on the hydrate success path.

## Tip rules

- Gate catalog on `isClientLivePhase(2)`, feeds on `(3)`, heavy widgets on `(4)`.
- Do **not** use tip `setState` to stagger phases — that remounts the shell.
- Keep header links as real absolute `<a>` so phase 0 stays navigable.
- **DeferredMarkdown:** prefetch tip `src` at phase **≥2**; fill at **≥3** as a sibling island (stable `section.pt-deferred-md` / `aria-label="Deferred section"`). Long single-file tips may auto-split to lead + virtual tail — never wipe SSR for size.
