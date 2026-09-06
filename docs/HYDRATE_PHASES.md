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

## Host scheduler (normative)

1. Boot with phase **0**; first hook render matches SSR (placeholders for P2+).
2. Attach hydrate handlers; set phase **1**; enable `.pt-client-only` chrome **without** wiping P0 HTML.
3. **Yield** (rAF × 2 + `requestIdleCallback` / Choreographer / `DispatchQueue`).
4. Advance **2 → 3 → 4**, one phase per idle slice; prefer **applyPatches** over full wipe.
5. Never run phases 2–4 synchronously on the hydrate success path.

## Tip rules

- Gate catalog on `isClientLivePhase(2)`, feeds on `(3)`, heavy widgets on `(4)`.
- Do **not** use tip `setState` to stagger phases — that remounts the shell.
- Keep header links as real absolute `<a>` so phase 0 stays navigable.
