# Client hook fixtures (Phase 7)

- **`components/Layout.jsx`** — copied from `relay-template/hooks/client/components/Layout.jsx` for parse/analyze tracking (`integration_fixtures::fixture_layout_parses_and_analyzes`, ignored until parser catches up).
- **`mini_ok.jsx`** — minimal default export used for end-to-end parse → run → VNode → reconcile tests.

Refresh `Layout.jsx` from relay-template when updating snapshots:

```bash
cp "$RELAY_TEMPLATE/hooks/client/components/Layout.jsx" tests/fixtures/client/components/
```
