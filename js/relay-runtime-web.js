/**
 * Vendored from `relay-runtime/js/relay-runtime-web.js`.
 * Import path points at this package's wasm-pack output (see `npm run build:wasm`).
 */
import init, {
  run_hook_with_host,
  get_themes,
  set_theme_and_render,
  get_injected_css,
  push_message_from_host,
  invoke_handler,
  consume_render_requested,
  set_client_live,
  is_client_live,
  set_client_live_phase,
  get_client_live_phase,
  get_current_theme_name,
} from '../wasm/relay_runtime_pkg/relay_runtime.js';

let _styleId = 'relay-ssr-theme';

/** Yield so pending clicks/paint can run before a long WASM sync stretch. */
function yieldToMain() {
  return new Promise((resolve) => {
    try {
      if (typeof scheduler !== 'undefined' && typeof scheduler.yield === 'function') {
        scheduler.yield().then(resolve, () => setTimeout(resolve, 0));
        return;
      }
    } catch (_) {
      /* optional */
    }
    try {
      if (typeof requestAnimationFrame === 'function') {
        requestAnimationFrame(() => setTimeout(resolve, 0));
        return;
      }
    } catch (_) {
      /* optional */
    }
    setTimeout(resolve, 0);
  });
}

/**
 * Parked SSR: flush paint + pending input before the first WASM `run_hook`.
 * Absolute header `<a>` must stay hittable; a sync WASM stretch otherwise queues clicks.
 */
export async function yieldForInteractive(preferIdleMs = 120) {
  await yieldToMain();
  await new Promise((resolve) => {
    try {
      if (typeof requestAnimationFrame === 'function') {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => resolve());
        });
        return;
      }
    } catch (_) {
      /* optional */
    }
    setTimeout(resolve, 0);
  });
  await yieldToMain();
  await new Promise((resolve) => {
    try {
      if (typeof requestIdleCallback === 'function') {
        requestIdleCallback(() => resolve(), { timeout: Math.max(0, preferIdleMs | 0) });
        return;
      }
    } catch (_) {
      /* optional */
    }
    setTimeout(resolve, 0);
  });
}

/** Document that owns `container` (iframe-safe; never assume the SPA `document`). */
export function documentFor(container) {
  if (container && container.ownerDocument) return container.ownerDocument;
  return typeof document !== 'undefined' ? document : null;
}

/** Parent for injected theme CSS: ShadowRoot, else the mount document's <head> (iframe-safe). */
export function styleParentFor(container) {
  if (container && typeof container.getRootNode === 'function') {
    const root = container.getRootNode();
    if (root && typeof ShadowRoot !== 'undefined' && root instanceof ShadowRoot) {
      return root;
    }
  }
  const doc = documentFor(container);
  return doc ? doc.head : null;
}

export function themeTargetFor(container) {
  if (container && typeof container.getRootNode === 'function') {
    const root = container.getRootNode();
    if (root && typeof ShadowRoot !== 'undefined' && root instanceof ShadowRoot) {
      return container.closest?.('[data-relay-theme-scope="hook"]')
        || root.querySelector?.('[data-relay-theme-scope="hook"]')
        || container;
    }
  }
  const doc = documentFor(container);
  return doc ? doc.documentElement : null;
}

/** Ensure a <style id=...> exists on parent (document.head or a ShadowRoot). */
export function ensureStyleElement(id, parent) {
  if (!parent) return null;
  const doc = parent.ownerDocument || (typeof document !== 'undefined' ? document : null);
  if (!doc) return null;
  let el = parent.querySelector ? parent.querySelector(`style#${id}`) : null;
  if (!el && typeof doc.getElementById === 'function') {
    const existing = doc.getElementById(id);
    // Same-document stray node only — never steal `#relay-ssr-theme` from a parent SPA.
    if (existing && existing.tagName === 'STYLE' && existing.ownerDocument === doc) {
      parent.appendChild(existing);
      el = existing;
    }
  }
  if (!el) {
    el = doc.createElement('style');
    el.id = id;
    parent.appendChild(el);
  } else if (el.parentNode !== parent) {
    parent.appendChild(el);
  }
  return el;
}

/** Apply CSS from the last run_hook_with_host into the mount's root (shadow or document). */
function applyInjectedCss(container) {
  try {
    const css = typeof get_injected_css === 'function' ? get_injected_css() : '';
    const parent = styleParentFor(container);
    if (css && String(css).trim()) {
      const styleEl = ensureStyleElement(_styleId, parent);
      if (styleEl) styleEl.textContent = String(css);
    }
  } catch (_) {
    /* optional */
  }
  syncHtmlThemeClass(themeTargetFor(container));
}

/**
 * Mirror CleverScript/WASM theme name onto <html class="light|dark"> for system dual CSS.
 * Park tips that flip appearance without setCurrentTheme store the choice on
 * data-pt-theme — prefer that so catalog remounts do not force theme.yaml light.
 */
function syncHtmlThemeClass(target) {
  if (typeof document === 'undefined') return;
  const root = target || document.documentElement;
  let tip = '';
  try {
    tip = String(root.getAttribute('data-pt-theme') || '').trim();
  } catch (_) {
    tip = '';
  }
  if (tip === 'light' || tip === 'dark') {
    try {
      root.setAttribute('class', tip);
    } catch (_) {
      /* classList optional */
      try {
        root.classList.remove('light', 'dark');
        root.classList.add(tip);
      } catch (_2) { /* ignore */ }
    }
    return;
  }
  let name = '';
  try {
    if (typeof get_current_theme_name === 'function') {
      name = String(get_current_theme_name() || '').trim();
    }
  } catch (_) {
    return;
  }
  if (name !== 'light' && name !== 'dark') return;
  root.classList.remove('light', 'dark');
  root.classList.add(name);
}

/**
 * Park tips style inactive chrome via html[data-pt-hydrated] + data-pt-live-phase.
 * Phase 0 = SSR/cold · 1 = shell · 2 = catalog · 3 = feeds · 4 = interactive.
 * Gateway SSR HTML should emit data-pt-hydrated="0"; this flips as the host advances.
 */
export function syncHtmlHydratedAttr(live, container) {
  const doc = documentFor(container);
  if (!doc) return;
  try {
    const root = doc.documentElement;
    if (!root || typeof root.setAttribute !== 'function') return;
    root.setAttribute('data-pt-hydrated', live ? '1' : '0');
    let phase = live ? 1 : 0;
    try {
      if (typeof get_client_live_phase === 'function') {
        phase = Number(get_client_live_phase()) || phase;
      }
    } catch (_) {
      /* optional until wasm rebuild */
    }
    root.setAttribute('data-pt-live-phase', String(phase));
  } catch (_) {
    /* optional */
  }
}

export function syncHtmlLivePhaseAttr(phase, container) {
  const doc = documentFor(container);
  if (!doc) return;
  try {
    const root = doc.documentElement;
    if (!root || typeof root.setAttribute !== 'function') return;
    const p = Math.max(0, Math.min(4, Number(phase) || 0));
    root.setAttribute('data-pt-live-phase', String(p));
    root.setAttribute('data-pt-hydrated', p >= 1 ? '1' : '0');
  } catch (_) {
    /* optional */
  }
}

/**
 * Optional same-origin RUM beacon (gateway `POST /api/rum` when RELAY_RUM_ENABLED=1).
 * Records TTFC (first hit-testable nav link) + LCP/INP/CLS when PerformanceObserver exists.
 */
export function installRumBeacon(container, { hydrateOk = true } = {}) {
  try {
    const doc = documentFor(container) || document;
    const win = doc.defaultView || window;
    if (!win || typeof win.fetch !== 'function') return;
    const sample = (() => {
      try {
        const m = /(?:^|[?&])rum_sample=([0-9.]+)/.exec(win.location.search || '');
        if (m) return Math.min(1, Math.max(0, parseFloat(m[1])));
      } catch (_) { /* optional */ }
      return 1;
    })();
    if (Math.random() > sample) return;
    if (win.__relayRumInstalled) return;
    win.__relayRumInstalled = true;
    const t0 = (win.performance && performance.now) ? performance.now() : Date.now();
    const payload = {
      path: (win.location && win.location.pathname) || '/',
      embed_oid: null,
      hydrate_ok: !!hydrateOk,
      ttfc_ms: null,
      lcp_ms: null,
      inp_ms: null,
      cls: null,
    };
    try {
      const scripts = doc.querySelectorAll('script[src*="/assets/"]');
      for (const s of scripts) {
        const m = /\/assets\/c\/([0-9a-f]{7,40})\//i.exec(s.src || '');
        if (m) {
          payload.embed_oid = m[1];
          break;
        }
      }
    } catch (_) { /* optional */ }

    const send = () => {
      try {
        const body = JSON.stringify(payload);
        if (typeof navigator !== 'undefined' && typeof navigator.sendBeacon === 'function') {
          const blob = new Blob([body], { type: 'application/json' });
          if (navigator.sendBeacon('/api/rum', blob)) return;
        }
        win.fetch('/api/rum', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body,
          keepalive: true,
        }).catch(() => {});
      } catch (_) { /* optional */ }
    };

    let sent = false;
    const maybeSend = () => {
      if (sent) return;
      if (payload.ttfc_ms == null) return;
      sent = true;
      // Allow LCP a moment; then flush.
      setTimeout(send, 2500);
    };

    const markTtfc = () => {
      if (payload.ttfc_ms != null) return;
      const now = (win.performance && performance.now) ? performance.now() : Date.now();
      payload.ttfc_ms = Math.round(now - t0);
      maybeSend();
    };

    try {
      const nav = doc.querySelector('nav a[href], a[href]');
      if (nav) {
        const r = nav.getBoundingClientRect();
        if (r.width > 0 && r.height > 0) {
          const x = Math.min(win.innerWidth - 2, Math.max(2, r.left + 4));
          const y = Math.min(win.innerHeight - 2, Math.max(2, r.top + 4));
          const hit = doc.elementFromPoint(x, y);
          if (hit && (hit === nav || nav.contains(hit))) markTtfc();
        }
      }
    } catch (_) { /* optional */ }

    try {
      doc.addEventListener(
        'pointerdown',
        (e) => {
          const a = e.target && e.target.closest && e.target.closest('a[href]');
          if (a) markTtfc();
        },
        { once: true, capture: true },
      );
    } catch (_) { /* optional */ }

    try {
      if (typeof PerformanceObserver === 'function') {
        const po = new PerformanceObserver((list) => {
          for (const e of list.getEntries()) {
            if (e.entryType === 'largest-contentful-paint') {
              payload.lcp_ms = Math.round(e.startTime);
            } else if (e.entryType === 'event' && e.name === 'event' && e.duration != null) {
              // INP-ish: keep max interaction delay when available
              const d = e.duration;
              if (payload.inp_ms == null || d > payload.inp_ms) payload.inp_ms = Math.round(d);
            } else if (e.entryType === 'layout-shift' && !e.hadRecentInput) {
              payload.cls = (payload.cls || 0) + (e.value || 0);
            }
          }
        });
        try {
          po.observe({ type: 'largest-contentful-paint', buffered: true });
        } catch (_) { /* optional */ }
        try {
          po.observe({ type: 'layout-shift', buffered: true });
        } catch (_) { /* optional */ }
        try {
          po.observe({ type: 'event', buffered: true, durationThreshold: 16 });
        } catch (_) { /* optional */ }
      }
    } catch (_) { /* optional */ }

    // Fallback TTFC if hit-test was early but not marked
    setTimeout(() => {
      if (payload.ttfc_ms == null) markTtfc();
      maybeSend();
    }, 3000);
  } catch (_) {
    /* optional */
  }
}

const unitlessStyleProperties = new Set([
  'animationIterationCount', 'columnCount', 'fillOpacity', 'flex', 'flexGrow',
  'flexShrink', 'fontWeight', 'gridColumn', 'gridRow', 'lineHeight', 'opacity',
  'order', 'orphans', 'scale', 'strokeOpacity', 'strokeWidth', 'tabSize',
  'widows', 'zIndex', 'zoom',
]);

/** Match React style semantics: non-zero numeric dimensions become CSS pixels. */
function normalizeStyleValue(property, value) {
  const stringValue = String(value).trim();
  if (
    stringValue === ''
    || property.startsWith('--')
    || unitlessStyleProperties.has(property)
    || !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(stringValue)
    || Number(stringValue) === 0
  ) {
    return stringValue;
  }
  return `${stringValue}px`;
}

const SVG_NS = 'http://www.w3.org/2000/svg';
/** Lowercase tags that must be created in the SVG namespace (createElement puts them in HTML). */
const SVG_TAGS = new Set([
  'svg', 'path', 'circle', 'rect', 'line', 'polyline', 'polygon', 'ellipse',
  'g', 'defs', 'use', 'symbol', 'text', 'tspan', 'clippath', 'mask',
  'lineargradient', 'radialgradient', 'stop', 'foreignobject', 'image',
  'marker', 'pattern', 'title', 'desc',
]);
/** Mixed-case SVG tag names required by createElementNS. */
const SVG_TAG_NAME = {
  clippath: 'clipPath',
  lineargradient: 'linearGradient',
  radialgradient: 'radialGradient',
  foreignobject: 'foreignObject',
};

function createVnodeDomElement(tag) {
  const lower = String(tag || '').toLowerCase();
  if (SVG_TAGS.has(lower)) {
    return document.createElementNS(SVG_NS, SVG_TAG_NAME[lower] || lower);
  }
  return document.createElement(tag);
}

function setVnodeDomAttribute(el, key, str) {
  if (key === 'className') {
    if (el.namespaceURI === SVG_NS) el.setAttribute('class', str);
    else el.className = str;
    return;
  }
  if (key.startsWith('data') && key.length > 4) {
    const attr = 'data-' + key.slice(4).replace(/([A-Z])/g, '-$1').toLowerCase().replace(/^-/, '');
    el.setAttribute(attr, str);
    return;
  }
  el.setAttribute(key, str);
}

/**
 * Apply VNode props onto a DOM element (strings, numbers, bools, style, handlers).
 * @param {Element} el
 * @param {string} tag
 * @param {object} props
 * @param {function(number): void} invokeHandler
 * @param {function(): void|null} onAfterInvoke
 */
function applyVnodeDomProps(el, tag, props, invokeHandler, onAfterInvoke) {
  if (!props) return;
  const relayControlled = propBoolFromVnodeProps(props, 'relayControlled');
  for (const [k, val] of Object.entries(props)) {
    if (val && typeof val === 'object' && (val.type === 'Handler' || val.type === 'HandlerId') && typeof val.value === 'number') {
      const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
      if (tag === 'a' && eventType === 'click' && !relayControlled) {
        continue;
      }
      const handlerId = val.value;
      el.addEventListener(eventType, (e) => {
        if (tag === 'a' && eventType === 'click' && relayControlled) {
          if (!shouldInterceptRelayControlledAnchorClick(e, el)) return;
          e.preventDefault();
        }
        try {
          invokeHandler(handlerId);
        } catch (err) {
          console.error('[relay-runtime] Handler invocation failed:', err);
          throw err;
        }
        if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
      });
      continue;
    }
    if (k === 'style') {
      const styleObj = val && typeof val === 'object' && val.type === 'Style' ? val.value : val;
      if (styleObj && typeof styleObj === 'object') {
        for (const [pk, pv] of Object.entries(styleObj)) {
          if (pv != null && typeof pv !== 'object') {
            el.style[pk] = normalizeStyleValue(pk, pv);
          }
        }
      }
      continue;
    }
    if (val && typeof val === 'object' && val.type === 'Bool') {
      if (val.value) setVnodeDomAttribute(el, k, '');
      else if (k === 'disabled' || k === 'checked' || k === 'selected' || k === 'readOnly') {
        el[k === 'readOnly' ? 'readOnly' : k] = false;
      }
      continue;
    }
    if (typeof val === 'boolean') {
      if (val) setVnodeDomAttribute(el, k, '');
      continue;
    }
    const str =
      val && typeof val === 'object' && val.type === 'String' && val.value !== undefined
        ? val.value
        : typeof val === 'string'
          ? val
          : val && typeof val === 'object' && val.type === 'Number' && val.value !== undefined
            ? String(val.value)
            : typeof val === 'number'
              ? String(val)
              : null;
    if (str != null) setVnodeDomAttribute(el, k, str);
  }
}

function appendVnodeDomChildren(el, props, children, invokeHandler, onAfterInvoke) {
  if (!children || !Array.isArray(children)) return;
  const parentStyle = props?.style?.value ?? props?.style ?? {};
  const isRow = parentStyle.flexDirection === 'row' || (!parentStyle.flexDirection && parentStyle.display === 'flex');
  const spacing = isRow ? (parentStyle.spaceX ?? parentStyle.gap) : (parentStyle.spaceY ?? parentStyle.gap);
  for (let i = 0; i < children.length; i++) {
    const child = vnodeToDom(children[i], invokeHandler, onAfterInvoke);
    if (child) {
      if (i > 0 && spacing != null && child.style) {
        const px = typeof spacing === 'number' ? spacing + 'px' : String(spacing);
        child.style[isRow ? 'marginLeft' : 'marginTop'] = px;
      }
      el.appendChild(child);
    }
  }
}

/**
 * Install globalThis.__relay_host_fetch(url, initJson) for CleverScript Host.fetch (sync XHR).
 * Safe to call multiple times; first wins unless already set.
 */
export function installRelayHostFetch() {
  if (typeof globalThis === 'undefined') return;
  if (typeof globalThis.__relay_host_fetch === 'function') return;
  globalThis.__relay_host_fetch = (url, initJson) => {
    let method = 'GET';
    let headers = null;
    let body = null;
    let responseType = '';
    try {
      if (initJson && String(initJson).length) {
        const o = JSON.parse(String(initJson));
        if (o && o.method) method = String(o.method).toUpperCase();
        if (o && o.headers && typeof o.headers === 'object') headers = o.headers;
        if (o && typeof o.body === 'string') body = o.body;
        if (o && typeof o.responseType === 'string') responseType = o.responseType;
      }
    } catch {
      /* ignore */
    }
    let fetchUrl = String(url ?? '');
    if (fetchUrl && !/^https?:\/\//i.test(fetchUrl)) {
      const base = globalThis.__relay_fetch_base_url;
      if (typeof base === 'string' && base.trim()) {
        try {
          const baseNorm = base.trim().replace(/\/+$/, '') + '/';
          fetchUrl = new URL(fetchUrl, baseNorm).href;
        } catch {
          /* keep original url */
        }
      }
    }
    try {
      const xhr = new XMLHttpRequest();
      xhr.open(method, fetchUrl, false);
      const wantBase64 = responseType === 'base64' || responseType === 'arraybuffer';
      if (wantBase64) xhr.responseType = 'arraybuffer';
      const branch = globalThis.__relay_fetch_branch;
      if (typeof branch === 'string' && branch.trim()) {
        xhr.setRequestHeader('X-Relay-Branch', branch.trim());
      }
      if (headers) {
        for (const [k, v] of Object.entries(headers)) {
          if (v == null) continue;
          xhr.setRequestHeader(String(k), String(v));
        }
      }
      if (body != null && method !== 'GET' && method !== 'HEAD') {
        if (!headers || headers['Content-Type'] == null && headers['content-type'] == null) {
          xhr.setRequestHeader('Content-Type', 'application/json');
        }
        xhr.send(body);
      } else {
        xhr.send(null);
      }
      const ok = xhr.status >= 200 && xhr.status < 300;
      if (wantBase64) {
        const buf = xhr.response;
        let b64 = '';
        if (buf && buf.byteLength != null) {
          const bytes = new Uint8Array(buf);
          const chunk = 0x8000;
          let binary = '';
          for (let i = 0; i < bytes.length; i += chunk) {
            binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
          }
          b64 = btoa(binary);
        }
        const ct = xhr.getResponseHeader('Content-Type') || 'application/octet-stream';
        return JSON.stringify({
          ok,
          status: xhr.status,
          text: '',
          base64: b64,
          contentType: ct,
        });
      }
      return JSON.stringify({
        ok,
        status: xhr.status,
        text: xhr.responseText != null ? String(xhr.responseText) : '',
      });
    } catch (e) {
      return JSON.stringify({
        ok: false,
        status: 0,
        text: '',
        error: String(e?.message ?? e),
      });
    }
  };
}

/** Resolve CleverScript `fetch('/path')` against remote hook gateway base. */
export function setRelayFetchBaseUrl(baseUrl) {
  if (typeof globalThis === 'undefined') return;
  const b = (baseUrl ?? '').trim().replace(/\/+$/, '');
  globalThis.__relay_fetch_base_url = b || undefined;
}

/** Prefer the same branch SSR used (`X-Relay-Branch` on host fetches). */
export function setRelayFetchBranch(branch) {
  if (typeof globalThis === 'undefined') return;
  const b = (branch ?? '').trim();
  globalThis.__relay_fetch_branch = b || undefined;
}

async function fetchThemeYamlForHost(baseDir, branch) {
  const paths = [
    'hooks/client/theme.yaml',
    'theme.yaml',
    'hooks/theme.yaml',
  ];
  const headers = {};
  if (branch) headers['X-Relay-Branch'] = branch;
  for (const p of paths) {
    try {
      const resp = await fetch(resolveHookUrl(baseDir, p) + '?t=' + Date.now(), { headers });
      if (resp.ok) {
        const text = await resp.text();
        if (text && text.trim()) return text;
      }
    } catch {
      /* try next */
    }
  }
  return '';
}

/**
 * Local tip import specs (`hooks/...`, `./...`, `../...`) — not `@relay/*` / packages.
 * Mirrors gateway SSR `local_import_specs` in ssr_client_hook.rs.
 * @param {string} source
 * @returns {string[]}
 */
export function localImportSpecs(source) {
  const out = [];
  let rest = String(source ?? '');
  while (true) {
    const idx = rest.indexOf(' from ');
    if (idx === -1) break;
    const after = rest.slice(idx + 6).trimStart();
    let q = '';
    if (after.startsWith("'")) q = "'";
    else if (after.startsWith('"')) q = '"';
    else {
      rest = rest.slice(idx + 6);
      continue;
    }
    const body = after.slice(1);
    const end = body.indexOf(q);
    if (end === -1) break;
    const spec = body.slice(0, end);
    if (
      spec.startsWith('hooks/') ||
      spec.startsWith('./') ||
      spec.startsWith('../') ||
      spec.endsWith('.jsx') ||
      spec.endsWith('.js') ||
      spec.endsWith('.yaml') ||
      spec.endsWith('.yml')
    ) {
      out.push(spec);
    }
    rest = body.slice(end + 1);
  }
  return out;
}

/**
 * Resolve a local import spec relative to the importing module path.
 * Mirrors gateway SSR `resolve_import_rel`.
 * @param {string} importer
 * @param {string} spec
 * @returns {string}
 */
export function resolveImportRel(importer, spec) {
  const s = String(spec ?? '');
  if (s.startsWith('hooks/') || !s.startsWith('.')) {
    return s.replace(/^\/+/, '');
  }
  const imp = String(importer ?? '').replace(/^\/+/, '');
  const slash = imp.lastIndexOf('/');
  const base = slash === -1 ? '' : imp.slice(0, slash);
  const joined = (base ? `${base}/` : '') + s;
  const parts = [];
  for (const c of joined.split('/')) {
    if (c === '..') parts.pop();
    else if (c && c !== '.') parts.push(c);
  }
  return parts.join('/');
}

/**
 * Insert source under both `rel` and `/rel` (SSR `insert_module` parity).
 * @param {Record<string, string>} moduleMap
 * @param {string} pathKey
 * @param {string} text
 */
export function putModuleInMap(moduleMap, pathKey, text) {
  const k = String(pathKey ?? '').trim();
  if (!k || typeof text !== 'string') return;
  if (!looksLikeHookSource(text)) return;
  const rel = k.replace(/^\/+/, '');
  moduleMap[rel] = text;
  moduleMap[`/${rel}`] = text;
  if (k !== rel && k !== `/${rel}`) moduleMap[k] = text;
}

/** Skip SPA HTML fallbacks so import closure does not crawl index.html forever. */
export function looksLikeHookSource(text) {
  const t = String(text ?? '').trimStart();
  if (!t) return false;
  const head = t.slice(0, 64).toLowerCase();
  if (head.startsWith('<!doctype') || head.startsWith('<html')) return false;
  return true;
}

function moduleMapHas(moduleMap, rel) {
  const r = String(rel ?? '').replace(/^\/+/, '');
  return Object.prototype.hasOwnProperty.call(moduleMap, r)
    || Object.prototype.hasOwnProperty.call(moduleMap, `/${r}`);
}

/** Hard cap so a mis-served HTML tree cannot unbounded-BFS. */
export const IMPORT_CLOSURE_MAX_MODULES = 256;

/**
 * BFS of static local imports so CSR matches gateway SSR `preload_import_closure`.
 * `fetchSource(rel)` should return module text or null/undefined when missing.
 * @param {Record<string, string>} moduleMap
 * @param {(rel: string) => Promise<string|null|undefined>} fetchSource
 * @param {(rel: string) => void} [onLoaded]
 * @returns {Promise<Record<string, string>>}
 */
export async function preloadImportClosure(moduleMap, fetchSource, onLoaded) {
  const seen = new Set();
  let frontier = Object.keys(moduleMap).filter((k) => !k.startsWith('/'));
  while (frontier.length) {
    if (moduleCount(moduleMap) >= IMPORT_CLOSURE_MAX_MODULES) {
      console.warn('[relay-runtime] import closure hit', IMPORT_CLOSURE_MAX_MODULES, 'module cap');
      break;
    }
    const toFetch = [];
    const queued = new Set();
    for (const path of frontier) {
      if (seen.has(path)) continue;
      seen.add(path);
      const src = moduleMap[path];
      if (typeof src !== 'string') continue;
      for (const spec of localImportSpecs(src)) {
        const rel = resolveImportRel(path, spec);
        if (!rel || moduleMapHas(moduleMap, rel) || queued.has(rel)) continue;
        queued.add(rel);
        toFetch.push(rel);
      }
    }
    if (toFetch.length === 0) break;
    const nextFrontier = [];
    await Promise.all(
      toFetch.map(async (rel) => {
        if (moduleMapHas(moduleMap, rel)) return;
        if (moduleCount(moduleMap) >= IMPORT_CLOSURE_MAX_MODULES) return;
        let body = null;
        try {
          body = await fetchSource(rel);
        } catch (err) {
          console.warn('[relay-runtime] import closure fetch error:', rel, err);
          return;
        }
        if (typeof body !== 'string' || !looksLikeHookSource(body)) return;
        putModuleInMap(moduleMap, rel, body);
        if (!moduleMapHas(moduleMap, rel)) return;
        nextFrontier.push(rel);
        if (typeof onLoaded === 'function') onLoaded(rel);
      }),
    );
    frontier = nextFrontier;
  }
  return moduleMap;
}

function moduleCount(moduleMap) {
  return Object.keys(moduleMap).filter((k) => !k.startsWith('/')).length;
}

/** Resolve relative media paths against hook gateway base. */
function resolveMediaUrl(url) {
  const u = String(url ?? '').trim();
  if (!u) return u;
  if (/^https?:\/\//i.test(u) || /^blob:/i.test(u) || /^data:/i.test(u)) return u;
  const base = globalThis.__relay_fetch_base_url;
  if (typeof base === 'string' && base.trim()) {
    try {
      return new URL(u, base.trim().replace(/\/+$/, '') + '/').href;
    } catch {
      return u;
    }
  }
  return u;
}

function propStringFromVnodeProps(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'String') return val.value;
  if (typeof val === 'string') return val;
  return null;
}

function propBoolFromVnodePropsLoose(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'Bool') return !!val.value;
  if (typeof val === 'boolean') return val;
  return null;
}

function propNumberFromVnodeProps(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'Number') return val.value;
  if (typeof val === 'number') return val;
  return null;
}

/** Minimal SRT → WebVTT for `<track src>` (browser requires WebVTT). */
function srtToVtt(srt) {
  let body = String(srt).replace(/\r/g, '').trim();
  if (body.startsWith('WEBVTT')) return body;
  body = body.replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2');
  return 'WEBVTT\n\n' + body;
}

function fetchTextSync(url) {
  if (typeof globalThis.__relay_host_fetch !== 'function') return null;
  try {
    const raw = globalThis.__relay_host_fetch(url, '{}');
    const j = JSON.parse(raw);
    if (j && j.ok && typeof j.text === 'string') return j.text;
  } catch {
    /* ignore */
  }
  return null;
}

function attachMediaHandler(el, eventName, props, key, invokeHandler, onAfterInvoke, payloadFn) {
  const val = props && props[key];
  if (!val || typeof val !== 'object' || val.type !== 'HandlerId' || typeof val.value !== 'number') return;
  const handlerId = val.value;
  el.addEventListener(eventName, () => {
    try {
      if (payloadFn) {
        const payload = payloadFn();
        if (payload !== undefined) {
          push_message_from_host(
            JSON.stringify({ type: 'relay-video', event: key, handlerId, ...payload }),
          );
        }
      }
      invokeHandler(handlerId);
    } catch (err) {
      console.error('[relay-runtime] video handler failed:', err);
    }
    if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
  });
}

/**
 * Build native HTML5 audio shell for `relay-audio-player` VNodes.
 * CleverScript controls: playing, command/commandSeq (1 play, 2 pause, 3 seek).
 */
function buildRelayAudioPlayer(props, invokeHandler, onAfterInvoke) {
  const audio = document.createElement('audio');
  audio.className = 'relay-audio-player';
  audio.setAttribute('preload', 'auto');
  audio.style.display = 'none';

  const src = resolveMediaUrl(propStringFromVnodeProps(props, 'src'));
  if (src) audio.src = src;

  const controls = propBoolFromVnodePropsLoose(props, 'controls');
  if (controls === true) {
    audio.controls = true;
    audio.style.display = 'block';
  }

  const playerId = propStringFromVnodeProps(props, 'playerId') || `rap-${Math.random().toString(36).slice(2)}`;
  audio.dataset.relayPlayerId = playerId;

  attachMediaHandler(audio, 'play', props, 'onPlay', invokeHandler, onAfterInvoke);
  attachMediaHandler(audio, 'pause', props, 'onPause', invokeHandler, onAfterInvoke);
  attachMediaHandler(audio, 'ended', props, 'onEnded', invokeHandler, onAfterInvoke);
  attachMediaHandler(audio, 'error', props, 'onError', invokeHandler, onAfterInvoke);

  const playing = propBoolFromVnodePropsLoose(props, 'playing');
  if (playing === true) {
    audio.play().catch(() => {});
  } else if (playing === false) {
    audio.pause();
  }

  const rate = propNumberFromVnodeProps(props, 'playbackRate');
  if (rate != null && !Number.isNaN(rate)) audio.playbackRate = rate;

  const cmdSeq = propNumberFromVnodeProps(props, 'commandSeq');
  const cmd = propNumberFromVnodeProps(props, 'command');
  const cmdArg = propNumberFromVnodeProps(props, 'commandArg');
  if (cmdSeq != null && String(cmdSeq) !== audio.dataset.relayCommandSeq) {
    audio.dataset.relayCommandSeq = String(cmdSeq);
    if (cmd === 1) audio.play().catch(() => {});
    else if (cmd === 2) audio.pause();
    else if (cmd === 3 && cmdArg != null) audio.currentTime = cmdArg;
    else if (cmd === 4) {
      audio.pause();
      audio.currentTime = 0;
    }
  }

  const className = propStringFromVnodeProps(props, 'className');
  if (className) audio.className = `${audio.className} ${className}`.trim();

  return audio;
}

/**
 * Build native HTML5 video shell for `relay-video-player` VNodes.
 * CleverScript controls: playing, currentTime, command/commandSeq/commandArg props.
 */
function buildRelayVideoPlayer(props, invokeHandler, onAfterInvoke) {
  const shell = document.createElement('div');
  shell.className = 'relay-video-player-shell';
  const video = document.createElement('video');
  video.className = 'relay-video-player';
  video.setAttribute('playsinline', '');
  video.setAttribute('preload', 'metadata');

  const src = resolveMediaUrl(propStringFromVnodeProps(props, 'src'));
  if (src) video.src = src;
  const poster = propStringFromVnodeProps(props, 'poster');
  if (poster) video.poster = resolveMediaUrl(poster);

  const controls = propBoolFromVnodePropsLoose(props, 'controls');
  video.controls = controls !== false;
  const autoPlay = propBoolFromVnodePropsLoose(props, 'autoPlay');
  if (autoPlay === true) video.autoplay = true;
  const muted = propBoolFromVnodePropsLoose(props, 'muted');
  if (muted === true) video.muted = true;
  const loop = propBoolFromVnodePropsLoose(props, 'loop');
  if (loop === true) video.loop = true;

  const vol = propNumberFromVnodeProps(props, 'volume');
  if (vol != null && !Number.isNaN(vol)) video.volume = Math.min(1, Math.max(0, vol));
  const rate = propNumberFromVnodeProps(props, 'playbackRate');
  if (rate != null && !Number.isNaN(rate)) video.playbackRate = rate;

  const playerId = propStringFromVnodeProps(props, 'playerId') || `rvp-${Math.random().toString(36).slice(2)}`;
  video.dataset.relayPlayerId = playerId;

  const subtitlesJson = propStringFromVnodeProps(props, 'subtitlesJson');
  if (subtitlesJson) {
    try {
      const tracks = JSON.parse(subtitlesJson);
      if (Array.isArray(tracks)) {
        tracks.forEach((t, i) => {
          if (!t || !t.src) return;
          const track = document.createElement('track');
          track.kind = 'subtitles';
          const trackUrl = resolveMediaUrl(String(t.src));
          let srcForTrack = trackUrl;
          if (/\.srt$/i.test(trackUrl)) {
            const srt = fetchTextSync(trackUrl);
            if (srt) {
              srcForTrack = URL.createObjectURL(new Blob([srtToVtt(srt)], { type: 'text/vtt' }));
            }
          }
          track.src = srcForTrack;
          if (t.label) track.label = String(t.label);
          if (t.lang) track.srclang = String(t.lang);
          if (t.default) track.default = true;
          video.appendChild(track);
        });
      }
    } catch (e) {
      console.warn('[relay-runtime] subtitlesJson parse failed', e);
    }
  }

  attachMediaHandler(video, 'play', props, 'onPlay', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'pause', props, 'onPause', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'ended', props, 'onEnded', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'error', props, 'onError', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'loadedmetadata', props, 'onLoadedMetadata', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'seeked', props, 'onSeeked', invokeHandler, onAfterInvoke);

  let lastTimeEvent = 0;
  video.addEventListener('timeupdate', () => {
    const now = Date.now();
    if (now - lastTimeEvent < 250) return;
    lastTimeEvent = now;
    const val = props && props.onTimeUpdate;
    if (!val || val.type !== 'HandlerId') return;
    try {
      push_message_from_host(
        JSON.stringify({
          type: 'relay-video',
          event: 'onTimeUpdate',
          handlerId: val.value,
          playerId,
          currentTime: video.currentTime,
          duration: video.duration,
        }),
      );
      invokeHandler(val.value);
    } catch (err) {
      console.error('[relay-runtime] onTimeUpdate failed', err);
    }
    if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
  });

  const playing = propBoolFromVnodePropsLoose(props, 'playing');
  if (playing === true) {
    video.play().catch(() => {});
  } else if (playing === false) {
    video.pause();
  }

  const ct = propNumberFromVnodeProps(props, 'currentTime');
  if (ct != null && !Number.isNaN(ct) && Math.abs(video.currentTime - ct) > 0.25) {
    video.currentTime = ct;
  }

  const cmdSeq = propNumberFromVnodeProps(props, 'commandSeq');
  const cmd = propNumberFromVnodeProps(props, 'command');
  const cmdArg = propNumberFromVnodeProps(props, 'commandArg');
  if (cmdSeq != null && String(cmdSeq) !== video.dataset.relayCommandSeq) {
    video.dataset.relayCommandSeq = String(cmdSeq);
    if (cmd === 1) video.play().catch(() => {});
    else if (cmd === 2) video.pause();
    else if (cmd === 3 && cmdArg != null) video.currentTime = cmdArg;
  }

  shell.appendChild(video);
  return shell;
}

/** VNode PropValue.Bool from relay-runtime serde: `{ type: 'Bool', value: true }`. */
export function propBoolFromVnodeProps(props, key) {
  const val = props && props[key];
  return !!(val && typeof val === 'object' && val.type === 'Bool' && val.value === true);
}

/**
 * When relayControlled anchor: only intercept ordinary left-clicks (SPA); let modified / special targets behave natively.
 * Absolute http(s) and pre-live SSR (data-pt-hydrated≠1) keep native navigation so links work during boot.
 */
export function shouldInterceptRelayControlledAnchorClick(e, el) {
  if (e.defaultPrevented || e.button !== 0) return false;
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return false;
  try {
    const doc = el && el.ownerDocument ? el.ownerDocument : document;
    if (doc?.documentElement?.getAttribute?.('data-pt-hydrated') !== '1') return false;
  } catch (_) {
    /* optional */
  }
  const t = el.getAttribute('target');
  if (t && t !== '_self') return false;
  const href = el.getAttribute('href');
  if (!href) return true;
  if (/^(mailto:|tel:|#)/i.test(href)) return false;
  if (/^(https?:|\/\/)/i.test(href)) return false;
  return true;
}

/**
 * Attach event handlers from VNode props onto an existing DOM element (hydration).
 */
function attachHandlersFromProps(el, tag, props, invokeHandler, onAfterInvoke) {
  if (!props || !el || typeof el.addEventListener !== 'function') return;
  const relayControlled = propBoolFromVnodeProps(props, 'relayControlled');
  for (const [k, val] of Object.entries(props)) {
    if (
      val &&
      typeof val === 'object' &&
      (val.type === 'Handler' || val.type === 'HandlerId') &&
      typeof val.value === 'number'
    ) {
      const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
      if (tag === 'a' && eventType === 'click' && !relayControlled) {
        continue;
      }
      const handlerId = val.value;
      el.addEventListener(eventType, (e) => {
        if (tag === 'a' && eventType === 'click' && relayControlled) {
          if (!shouldInterceptRelayControlledAnchorClick(e, el)) return;
          e.preventDefault();
        }
        try {
          invokeHandler(handlerId);
        } catch (err) {
          console.error('[relay-runtime] Handler invocation failed:', err);
          throw err;
        }
        if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
      });
    }
  }
}

function elementChildNodes(parent) {
  const out = [];
  if (!parent || !parent.childNodes) return out;
  for (const n of parent.childNodes) {
    if (n.nodeType === 1) out.push(n); // Element
  }
  return out;
}

function expectedDomTag(tag) {
  if (tag === 'relay-video-player' || tag === 'relay-audio-player' || tag === 'relay-hook-renderer') return 'DIV';
  if (tag === 'fragment') return null;
  return String(tag || 'div').toUpperCase();
}

/**
 * Best-effort hydrate: attach handlers on matching subtrees; leave mismatched
 * SSR subtrees intact. Parked tips must not fail the whole tree on article skew.
 *
 * @param {object} v - VNode JSON
 * @param {HTMLElement} container - Element whose children are the SSR markup (e.g. #root)
 * @param {function(number): void} [invokeHandler]
 * @param {function(): void} [onAfterInvoke]
 * @returns {boolean}
 */
export function hydrateVnode(v, container, invokeHandler = invoke_handler, onAfterInvoke = null) {
  if (!v || !container) return false;
  try {
    if (v.Empty !== undefined) return true;
    if (v.Text !== undefined) return true;
    if (v.Element) {
      const { tag, children } = v.Element;
      if (tag === 'fragment') {
        return hydrateChildren(children || [], elementChildNodes(container), invokeHandler, onAfterInvoke, '/fragment');
      }
      const kids = elementChildNodes(container);
      if (kids.length === 1) {
        return hydrateElementNode(v, kids[0], invokeHandler, onAfterInvoke, `/${tag || 'root'}`, true);
      }
      if (kids.length === 0 && (!children || children.length === 0)) return true;
      if (kids.length > 0 && children && children.length) {
        console.warn(
          `[relay-runtime] hydrate root child-count soft mismatch: vnode children vs dom=${kids.length} — best-effort`
        );
        return hydrateChildren(children, kids, invokeHandler, onAfterInvoke, `/${tag || 'root'}`);
      }
      console.warn(
        `[relay-runtime] hydrate root child-count mismatch: vnode children vs dom=${kids.length} — keeping SSR`
      );
      return true;
    }
    if (Array.isArray(v)) {
      return hydrateChildren(v, elementChildNodes(container), invokeHandler, onAfterInvoke, '/[]');
    }
    return false;
  } catch (e) {
    console.warn('[relay-runtime] hydrateVnode failed:', e);
    return false;
  }
}

function hydrateChildren(vnodes, domEls, invokeHandler, onAfterInvoke, path) {
  const basePath = path || '';
  const flat = [];
  for (const c of vnodes || []) {
    if (!c || c.Empty !== undefined || c.Text !== undefined) continue;
    if (c.Element && c.Element.tag === 'fragment') {
      for (const nested of c.Element.children || []) {
        if (nested && nested.Element) flat.push(nested);
      }
    } else if (c.Element) {
      flat.push(c);
    }
  }
  if (flat.length === 0 && domEls.length === 0) return true;
  if (flat.length !== domEls.length) {
    const vnodeTags = flat.map((v) => (v.Element && v.Element.tag) || '?').join(',');
    const domTags = domEls.map((el) => el.tagName || '?').join(',');
    console.warn(
      `[relay-runtime] hydrate child-count mismatch at ${basePath || '/'}: vnode=${flat.length}[${vnodeTags}] dom=${domEls.length}[${domTags}] — soft hydrate`
    );
  }
  const n = Math.min(flat.length, domEls.length);
  for (let i = 0; i < n; i++) {
    const expect = expectedDomTag(flat[i].Element && flat[i].Element.tag);
    const got = String(domEls[i].tagName || '').toUpperCase();
    if (expect && got && expect !== got && !(expect === 'DIV' && got === 'DIV')) {
      console.warn(
        `[relay-runtime] hydrate skip tag mismatch at ${basePath}/${expect}[${i}]: vnode=${expect} dom=${got}`
      );
      continue;
    }
    const tag = (flat[i].Element && flat[i].Element.tag) || i;
    hydrateElementNode(flat[i], domEls[i], invokeHandler, onAfterInvoke, `${basePath}/${tag}[${i}]`, true);
  }
  // Soft success: keep SSR even when counts diverge (absolute anchors stay clickable).
  return true;
}

/**
 * @param {boolean} [soft] when true, tag mismatches do not fail the parent walk
 */
function hydrateElementNode(v, el, invokeHandler, onAfterInvoke, path, soft = false) {
  if (!v || !v.Element || !el) return soft;
  const { tag, props, children } = v.Element;
  const here = path || tag || '?';
  if (tag === 'fragment') {
    return hydrateChildren(children || [], elementChildNodes(el), invokeHandler, onAfterInvoke, here);
  }
  const expect = expectedDomTag(tag);
  if (expect && String(el.tagName || '').toUpperCase() !== expect) {
    if (!(expect === 'DIV' && String(el.tagName || '').toUpperCase() === 'DIV')) {
      console.warn(
        `[relay-runtime] hydrate tag mismatch at ${here}: vnode=${expect} dom=${el.tagName}`
      );
      return soft;
    }
  }
  attachHandlersFromProps(el, tag, props || {}, invokeHandler, onAfterInvoke);
  return hydrateChildren(children || [], elementChildNodes(el), invokeHandler, onAfterInvoke, here);
}

/**
 * Promote parked SSR shell without wiping `#root`.
 * @param {HTMLElement} container
 * @param {{ hydrateOk?: boolean }} [opts]
 */
export function promoteSsrShellWithoutWipe(container, opts = {}) {
  if (!container) return;
  try {
    container.dataset.relayHydrated = '1';
  } catch (_) { /* optional */ }
  try {
    if (typeof set_client_live_phase === 'function') {
      set_client_live_phase(1);
    } else if (typeof set_client_live === 'function') {
      set_client_live(true);
    }
  } catch (_) { /* optional */ }
  syncHtmlLivePhaseAttr(1, container);
  try {
    const nodes = container.querySelectorAll
      ? container.querySelectorAll('.pt-client-only')
      : [];
    for (let i = 0; i < nodes.length; i++) {
      const el = nodes[i];
      try {
        if (el.disabled) el.disabled = false;
      } catch (_) { /* optional */ }
      try {
        if (el.getAttribute('aria-disabled') === 'true') {
          el.setAttribute('aria-disabled', 'false');
        }
      } catch (_) { /* optional */ }
    }
  } catch (_) { /* optional */ }
  installRumBeacon(container, { hydrateOk: opts.hydrateOk !== false });
}

const SSR_PLACEHOLDER_SELECTORS =
  '[data-pt-dirnav-ssr],[data-pt-changelog-ssr],[data-pt-autoindex-ssr],[data-pt-deferred-ssr]';

function cssEscapeIdent(s) {
  try {
    if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(s);
  } catch (_) { /* optional */ }
  return String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

/**
 * Replace only marked SSR placeholders (and known catalog islands) from a new
 * vnode tree — never root-wipe. Safe after soft-hydrate when DOM ≠ prevVnode.
 * @returns {boolean} true if at least one node was replaced
 */
export function replaceSsrPlaceholdersFromVnode(container, vnode, invokeHandler, onAfterInvoke) {
  if (!container || !vnode) return false;
  const searchRoot = vnodeToDom(vnode, invokeHandler, onAfterInvoke);
  if (!searchRoot || typeof searchRoot.querySelectorAll !== 'function') return false;

  const targets = [];
  const seen = new Set();
  const add = (el) => {
    if (!el || seen.has(el)) return;
    seen.add(el);
    targets.push(el);
  };
  container.querySelectorAll(SSR_PLACEHOLDER_SELECTORS).forEach(add);
  // Re-upgrade already-filled islands on later phases (markers removed).
  ;['Directory', 'Change log', 'Auto index', 'Deferred section'].forEach((label) => {
    container.querySelectorAll(`[aria-label="${cssEscapeIdent(label)}"]`).forEach(add);
  });
  if (!targets.length) return false;

  let replaced = 0;
  for (let i = 0; i < targets.length; i++) {
    const oldEl = targets[i];
    if (!oldEl.isConnected) continue;
    let next = null;
    const label = oldEl.getAttribute('aria-label');
    if (label) {
      next = searchRoot.querySelector(`[aria-label="${cssEscapeIdent(label)}"]`);
    }
    if (!next) {
      if (oldEl.classList?.contains('pt-dir-nav') || oldEl.hasAttribute('data-pt-dirnav-ssr')) {
        next = searchRoot.querySelector('.pt-dir-nav');
      } else if (oldEl.classList?.contains('pt-changelog') || oldEl.hasAttribute('data-pt-changelog-ssr')) {
        next = searchRoot.querySelector('.pt-changelog');
      } else if (oldEl.classList?.contains('pt-autoindex') || oldEl.hasAttribute('data-pt-autoindex-ssr')) {
        next = searchRoot.querySelector('.pt-autoindex');
      } else if (oldEl.classList?.contains('pt-deferred-md') || oldEl.hasAttribute('data-pt-deferred-ssr')) {
        next = searchRoot.querySelector('section.pt-deferred-md') || searchRoot.querySelector('[data-pt-deferred],[data-pt-deferred-pending],[data-pt-deferred-ssr]');
      }
    }
    if (!next || !oldEl.parentNode) continue;
    // Don't steal a node already moved into the live tree.
    if (container.contains(next)) continue;
    try {
      oldEl.parentNode.replaceChild(next, oldEl);
      replaced += 1;
    } catch (_) { /* optional */ }
  }
  return replaced > 0;
}

/**
 * Convert VNode JSON to DOM nodes.
 * Handles Element (VNode format), type: "element" (Value format), Text, and arrays.
 *
 * @param {object} v - VNode or Value
 * @param {function(number): void} [invokeHandler] - Called for Handler props (onClick, etc.). Default: invoke_handler from WASM.
 * @param {function(): void} [onAfterInvoke] - Called after each handler; if consume_render_requested(), should re-run and re-apply.
 * @returns {DocumentFragment|HTMLElement|Text|null}
 */
/**
 * Coarse VNode patches (mirrors `vdom::reconcile::patches`). Prefer replacing the smallest
 * divergent subtree; prop-only changes replace that element.
 * @returns {{ path: number[], tree: object }[]}
 */
export function computeVnodePatches(oldV, newV) {
  const acc = [];
  patchesAt(oldV, newV, [], acc);
  return acc;
}

function vnodeDeepEqual(a, b) {
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}

function patchesAt(oldV, newV, path, acc) {
  if (vnodeDeepEqual(oldV, newV)) return;
  const oEl = oldV && oldV.Element;
  const nEl = newV && newV.Element;
  if (oEl && nEl) {
    const sameTag = oEl.tag === nEl.tag && (oEl.key ?? null) === (nEl.key ?? null);
    const oKids = oEl.children || [];
    const nKids = nEl.children || [];
    if (sameTag && oKids.length === nKids.length) {
      if (!vnodeDeepEqual(oEl.props || {}, nEl.props || {})) {
        acc.push({ path: path.slice(), tree: newV });
        return;
      }
      for (let i = 0; i < oKids.length; i++) {
        patchesAt(oKids[i], nKids[i], path.concat(i), acc);
      }
      return;
    }
  }
  const oText = oldV && oldV.Text !== undefined;
  const nText = newV && newV.Text !== undefined;
  if (oText && nText && oldV.Text === newV.Text) return;
  acc.push({ path: path.slice(), tree: newV });
}

function vnodeProducesDom(v) {
  if (!v || v.Empty !== undefined) return false;
  return true;
}

function patchableChildNodes(parent) {
  const out = [];
  if (!parent || !parent.childNodes) return out;
  for (const n of parent.childNodes) {
    if (n.nodeType === 1 || n.nodeType === 3) out.push(n);
  }
  return out;
}

function childrenForPatchPath(v) {
  if (v && v.Element) return v.Element.children || [];
  if (Array.isArray(v)) return v;
  return [];
}

/**
 * Apply coarse ReplaceSubtree patches under `container` (hydrate/CSR root).
 * @returns {boolean} false → caller should fall back to full wipe
 */
export function applyVnodePatches(container, prevRoot, patches, invokeHandler, onAfterInvoke) {
  if (!container || !patches || !patches.length) return true;
  for (const p of patches) {
    const path = p.path || [];
    const tree = p.tree;
    if (path.length === 0) {
      // Root replace — not cheaper than wipe; signal fallback.
      return false;
    }
    let parentDom = container;
    let vnode = prevRoot;
    let targetNode = null;
    for (let d = 0; d < path.length; d++) {
      const idx = path[d];
      const kids = childrenForPatchPath(vnode);
      const domKids = patchableChildNodes(parentDom);
      let di = 0;
      let found = null;
      let foundV = null;
      for (let i = 0; i < kids.length; i++) {
        if (!vnodeProducesDom(kids[i])) continue;
        if (i === idx) {
          found = domKids[di];
          foundV = kids[i];
          break;
        }
        di += 1;
      }
      if (!found) return false;
      if (d === path.length - 1) {
        targetNode = found;
        break;
      }
      parentDom = found;
      vnode = foundV;
    }
    if (!targetNode || !parentDom) return false;
    const next = vnodeToDom(tree, invokeHandler, onAfterInvoke);
    if (!next) {
      parentDom.removeChild(targetNode);
      continue;
    }
    parentDom.replaceChild(next, targetNode);
  }
  return true;
}

export function vnodeToDom(v, invokeHandler = invoke_handler, onAfterInvoke = null) {
  if (!v) return null;
  if (v.Empty !== undefined) return null; // VNode::Empty
  if (v.Text !== undefined) return document.createTextNode(v.Text);
  if (v.Element) {
    const { tag, props, children } = v.Element;
    if (tag === 'relay-hook-renderer') {
      const host = (props?.host?.value ?? props?.host) ?? '';
      const hookPath = (props?.hookPath?.value ?? props?.hookPath) ?? 'hooks/app.jsx';
      const baseUrl = (props?.baseUrl?.value ?? props?.baseUrl) ?? undefined;
      const container = document.createElement('div');
      container.className = 'relay-hook-renderer';
      renderHook(container, { hookPath, host: host || undefined, baseUrl }).catch((e) => {
        renderHookErrorUI(container, e, {
          keepContent: container.dataset?.relaySsr === '1',
        });
      });
      return container;
    }
    if (tag === 'relay-video-player') {
      return buildRelayVideoPlayer(props || {}, invokeHandler, onAfterInvoke);
    }
    if (tag === 'relay-audio-player') {
      return buildRelayAudioPlayer(props || {}, invokeHandler, onAfterInvoke);
    }
    if (tag === 'fragment') {
      const frag = document.createDocumentFragment();
      for (const c of children || []) {
        const child = vnodeToDom(c, invokeHandler, onAfterInvoke);
        if (child) frag.appendChild(child);
      }
      return frag;
    }
    const el = createVnodeDomElement(tag);
    applyVnodeDomProps(el, tag, props, invokeHandler, onAfterInvoke);
    appendVnodeDomChildren(el, props, children, invokeHandler, onAfterInvoke);
    return el;
  }
  if (v.type === 'element') {
    const rawTag = v.tag === 'fragment' ? 'div' : v.tag;
    if (rawTag === 'relay-video-player') {
      return buildRelayVideoPlayer(v.props || {}, invokeHandler, onAfterInvoke);
    }
    if (rawTag === 'relay-audio-player') {
      return buildRelayAudioPlayer(v.props || {}, invokeHandler, onAfterInvoke);
    }
    const el = createVnodeDomElement(rawTag);
    applyVnodeDomProps(el, rawTag, v.props, invokeHandler, onAfterInvoke);
    appendVnodeDomChildren(el, v.props, v.children, invokeHandler, onAfterInvoke);
    return el;
  }
  if (Array.isArray(v)) {
    const frag = document.createDocumentFragment();
    for (const c of v) {
      const child = vnodeToDom(c, invokeHandler, onAfterInvoke);
      if (child) frag.appendChild(child);
    }
    return frag;
  }
  return null;
}

/**
 * Build a readable Error from a failed hook fetch. The gateway 500 body
 * (get_hook_execution_error_response) carries `summaryForReaders`, `technicalReport`,
 * repo/branch, and structured `cleverScript` diagnostics — surface them instead of
 * just "failed: 500". The parsed body is kept on `err.gateway` for advanced UI.
 */
async function hookFetchError(resp, url) {
  const err = new Error(`Fetch entry ${url} failed: ${resp.status}`);
  err.status = resp.status;
  err.url = url;
  let text = '';
  try {
    text = await resp.text();
  } catch {
    return err;
  }
  let detail = null;
  try {
    detail = JSON.parse(text);
  } catch {
    /* not JSON */
  }
  if (!detail || typeof detail !== 'object') {
    if (text.trim()) err.message += `\n${text.trim().slice(0, 500)}`;
    return err;
  }
  err.gateway = detail;
  const parts = [err.message];
  if (detail.repo || detail.branch) {
    parts.push(`repo: ${detail.repo ?? '?'}  branch: ${detail.branch ?? '?'}`);
  }
  const cs = detail.cleverScript;
  if (cs && typeof cs === 'object' && cs.message) {
    const loc = [cs.file, cs.line, cs.column].filter((v) => v != null && v !== '').join(':');
    parts.push(`CleverScript ${cs.type || 'error'}${loc ? ` at ${loc}` : ''}: ${cs.message}`);
    if (cs.source_line) {
      parts.push(`  ${cs.source_line}`);
      if (cs.column) parts.push(`  ${' '.repeat(Math.max(0, cs.column - 1))}^`);
    }
    if (Array.isArray(cs.call_stack) && cs.call_stack.length) {
      parts.push(`  call stack: ${cs.call_stack.join(' > ')}`);
    }
  } else if (typeof detail.technicalReport === 'string' && detail.technicalReport) {
    parts.push(detail.technicalReport.slice(0, 1000));
  }
  if (typeof detail.summaryForReaders === 'string' && detail.summaryForReaders) {
    parts.push(detail.summaryForReaders);
  }
  err.message = parts.join('\n');
  return err;
}

/**
 * Render a structured hook error panel into (or on top of) `container`.
 * When `keepContent` is true (e.g. SSR markup is still valid), the panel is prepended
 * so readers keep the static page; otherwise the container is replaced.
 * Shows: title, message (pre-wrap), fetch URL + HTTP status, repo/branch, and
 * CleverScript diagnostics (type, file:line:column, source line, call stack) when the
 * gateway 500 JSON was attached to the error (see hookFetchError).
 */
/**
 * WASM parse/runtime errors arrive as JSON strings `{message, line, column, file?, source_line?}`
 * (see run_hook_with_file docs). Format them as `file:line:column: message` with a caret line.
 */
function formatRuntimeErrorMessage(msg) {
  const s = String(msg ?? '');
  const start = s.indexOf('{');
  if (start === -1) return s;
  try {
    const parsed = JSON.parse(s.slice(start));
    if (!parsed || typeof parsed.message !== 'string') return s;
    const loc = [parsed.file, parsed.line, parsed.column].filter((v) => v != null && v !== '').join(':');
    let out = (s.slice(0, start) + (loc ? `${loc}: ` : '') + parsed.message).trim();
    if (parsed.source_line) {
      out += `\n  ${parsed.source_line}\n  ${' '.repeat(Math.max(0, (parsed.column || 1) - 1))}^`;
    }
    return out;
  } catch {
    return s;
  }
}

/** @typedef {'idle'|'wasm'|'manifest'|'modules'|'theme'|'render'|'live'|'error'} BootPhase */

let _currentBootStatus = { phase: 'idle', message: '', detail: '', progress: '' };

/**
 * Publish boot phase to mount dataset + CustomEvent for hook RelayBootShell.
 * @param {HTMLElement|null|undefined} container
 * @param {{ phase?: BootPhase, message?: string, detail?: string, progress?: string }} status
 */
export function setBootStatus(container, { phase = 'idle', message = '', detail = '', progress = '' } = {}) {
  _currentBootStatus = { phase, message, detail, progress };
  if (container && container.dataset) {
    container.dataset.relayBootPhase = phase;
    container.dataset.relayBootMessage = message;
    if (detail) container.dataset.relayBootDetail = detail;
    else delete container.dataset.relayBootDetail;
    if (progress) container.dataset.relayBootProgress = progress;
    else delete container.dataset.relayBootProgress;
  }
  try {
    if (container && typeof CustomEvent !== 'undefined') {
      container.dispatchEvent(
        new CustomEvent('relay-boot-status', { detail: { ..._currentBootStatus }, bubbles: true }),
      );
    }
  } catch (_) {
    /* optional */
  }
}

/**
 * Read last boot status from container dataset or module-global fallback.
 * @param {HTMLElement|null|undefined} container
 */
export function getBootStatus(container) {
  if (container && container.dataset && container.dataset.relayBootPhase) {
    return {
      phase: container.dataset.relayBootPhase,
      message: container.dataset.relayBootMessage || '',
      detail: container.dataset.relayBootDetail || '',
      progress: container.dataset.relayBootProgress || '',
    };
  }
  return { ..._currentBootStatus };
}

/** Inject host mode + boot fields into hooks manifest for CleverScript `@hooks`. */
function injectHostManifestFields(hooksManifest, container) {
  if (!hooksManifest || typeof hooksManifest !== 'object') return;
  const hasSsrMarkup =
    container &&
    container.dataset &&
    container.dataset.relaySsr === '1' &&
    container.childElementCount > 0;
  let ssrArticle = false;
  try {
    ssrArticle = Boolean(hasSsrMarkup && container.querySelector('[data-pt-article]'));
  } catch (_) {
    ssrArticle = false;
  }
  hooksManifest.relayHostMode = hasSsrMarkup ? 'ssr' : 'csr';
  // Parked hydrate: ssr → RelayBootShell returns children (P0 HTML). Empty CSR
  // mounts: csr → full overlay until isClientLive(). Gateway SSR never runs this
  // inject; BootShell/hostMode default missing relayHostMode to ssr.
  hooksManifest.relaySsrArticlePresent = ssrArticle;
  const boot = getBootStatus(container);
  hooksManifest.relayBootPhase = boot.phase;
  hooksManifest.relayBootMessage = boot.message;
  hooksManifest.relayBootDetail = boot.detail;
  hooksManifest.relayBootProgress = boot.progress;
}

export function renderHookErrorUI(container, err, { keepContent = false, title = 'Hook error' } = {}) {
  if (!container) return;
  const panel = document.createElement('div');
  panel.className = 'relay-hook-error';
  panel.style.cssText =
    'font: 13px/1.5 ui-monospace, monospace; color: #ffb4b4; background: #2a0f0f;' +
    'border: 1px solid #7f1d1d; border-radius: 6px; padding: 10px 12px; margin: 8px;' +
    'white-space: pre-wrap; overflow-wrap: anywhere; text-align: left;';

  const head = document.createElement('div');
  head.style.cssText = 'font-weight: bold; color: #fecaca; margin-bottom: 4px;';
  head.textContent = title;
  panel.appendChild(head);

  const msg = document.createElement('div');
  msg.textContent = formatRuntimeErrorMessage(err?.message ?? err ?? 'Unknown error');
  panel.appendChild(msg);

  const meta = [];
  if (err?.status) meta.push(`HTTP ${err.status}`);
  const gw = err?.gateway;
  if (gw?.repo) meta.push(`repo ${gw.repo}`);
  if (gw?.branch) meta.push(`branch ${gw.branch}`);
  if (typeof location !== 'undefined') meta.push(`page ${location.pathname}${location.search}`);
  meta.push(`at ${new Date().toISOString()}`);
  if (meta.length) {
    const info = document.createElement('div');
    info.style.cssText = 'margin-top: 6px; color: #f87171; opacity: 0.8; font-size: 12px;';
    info.textContent = meta.join(' · ');
    panel.appendChild(info);
  }

  const hint = document.createElement('div');
  hint.style.cssText = 'margin-top: 6px; color: #fca5a5; opacity: 0.8; font-size: 12px;';
  hint.textContent = keepContent
    ? 'Static server-rendered content is shown below; interactive features are unavailable.'
    : 'Details are also logged to the browser console.';
  panel.appendChild(hint);

  if (keepContent) {
    // Errors can surface twice (renderHook catch + caller catch) — keep one panel.
    const existing = container.querySelector(':scope > .relay-hook-error');
    if (existing) existing.replaceWith(panel);
    else container.prepend(panel);
  } else {
    container.innerHTML = '';
    container.className = (container.className || '') + ' error';
    container.appendChild(panel);
  }
}

/**
 * HookRenderer: run a hook and render into container.
 * Local: hookPath + baseUrl. Remote: host + hookPath (fetches from host).
 *
 * @param {HTMLElement} container - DOM element to render into
 * @param {object} options
 * @param {string} options.hookPath - Path (e.g. 'hooks/app.jsx')
 * @param {string} [options.host] - For remote: base URL to fetch from
 * @param {string} [options.baseUrl] - For local: base URL for resolving paths
 * @returns {Promise<void>}
 */
export async function renderHook(container, options = {}) {
  const {
    hookPath,
    host,
    baseUrl = getDefaultBaseUrl(),
    hydrate = false,
    mode = 'auto',
    branch = '',
    initialPath = '',
    themeYaml = '',
  } = options;
  const wantHydrate =
    mode === 'hydrate' ? true : mode === 'csr' ? false : hydrate || container?.dataset?.relaySsr === '1';

  let moduleMap = {};
  let source = '';
  let effectivePath = hookPath || 'hooks/app.jsx';
  let hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
  let hydratedOnce = false;
  let livePromoted = false;
  let liveUpgradeScheduled = false;
  let prevVnode = null;
  let themeYamlText = typeof themeYaml === 'string' ? themeYaml : '';
  const branchNorm = typeof branch === 'string' ? branch.trim() : '';
  const initialPathNorm = typeof initialPath === 'string' ? initialPath.trim() : '';

  /** Enable SSR-disabled chrome without waiting for a full live rebuild. */
  function enableClientOnlyChrome(root) {
    try {
      const scope = root && root.querySelectorAll ? root : document;
      const nodes = scope.querySelectorAll('.pt-client-only');
      for (let i = 0; i < nodes.length; i++) {
        const el = nodes[i];
        try {
          if (el.disabled) el.disabled = false;
        } catch (_) { /* optional */ }
        try {
          if (el.getAttribute('aria-disabled') === 'true') {
            el.setAttribute('aria-disabled', 'false');
          }
        } catch (_) { /* optional */ }
        try {
          if (el.hasAttribute('title') && /after page loads/i.test(el.getAttribute('title') || '')) {
            el.removeAttribute('title');
          }
        } catch (_) { /* optional */ }
      }
    } catch (_) {
      /* optional */
    }
  }

  // Assigned below. Must be `let` (not a later `const`) so esbuild keeps the same
  // binding inside scheduleDeferredLiveUpgrade — a post-declared `const doRender`
  // was renamed to `doRender2` while idle callbacks still called bare `doRender()`.
  let renderHookPass = () => {};

  /**
   * Incremental live ladder (uniform with Android JNI nativeSetClientLivePhase):
   *   1 shell → 2 catalog → 3 feeds → 4 interactive
   * Each step is one idle renderHookPass (applyPatches when possible) so mobile/web
   * never pay the full P2+P3 tree on the first frame after hydrate.
   */
  function scheduleDeferredLiveUpgrade() {
    if (liveUpgradeScheduled) return;
    liveUpgradeScheduled = true;
    const phases = [2, 3, 4];
    let idx = 0;
    const runNext = () => {
      if (idx >= phases.length) return;
      const phase = phases[idx++];
      try {
        // Skip if host/WASM already at or past this phase (monotonic ladder).
        if (typeof get_client_live_phase === 'function') {
          const cur = Number(get_client_live_phase()) || 0;
          if (cur >= phase) {
            syncHtmlLivePhaseAttr(cur, container);
            if (idx < phases.length) scheduleIdle(runNext, 80);
            return;
          }
        }
        if (typeof set_client_live_phase === 'function') {
          set_client_live_phase(phase);
        } else if (typeof set_client_live === 'function') {
          set_client_live(true);
        }
      } catch (_) {
        /* optional until wasm rebuild */
      }
      syncHtmlLivePhaseAttr(phase, container);
      try {
        renderHookPass();
      } catch (e) {
        console.error('[relay-runtime] live phase', phase, 'failed:', e);
      }
      if (idx < phases.length) {
        scheduleIdle(runNext, 120 + idx * 80);
      }
    };
    // Two rAFs then idle: paint SSR chrome + flush input before catalog work.
    try {
      if (typeof requestAnimationFrame === 'function') {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => scheduleIdle(runNext, 200));
        });
        return;
      }
    } catch (_) {
      /* optional */
    }
    scheduleIdle(runNext, 200);
  }

  function scheduleIdle(fn, timeoutMs) {
    try {
      if (typeof requestIdleCallback === 'function') {
        requestIdleCallback(
          () => {
            try {
              fn();
            } catch (e) {
              console.error('[relay-runtime] idle task failed:', e);
            }
          },
          { timeout: timeoutMs || 2500 },
        );
        return;
      }
    } catch (_) {
      /* optional */
    }
    setTimeout(fn, Math.min(timeoutMs || 120, 500));
  }

  try {
    setBootStatus(container, { phase: 'wasm', message: 'Preparing runtime…' });
    try {
      if (typeof set_client_live === 'function') set_client_live(false);
    } catch (_) {
      /* optional until wasm rebuild */
    }
    syncHtmlHydratedAttr(false, container);

    if (host) {
      setBootStatus(container, { phase: 'manifest', message: 'Discovering hooks…' });
      const base = host.trim().replace(/\/+$/, '');
    const baseDir = `${base}/`;
    const cacheBust = '?t=' + Date.now();
    const branchHeaders = branchNorm ? { 'X-Relay-Branch': branchNorm } : {};

    let hookHostSameOrigin = true;
    if (typeof location !== 'undefined' && location.origin) {
      try {
        hookHostSameOrigin = new URL(baseDir).origin === location.origin;
      } catch {
        hookHostSameOrigin = true;
      }
    }

    // Prefer explicit hookPath (e.g. #relay-embed-config) over OPTIONS discovery so
    // SSR hydrate and CSR shell can target different entry hooks on the same origin.
    const explicitHookPath = typeof hookPath === 'string' && hookPath.trim().length > 0;
    let entryPath = effectivePath;
    try {
      // Cross-origin OPTIONS preflights; static mirrors often lack
      // Access-Control-Allow-Methods: OPTIONS. Skip when origins differ.
      if (!explicitHookPath && hookHostSameOrigin) {
        const optsResp = await fetch(baseDir, { method: 'OPTIONS', headers: branchHeaders });
        if (optsResp.ok) {
          let p = optsResp.headers.get('x-hook-path') || optsResp.headers.get('x-relay-hook-path');
          if (!p && optsResp.headers.get('content-type')?.includes('application/json')) {
            const j = await optsResp.json();
            p = j?.hookPath ?? j?.hook_path ?? j?.path;
          }
          if (p) entryPath = p;
        }
      }
    } catch {
      /* discovery optional */
    }

    hooksManifest = { hooks: [] };
    // Same-origin: safe to probe OPTIONS above. Cross-origin: still GET theme + hooks manifest
    // when the static host sends Access-Control-Allow-Origin (e.g. `serve --cors`).
    const hooksUrl = resolveHookUrl(baseDir, 'hooks.json') + cacheBust;
    const hooksResp = await fetch(hooksUrl, { headers: branchHeaders }).catch(() => null);
    if (hooksResp && hooksResp.ok) {
      try {
        hooksManifest = await hooksResp.json();
      } catch {
        hooksManifest = { hooks: [] };
      }
    }

    moduleMap = {};
    const keyFromPath = (p) => {
      const s = (p || '').trim();
      if (s.startsWith('/')) return s.slice(1);
      return s.replace(/^\.\//, '');
    };
    const putModule = (pathKey, text) => {
      putModuleInMap(moduleMap, pathKey, text);
    };
    // Parallel prefetch (Android host already does this) — sequential await was ~N RTTs.
    const hookEntries = (hooksManifest.hooks || []).filter((h) => h && h.path);
    const hookTotal = hookEntries.length || 1;
    let hookDone = 0;
    setBootStatus(container, {
      phase: 'modules',
      message: `Fetching hooks 0/${hookTotal}…`,
      progress: `0/${hookTotal}`,
    });
    await Promise.all(
      hookEntries.map(async (h) => {
        const u = resolveHookUrl(baseDir, h.path) + cacheBust;
        try {
          const resp = await fetch(u, { headers: branchHeaders });
          if (resp.ok) {
            putModule(h.path, await resp.text());
          } else {
            console.warn('[relay-runtime] remote hook fetch failed:', h.path, resp.status);
          }
        } catch (err) {
          console.warn('[relay-runtime] remote hook fetch error:', h.path, err);
        } finally {
          hookDone += 1;
          setBootStatus(container, {
            phase: 'modules',
            message: `Fetching hooks ${hookDone}/${hookTotal}…`,
            progress: `${hookDone}/${hookTotal}`,
          });
        }
      }),
    );

    effectivePath = keyFromPath(entryPath);

    if (!moduleMap[effectivePath]) {
      const srcUrl = resolveHookUrl(baseDir, entryPath) + cacheBust;
      const srcResp = await fetch(srcUrl, { headers: branchHeaders });
      if (!srcResp.ok) throw await hookFetchError(srcResp, srcUrl);
      source = await srcResp.text();
      putModule(entryPath, source);
    } else {
      source = moduleMap[effectivePath];
    }

    setBootStatus(container, {
      phase: 'modules',
      message: 'Resolving hook imports…',
      progress: `${moduleCount(moduleMap)}`,
    });
    await preloadImportClosure(
      moduleMap,
      async (rel) => {
        const resp = await fetch(resolveHookUrl(baseDir, rel) + cacheBust, { headers: branchHeaders });
        if (!resp.ok) {
          console.warn('[relay-runtime] import closure fetch failed:', rel, resp.status);
          return null;
        }
        return resp.text();
      },
      () => {
        setBootStatus(container, {
          phase: 'modules',
          message: `Resolving hook imports…`,
          progress: `${moduleCount(moduleMap)}`,
        });
      },
    );

    if (!hooksManifest.hooks || hooksManifest.hooks.length === 0) {
      hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
    }
    const baseNorm = base.replace(/\/+$/, '');
    hooksManifest.relayClientBaseUrl = baseNorm;
    // CleverScript: avoid building URLs with '/' in hook source; optional full URL for repo README.
    hooksManifest.relayDefaultMarkdownUrl = `${baseNorm}/README.md`;
    hooksManifest.relayPathSlash = '/';
    if (initialPathNorm) {
      hooksManifest.relayInitialPath = initialPathNorm.startsWith('/')
        ? initialPathNorm
        : `/${initialPathNorm}`;
    }
    if (branchNorm) {
      hooksManifest.relayBranch = branchNorm;
    }
    setRelayFetchBaseUrl(baseNorm);
    setRelayFetchBranch(branchNorm);
    setBootStatus(container, { phase: 'theme', message: 'Loading theme…' });
    if (!themeYamlText) {
      themeYamlText = await fetchThemeYamlForHost(baseDir, branchNorm);
    }
    if (container && container.dataset) {
      container.dataset.relayClientBaseUrl = baseNorm;
      if (branchNorm) container.dataset.relayBranch = branchNorm;
      if (initialPathNorm) {
        container.dataset.relayInitialPath = hooksManifest.relayInitialPath || initialPathNorm;
      }
    }
  } else {
    const url = resolveUrl(baseUrl, effectivePath);
    const r = await fetch(url);
    if (!r.ok) throw await hookFetchError(r, url);
    source = await r.text();
    putModuleInMap(moduleMap, effectivePath, source);
    await preloadImportClosure(moduleMap, async (rel) => {
      const resp = await fetch(resolveUrl(baseUrl, rel));
      if (!resp.ok) {
        console.warn('[relay-runtime] import closure fetch failed:', rel, resp.status);
        return null;
      }
      return resp.text();
    });
    hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
    if (initialPathNorm) {
      hooksManifest.relayInitialPath = initialPathNorm.startsWith('/')
        ? initialPathNorm
        : `/${initialPathNorm}`;
    }
    if (branchNorm) {
      hooksManifest.relayBranch = branchNorm;
      setRelayFetchBranch(branchNorm);
    }
  }

  renderHookPass = () => {
    try {
      setBootStatus(container, { phase: 'render', message: 'Rendering UI…' });
      injectHostManifestFields(hooksManifest, container);
      let vnodeJson;
      for (let pass = 0; pass < 16; pass++) {
        vnodeJson = run_hook_with_host(
          source,
          JSON.stringify(moduleMap),
          themeYamlText || '',
          JSON.stringify(hooksManifest),
          effectivePath
        );
        if (!consume_render_requested()) break;
      }
      applyInjectedCss(container);
      let vnode = JSON.parse(vnodeJson);
      const tryHydrate = wantHydrate && !hydratedOnce && container.dataset?.relaySsr === '1';
      if (tryHydrate) {
        // Soft hydrate always keeps SSR; promote shell even if some subtrees skipped.
        const ok = hydrateVnode(vnode, container, invoke_handler, renderHookPass);
        hydratedOnce = true;
        prevVnode = vnode;
        livePromoted = true;
        promoteSsrShellWithoutWipe(container, { hydrateOk: !!ok });
        enableClientOnlyChrome(container);
        setBootStatus(container, { phase: 'live', message: 'Ready' });
        scheduleDeferredLiveUpgrade();
        return;
      }
      if (
        livePromoted &&
        prevVnode &&
        container.childNodes &&
        container.childNodes.length > 0
      ) {
        // Parked SSR: DOM may diverge from prevVnode after soft-hydrate. Never
        // applyPatches against skewed trees (that was wiping main/DirNav). Only
        // swap marked placeholders / known islands.
        if (container.dataset?.relaySsr === '1') {
          try {
            replaceSsrPlaceholdersFromVnode(container, vnode, invoke_handler, renderHookPass);
          } catch (e) {
            console.warn('[relay-runtime] placeholder replace failed; keeping SSR', e);
          }
          prevVnode = vnode;
          setBootStatus(container, { phase: 'live', message: 'Ready' });
          return;
        }
        try {
          const patchList = computeVnodePatches(prevVnode, vnode);
          if (patchList.length === 0) {
            prevVnode = vnode;
            setBootStatus(container, { phase: 'live', message: 'Ready' });
            return;
          }
          if (applyVnodePatches(container, prevVnode, patchList, invoke_handler, renderHookPass)) {
            prevVnode = vnode;
            setBootStatus(container, { phase: 'live', message: 'Ready' });
            return;
          }
        } catch (e) {
          console.warn('[relay-runtime] applyPatches failed; full replace', e);
        }
      }
      // CSR / non-SSR only: full replace.
      if (container.dataset?.relaySsr === '1') {
        // Soft promote without wipe if we somehow reach here without livePromoted.
        hydratedOnce = true;
        prevVnode = vnode;
        livePromoted = true;
        promoteSsrShellWithoutWipe(container, { hydrateOk: false });
        enableClientOnlyChrome(container);
        setBootStatus(container, { phase: 'live', message: 'Ready' });
        scheduleDeferredLiveUpgrade();
        return;
      }
      container.innerHTML = '';
      delete container.dataset.relaySsr;
      if (!livePromoted) {
        try {
          if (typeof set_client_live_phase === 'function') {
            set_client_live_phase(1);
          } else if (typeof set_client_live === 'function') {
            set_client_live(true);
          }
        } catch (_) { /* optional */ }
        syncHtmlLivePhaseAttr(1, container);
        livePromoted = true;
        setBootStatus(container, { phase: 'live', message: 'Ready' });
        injectHostManifestFields(hooksManifest, container);
        // Re-run at shell phase only; catalog/feeds fill on idle ladder.
        for (let pass = 0; pass < 16; pass++) {
          vnodeJson = run_hook_with_host(
            source,
            JSON.stringify(moduleMap),
            themeYamlText || '',
            JSON.stringify(hooksManifest),
            effectivePath
          );
          if (!consume_render_requested()) break;
        }
        applyInjectedCss(container);
        vnode = JSON.parse(vnodeJson);
      }
      const el = vnodeToDom(vnode, invoke_handler, renderHookPass);
      if (el) container.appendChild(el);
      prevVnode = vnode;
      if (livePromoted) {
        setBootStatus(container, { phase: 'live', message: 'Ready' });
        installRumBeacon(container, { hydrateOk: !!container.dataset?.relayHydrated });
        scheduleDeferredLiveUpgrade();
      }
    } catch (e) {
      // SSR markup (if any) is still intact when render fails before the innerHTML wipe;
      // keep it readable and show diagnostics on top instead of blanking the page.
      renderHookErrorUI(container, e, {
        title: 'Render error',
        keepContent: container.dataset?.relaySsr === '1',
      });
      throw e;
    }
  };

  installEmbedActionDelegation(container, () => {
    renderHookPass();
  });
  // Flush paint + pending pointer events before the first WASM render.
  // Parked SSR gets a longer interactive yield so header absolute <a> stay hittable.
  setBootStatus(container, { phase: 'render', message: 'Preparing UI…' });
  if (wantHydrate && container.dataset?.relaySsr === '1') {
    await yieldForInteractive(120);
  } else {
    await yieldToMain();
  }
  renderHookPass();
  } catch (e) {
    const msg = formatRuntimeErrorMessage(e?.message ?? e ?? 'Boot failed');
    setBootStatus(container, { phase: 'error', message: 'Boot failed', detail: msg });
    renderHookErrorUI(container, e, {
      title: 'Boot error',
      keepContent: container?.dataset?.relaySsr === '1',
    });
    throw e;
  }
}

/**
 * Default baseUrl: directory containing theme.yaml, hooks.json, etc.
 * Resolve relative to this script so it works regardless of document URL.
 */
function getDefaultBaseUrl() {
  try {
    return new URL('../', import.meta.url).href;
  } catch {
    return '.';
  }
}

/**
 * Default paths relative to baseUrl.
 */
const DEFAULTS = {
  rootId: 'root',
  hooksPath: 'hooks.json',
  appPath: 'hooks/app.jsx',
  get baseUrl() {
    return getDefaultBaseUrl();
  },
};

/**
 * Resolve hook/theme/manifest path against a base URL.
 * Handles absolute URLs, root-relative paths (`/foo`), and relative paths (`./x`, `../x`).
 * Matches `new URL(path, baseWithSlash)` semantics.
 *
 * @param {string} baseUrl - Directory or origin base, e.g. `https://cdn.example.com/pkg/` or `http://10.0.2.2:8080`
 * @param {string} path - Path or URL to resolve
 * @returns {string}
 */
export function resolveHookUrl(baseUrl, path) {
  const p = (path ?? '').trim();
  if (!p) return baseUrl || '';
  if (/^https?:\/\//i.test(p)) return p;
  if (!baseUrl || baseUrl === '.') {
    try {
      const base =
        typeof document !== 'undefined' && document.baseURI
          ? document.baseURI
          : 'http://localhost/';
      return new URL(p, base).href;
    } catch {
      return p;
    }
  }
  const dir = baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
  try {
    return new URL(p, dir).href;
  } catch {
    return p;
  }
}

/** @param {string} baseUrl @param {string} path */
function resolveUrl(baseUrl, path) {
  return resolveHookUrl(baseUrl, path);
}

/**
 * Bootstrap relay-runtime: load WASM, fetch hooks manifest, run app, render to DOM.
 * Theme YAML is loaded by hook `<StyleContext src="…">` (`@clevertree/style`).
 *
 * @param {object} [options]
 * @param {string} [options.rootId='root'] - DOM id for mounting
 * @param {string} [options.hooksPath='hooks.json']
 * @param {string} [options.appPath='hooks/app.jsx']
 * @param {string} [options.baseUrl='.'] - Base URL for resolving paths (relative to page)
 * @param {string|URL} [options.wasmUrl] - Override WASM module URL (default: next to relay_runtime.js)
 * @returns {Promise<void>}
 */
export async function bootstrap(options = {}) {
  const opts = { ...DEFAULTS, ...options };
  const { rootId, hooksPath, appPath, baseUrl, wasmUrl } = opts;

  const root = document.getElementById(rootId);
  if (!root) {
    console.error('[relay-runtime] Root element not found:', rootId);
    return;
  }

  try {
    await init(wasmUrl);
    installRelayHostFetch();

    // Forward window messages to relay-runtime so hooks can call getPendingMessages().
    // Skip messages with type "relay-runtime" (sent by postMessage) to avoid echo.
    if (typeof window !== 'undefined') {
      window.addEventListener('message', (e) => {
        try {
          const d = e.data;
          if (d && typeof d === 'object' && d.type === 'relay-runtime') return;
          const payload = typeof d === 'string' ? d : JSON.stringify(d ?? null);
          push_message_from_host(payload);
        } catch (_) { /* ignore */ }
      });
    }

    const hooksUrl = resolveUrl(baseUrl, hooksPath);
    const appUrl = resolveUrl(baseUrl, appPath);
    const cacheBust = '?t=' + Date.now();

    const [hooksResp, appResp] = await Promise.all([
      fetch(hooksUrl + cacheBust),
      fetch(appUrl + cacheBust),
    ]);

    if (!hooksResp.ok) {
      console.warn('[relay-runtime] hooks fetch failed:', hooksResp.status, hooksUrl);
    }
    if (!appResp.ok) {
      console.warn('[relay-runtime] app fetch failed:', appResp.status, appUrl);
    }

    const manifest = hooksResp.ok ? await hooksResp.json() : { hooks: [] };
    const appSource = appResp.ok ? await appResp.text() : '';

    const moduleMap = {};
    const bootHooks = (manifest.hooks || []).filter((h) => h && h.path);
    await Promise.all(
      bootHooks.map(async (h) => {
        const url = resolveUrl(baseUrl, h.path) + cacheBust;
        try {
          const resp = await fetch(url);
          if (resp.ok) {
            putModuleInMap(moduleMap, h.path, await resp.text());
          } else {
            console.warn('[relay-runtime] hook fetch failed:', resp.status, h.path, url);
          }
        } catch (err) {
          console.warn('[relay-runtime] hook fetch error:', h.path, err);
        }
      }),
    );
    if (appSource) {
      putModuleInMap(moduleMap, appPath || 'hooks/app.jsx', appSource);
    }
    await preloadImportClosure(moduleMap, async (rel) => {
      const url = resolveUrl(baseUrl, rel) + cacheBust;
      try {
        const resp = await fetch(url);
        if (!resp.ok) {
          console.warn('[relay-runtime] import closure fetch failed:', rel, resp.status, url);
          return null;
        }
        return resp.text();
      } catch (err) {
        console.warn('[relay-runtime] import closure fetch error:', rel, err);
        return null;
      }
    });

    const hookCount = (manifest.hooks || []).length;
    const loadedCount = Object.keys(moduleMap).length;
    if (hookCount === 0) {
      console.warn('[relay-runtime] hooks manifest has no hooks');
    } else if (loadedCount < hookCount) {
      console.warn('[relay-runtime] loaded', loadedCount, 'of', hookCount, 'hooks; baseUrl:', baseUrl);
    }

    // Debug: log what we're passing when troubleshooting empty render
    if (hookCount === 0 || loadedCount === 0 || !appSource) {
      console.warn('[relay-runtime] bootstrap inputs:', {
        hookCount,
        loadedCount,
        appLen: appSource.length,
        modulePaths: Object.keys(moduleMap),
        baseUrl,
      });
    }

    const doRender = () => {
      try {
        let vnodeJson;
        try {
          for (let pass = 0; pass < 16; pass++) {
            vnodeJson = run_hook_with_host(
              appSource,
              JSON.stringify(moduleMap),
              '',
              JSON.stringify(manifest),
              appPath || ''
            );
            if (!consume_render_requested()) break;
          }
        } catch (e) {
          console.error('[relay-runtime] run_hook_with_host failed:', e);
          throw e;
        }
        applyInjectedCss(root);
        root.innerHTML = '';
        const vnode = JSON.parse(vnodeJson);
        const el = vnodeToDom(vnode, invoke_handler, doRender);
        if (el) {
          root.appendChild(el);
        } else {
          const empty = document.createElement('p');
          empty.className = 'relay-empty';
          empty.textContent = 'Rendered (no content). Check that hooks manifest and theme are loaded.';
          root.appendChild(empty);
          if (typeof console !== 'undefined' && console.debug) {
            console.debug('[relay-runtime] vnode was empty:', vnode);
          }
        }
      } catch (e) {
        console.error('[relay-runtime] doRender failed:', e);
        throw e;
      }
    };
    doRender();

    // Event delegation: theme toggle (data-theme)
    root.addEventListener('click', (e) => {
      const theme = e.target?.dataset?.theme;
      if (theme) {
        setCurrentTheme(theme);
      }
    });
  } catch (e) {
    root.textContent = '';
    root.className = 'error';
    let msg = e?.message ?? e?.toString?.() ?? String(e);
    // Parse errors may be JSON {message, line, column, file?, source_line?} for advanced UI
    try {
      const parsed = JSON.parse(msg);
      if (parsed && typeof parsed.message === 'string') {
        const loc = [parsed.file, parsed.line, parsed.column].filter(Boolean).join(':');
        msg = loc ? `${loc}: ${parsed.message}` : parsed.message;
        if (parsed.source_line) {
          msg += `\n  ${parsed.source_line}\n  ${' '.repeat((parsed.column || 1) - 1)}^`;
        }
      }
    } catch (_) { /* not JSON, use as-is */ }
    root.textContent = `Init error: ${msg || 'Unknown error'}`;
    root.style.whiteSpace = 'pre-wrap';
    console.error('[relay-runtime] Init error:', e);
  }
}

/**
 * Get available theme names. Call after bootstrap().
 * @returns {string[]} Theme keys (e.g. ['default', 'light', 'dark'])
 */
export function getThemes() {
  try {
    const json = get_themes();
    const arr = JSON.parse(json || '[]');
    return Array.isArray(arr) ? arr.map((t) => (t && t.key ? t.key : String(t))) : [];
  } catch {
    return [];
  }
}

/**
 * Set current theme and update styles. Use with theme toggle buttons (`data-theme="…"`)
 * or CleverScript `setCurrentTheme` from `@clevertree/style-internal`.
 * @param {string} name - Theme name (e.g. 'light', 'dark')
 */
export function setCurrentTheme(name) {
  const n = String(name || '').trim();
  if (typeof document !== 'undefined') {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    if (n === 'light' || n === 'dark') {
      root.classList.add(n);
    }
  }
  let css = '';
  try {
    css = typeof set_theme_and_render === 'function' ? set_theme_and_render(n) : '';
  } catch (_) {
    /* optional */
  }
  if (!css || !String(css).trim()) {
    try {
      css = typeof get_injected_css === 'function' ? get_injected_css() : '';
    } catch (_) {
      css = '';
    }
  }
  if (css && String(css).trim()) {
    const styleEl = ensureStyleElement(_styleId);
    if (styleEl) styleEl.textContent = String(css);
  }
}

/** Install click delegation for `data-theme` and `data-relay-action` on a mount root. */
function installEmbedActionDelegation(root, onThemeChange) {
  if (!root || root.dataset.relayActionsBound === '1') return;
  root.dataset.relayActionsBound = '1';
  root.addEventListener('click', (e) => {
    const actionEl = e.target?.closest?.('[data-relay-action]');
    if (actionEl && !actionEl.disabled) {
      const action = actionEl.getAttribute('data-relay-action');
      if (action === 'print' && typeof window !== 'undefined' && typeof window.print === 'function') {
        e.preventDefault();
        window.print();
        return;
      }
    }
    const themeEl = e.target?.closest?.('[data-theme]');
    if (themeEl && !themeEl.disabled) {
      const theme = themeEl.getAttribute('data-theme');
      if (theme) {
        setCurrentTheme(theme);
        if (typeof onThemeChange === 'function') onThemeChange(theme);
      }
    }
  });
}
