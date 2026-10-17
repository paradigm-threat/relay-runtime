# paradigm-threat-files test fixtures

Mirror of tip sources under `paradigm-threat-files/hooks/…` for layer-1 CleverScript tests
(`paradigm_files_render`, `dirnav_expand`).

When editing the park tip, sync the corresponding files here (DirNav, FeelingLucky, Icons,
loadArticles, hostMode, propOr, jsonUtil, legacyAppHref, Layout for the mini Layout suite,
`hooks/client/native/LiveChat.js`, `hooks/client/native/ChatApp.js` for `livechat_stubs`).

**Nav.jsx** in this tree is a **minimal menu fixture** (open/close `useState` only). The tip’s
full `Nav.jsx` (large `navChromeCss`) overflows the CleverScript stack in unit tests; keep the
mini fixture for layer-1 and cover full Nav paint in park smoke / CDP.
