/**
 * @relay/gallery web host — Custom Elements + soft image retry (no npm React).
 * Loaded from relay-runtime-web.js; installs __relayGallery / CE tags.
 */

const BACKOFF_MS = [250, 500, 1000, 2000, 4000, 4000];
const DEFAULT_MAX_ATTEMPTS = 6;

function str(v) {
  return typeof v === 'string' ? v : v == null ? '' : String(v);
}

/** Strip prior `_rg` cache-bust and append a fresh attempt marker. */
export function cacheBustSrc(url, attempt) {
  const raw = str(url);
  if (!raw) return raw;
  try {
    const hashIdx = raw.indexOf('#');
    const hash = hashIdx >= 0 ? raw.slice(hashIdx) : '';
    const noHash = hashIdx >= 0 ? raw.slice(0, hashIdx) : raw;
    const qIdx = noHash.indexOf('?');
    const path = qIdx >= 0 ? noHash.slice(0, qIdx) : noHash;
    const qs = qIdx >= 0 ? noHash.slice(qIdx + 1) : '';
    const parts = qs
      ? qs.split('&').filter((p) => p && p.indexOf('_rg=') !== 0)
      : [];
    parts.push('_rg=' + String(attempt));
    return path + '?' + parts.join('&') + hash;
  } catch (_) {
    return raw + (raw.indexOf('?') >= 0 ? '&' : '?') + '_rg=' + String(attempt);
  }
}

export function backoffDelay(attemptIndex) {
  if (attemptIndex < 0) return BACKOFF_MS[0];
  if (attemptIndex >= BACKOFF_MS.length) return BACKOFF_MS[BACKOFF_MS.length - 1];
  return BACKOFF_MS[attemptIndex];
}

/**
 * Soft-retry image loader for transient 404s (IPFS/path-cache catch-up).
 * @param {HTMLImageElement} img
 * @param {string} src
 * @param {{ maxAttempts?: number, onState?: (state: string) => void }} [opts]
 * @returns {{ cancel: () => void, reload: (nextSrc: string) => void }}
 */
export function attachSoftRetryImage(img, src, opts) {
  const maxAttempts =
    opts && typeof opts.maxAttempts === 'number' && opts.maxAttempts > 0
      ? opts.maxAttempts
      : DEFAULT_MAX_ATTEMPTS;
  const onState = opts && typeof opts.onState === 'function' ? opts.onState : null;
  let baseSrc = str(src);
  let attempt = 0;
  let timer = null;
  let cancelled = false;

  function setState(state) {
    try {
      img.setAttribute('data-relay-img-state', state);
    } catch (_) {}
    if (onState) {
      try {
        onState(state);
      } catch (_) {}
    }
  }

  function clearTimer() {
    if (timer != null) {
      try {
        clearTimeout(timer);
      } catch (_) {}
      timer = null;
    }
  }

  function applySrc(next, bustAttempt) {
    const url = bustAttempt > 0 ? cacheBustSrc(next, bustAttempt) : next;
    try {
      img.src = url;
    } catch (_) {}
  }

  function onLoad() {
    if (cancelled) return;
    clearTimer();
    attempt = 0;
    setState('ok');
  }

  function onError() {
    if (cancelled) return;
    if (!baseSrc) {
      setState('failed');
      return;
    }
    if (attempt >= maxAttempts) {
      setState('failed');
      return;
    }
    setState('retrying');
    const delay = backoffDelay(attempt);
    attempt += 1;
    clearTimer();
    timer = setTimeout(() => {
      if (cancelled) return;
      applySrc(baseSrc, attempt);
    }, delay);
  }

  img.addEventListener('load', onLoad);
  img.addEventListener('error', onError);
  setState(baseSrc ? 'loading' : 'failed');
  if (baseSrc) applySrc(baseSrc, 0);

  return {
    cancel() {
      cancelled = true;
      clearTimer();
      try {
        img.removeEventListener('load', onLoad);
        img.removeEventListener('error', onError);
      } catch (_) {}
    },
    reload(nextSrc) {
      baseSrc = str(nextSrc);
      attempt = 0;
      clearTimer();
      cancelled = false;
      setState(baseSrc ? 'loading' : 'failed');
      if (baseSrc) applySrc(baseSrc, 0);
    },
  };
}

function createGalleryStore() {
  let s = {
    images: [],
    override: null,
    index: -1,
    open: false,
    rev: 0,
    listeners: [],
  };

  function emit() {
    s.rev += 1;
    const list = s.listeners.slice();
    for (let i = 0; i < list.length; i++) {
      try {
        list[i](s.rev);
      } catch (_) {}
    }
  }

  function images() {
    return s.override || s.images;
  }

  function strSrc(v) {
    return typeof v === 'string' ? v : '';
  }

  function normalizeImg(img) {
    const src = strSrc(img && img.src);
    if (!src) return null;
    return {
      src,
      alt: img && typeof img.alt === 'string' ? img.alt : '',
      highResSrc: img && typeof img.highResSrc === 'string' ? img.highResSrc : '',
      children: img && img.children != null ? img.children : null,
    };
  }

  function safeImgDomId(srcRaw) {
    const src = strSrc(srcRaw);
    let id = 'img-';
    for (let i = 0; i < src.length; i++) {
      const c = src.charAt(i);
      if (/[a-zA-Z0-9]/.test(c)) id += c;
      else id += '-';
    }
    return id || 'img-unknown';
  }

  function galleryImgHash(srcRaw) {
    return 'img=' + strSrc(srcRaw);
  }

  function readHashImgSrc(hash) {
    const h = str(hash);
    const body = h.charAt(0) === '#' ? h.slice(1) : h;
    if (body.indexOf('img=') !== 0) return '';
    try {
      return decodeURIComponent(body.slice(4));
    } catch (_) {
      return body.slice(4);
    }
  }

  function galleryDisplaySrc(img) {
    if (!img) return '';
    const hi = typeof img.highResSrc === 'string' ? img.highResSrc : '';
    if (hi) return hi;
    return strSrc(img.src);
  }

  return {
    galleryRev: () => s.rev,
    gallerySubscribe(fn) {
      s.listeners = s.listeners.concat([fn]);
      return () => {
        s.listeners = s.listeners.filter((f) => f !== fn);
      };
    },
    galleryImages: () => images().slice(),
    galleryIndex: () => s.index,
    galleryIsOpen: () => s.open === true,
    registerGalleryImage(img) {
      const n = normalizeImg(img);
      if (!n) return;
      if (s.images.some((i) => i.src === n.src)) return;
      s.images = s.images.concat([n]);
      emit();
    },
    syncGalleryImageMeta(img) {
      const n = normalizeImg(img);
      if (!n) return;
      s.images = s.images.map((i) => (i.src === n.src ? n : i));
    },
    unregisterGalleryImage(srcRaw) {
      const src = strSrc(srcRaw);
      if (!src) return;
      const next = s.images.filter((i) => i.src !== src);
      if (next.length === s.images.length) return;
      s.images = next;
      emit();
    },
    openGalleryAt(srcRaw) {
      const src = strSrc(srcRaw);
      const list = images();
      let idx = -1;
      for (let i = 0; i < list.length; i++) {
        if (list[i].src === src) {
          idx = i;
          break;
        }
      }
      if (idx < 0) {
        s.override = [{ src, alt: '', highResSrc: '', children: null }];
        s.index = 0;
        s.open = true;
        emit();
        return;
      }
      s.override = null;
      s.index = idx;
      s.open = true;
      emit();
    },
    openGallery(items, startIndex) {
      let list = [];
      if (items && typeof items.length === 'number') {
        for (let i = 0; i < items.length; i++) {
          const n = normalizeImg(items[i]);
          if (n) list = list.concat([n]);
        }
      }
      if (!list.length) return;
      let idx = typeof startIndex === 'number' ? startIndex : 0;
      if (idx < 0) idx = 0;
      if (idx >= list.length) idx = list.length - 1;
      s.override = list;
      s.index = idx;
      s.open = true;
      emit();
    },
    closeGallery() {
      if (!s.open && !s.override) return;
      s.open = false;
      s.override = null;
      try {
        const loc = document.location;
        if (loc && typeof loc.hash === 'string' && loc.hash.indexOf('#img=') === 0) {
          loc.hash = '';
        }
      } catch (_) {}
      emit();
    },
    galleryGoNext() {
      const list = images();
      if (list.length <= 1) return;
      s.index = (s.index + 1) % list.length;
      emit();
    },
    galleryGoPrev() {
      const list = images();
      if (list.length <= 1) return;
      s.index = (s.index - 1 + list.length) % list.length;
      emit();
    },
    setGalleryHash(srcRaw) {
      const src = strSrc(srcRaw);
      if (!src) return;
      try {
        const loc = document.location;
        if (!loc) return;
        const hashBody = galleryImgHash(src);
        if (loc.hash === '#' + hashBody) return;
        loc.hash = hashBody;
      } catch (_) {}
    },
    readHashImgSrc() {
      try {
        const loc = document.location;
        if (!loc || typeof loc.hash !== 'string') return '';
        return readHashImgSrc(loc.hash);
      } catch (_) {
        return '';
      }
    },
    galleryDisplaySrc,
    safeImgDomId,
    galleryImgHash,
  };
}

const GALLERY_CSS = `
:root {
  --relay-gallery-bg: rgba(0,0,0,0.92);
  --relay-gallery-fg: #f5f5f5;
  --relay-gallery-z: 200;
}
.relay-pop-image {
  display: block;
  margin: 1rem 0;
}
.relay-pop-image-control {
  cursor: pointer;
  position: relative;
  outline: none;
}
.relay-pop-image-inner {
  position: relative;
  overflow: hidden;
  background: rgba(127,127,127,0.12);
}
.relay-pop-image-img {
  display: block;
  width: 100%;
  height: auto;
  object-fit: cover;
}
.relay-pop-image-img[data-relay-img-state="loading"],
.relay-pop-image-img[data-relay-img-state="retrying"],
.relay-pop-image-img[data-relay-img-state="failed"] {
  min-height: 8rem;
  background: linear-gradient(90deg, rgba(127,127,127,0.08), rgba(127,127,127,0.18), rgba(127,127,127,0.08));
  background-size: 200% 100%;
}
.relay-pop-image-hover {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.15s ease;
  background: rgba(0,0,0,0.25);
  pointer-events: none;
}
.relay-pop-image-control:hover .relay-pop-image-hover,
.relay-pop-image-control:focus .relay-pop-image-hover {
  opacity: 1;
}
.relay-pop-image-max {
  color: #fff;
  font-size: 1.5rem;
}
.relay-pop-image-caption {
  margin-top: 0.5rem;
  font-size: 0.9rem;
  opacity: 0.85;
}
.relay-gallery {
  position: fixed;
  inset: 0;
  z-index: var(--relay-gallery-z);
  display: flex;
  flex-direction: column;
  background: var(--relay-gallery-bg);
  color: var(--relay-gallery-fg);
}
.relay-gallery[hidden] { display: none !important; }
.relay-gallery-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  gap: 1rem;
}
.relay-gallery-count {
  font-size: 0.85rem;
  opacity: 0.8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.relay-gallery-close,
.relay-gallery-nav {
  background: transparent;
  border: 0;
  color: inherit;
  cursor: pointer;
  font-size: 1.75rem;
  line-height: 1;
  padding: 0.5rem 0.75rem;
}
.relay-gallery-stage {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  min-height: 0;
  padding: 0 3rem;
}
.relay-gallery-img {
  max-width: 100%;
  max-height: 85vh;
  object-fit: contain;
}
.relay-gallery-img[data-relay-img-state="loading"],
.relay-gallery-img[data-relay-img-state="retrying"],
.relay-gallery-img[data-relay-img-state="failed"] {
  min-width: 12rem;
  min-height: 12rem;
  background: rgba(255,255,255,0.06);
}
.relay-gallery-prev { position: absolute; left: 0.25rem; top: 50%; transform: translateY(-50%); }
.relay-gallery-next { position: absolute; right: 0.25rem; top: 50%; transform: translateY(-50%); }
.relay-gallery-caption {
  text-align: center;
  padding: 0.75rem 1rem 1.25rem;
  font-size: 0.95rem;
}
`;

function ensureGalleryCss() {
  if (typeof document === 'undefined') return;
  if (document.getElementById('relay-gallery-css')) return;
  const style = document.createElement('style');
  style.id = 'relay-gallery-css';
  style.textContent = GALLERY_CSS;
  document.head.appendChild(style);
}

function propFromEl(el, key, fallback) {
  const attr = key === 'className' ? 'class' : key;
  const camel = key;
  const dataKey =
    'data-' + key.replace(/[A-Z]/g, (m) => '-' + m.toLowerCase());
  let v = el.getAttribute(attr);
  if (v == null || v === '') v = el.getAttribute(dataKey);
  if (v == null || v === '') {
    try {
      if (el.dataset && el.dataset[camel]) v = el.dataset[camel];
    } catch (_) {}
  }
  if (v == null || v === '') return fallback;
  return v;
}

function captionFor(img) {
  if (!img) return '';
  if (typeof img.children === 'string' && img.children) return img.children;
  if (typeof img.alt === 'string' && img.alt) return img.alt;
  const src = str(img.src);
  if (!src) return '';
  const parts = src.split('/');
  let base = parts.length ? parts[parts.length - 1] : '';
  const q = base.indexOf('?');
  if (q >= 0) base = base.slice(0, q);
  const dot = base.lastIndexOf('.');
  if (dot > 0) base = base.slice(0, dot);
  return base.split('_').join(' ');
}

function readPortraitSibling(src) {
  try {
    const map = globalThis.__RELAY_PORTRAIT_SIBLINGS__;
    if (!map || typeof map !== 'object') return '';
    // Expect map keys as repo-relative paths; portraitSiblingSrc convention: foo.ext → foo-portrait.ext
    const raw = str(src);
    const q = raw.indexOf('?');
    const path = q >= 0 ? raw.slice(0, q) : raw;
    const dot = path.lastIndexOf('.');
    if (dot <= 0) return '';
    const portrait = path.slice(0, dot) + '-portrait' + path.slice(dot);
    const rel = portrait.replace(/^\//, '');
    if (map[rel] || map[portrait] || map['/' + rel]) {
      return portrait;
    }
  } catch (_) {}
  return '';
}

function buildPopImageElement(props) {
  ensureGalleryCss();
  const gallery = ensureGalleryApi();
  const src = str(props.src || props['data-src'] || '');
  const alt = str(props.alt || props['data-alt'] || '');
  const className = str(props.className || props.class || '');
  const highResSrc = str(props.highResSrc || props['data-high-res-src'] || '');
  const aspectRatio = str(props.aspectRatio || props['data-aspect-ratio'] || '3 / 2');
  const retryRaw = props.retry != null ? props.retry : props['data-retry'];
  const maxAttempts =
    retryRaw != null && !Number.isNaN(Number(retryRaw)) ? Number(retryRaw) : DEFAULT_MAX_ATTEMPTS;
  let caption = '';
  if (typeof props.children === 'string') caption = props.children;
  else if (Array.isArray(props.children) && props.children.length === 1) {
    const c = props.children[0];
    if (typeof c === 'string') caption = c;
    else if (c && c.Text) caption = String(c.Text);
    else if (c && c.text) caption = String(c.text);
  }

  // Dual-class for park themes that still target .pt-popimage* (Paradigm Threat).
  const root = document.createElement('aside');
  root.className =
    'relay-pop-image pt-popimage' + (className ? ' ' + className : '');
  root.setAttribute('data-relay-popimage', 'true');
  root.setAttribute('data-pt-popimage', 'true');
  root.setAttribute('aria-label', alt || 'Figure');

  const control = document.createElement('div');
  control.className = 'relay-pop-image-control';
  control.id = gallery.safeImgDomId(src);
  control.setAttribute('role', 'button');
  control.tabIndex = 0;

  const inner = document.createElement('div');
  inner.className = 'relay-pop-image-inner pt-popimage-inner';
  inner.style.aspectRatio = aspectRatio;

  const img = document.createElement('img');
  img.className = 'relay-pop-image-img pt-popimage-img';
  img.alt = alt;
  img.loading = 'lazy';
  img.width = 360;
  img.height = 240;
  img.style.width = '100%';
  img.style.height = 'auto';
  img.style.aspectRatio = aspectRatio;

  const portraitSrc = readPortraitSibling(src);
  if (portraitSrc) {
    const picture = document.createElement('picture');
    const source = document.createElement('source');
    source.media = '(max-width: 768px)';
    source.srcset = portraitSrc;
    picture.appendChild(source);
    picture.appendChild(img);
    inner.appendChild(picture);
  } else {
    inner.appendChild(img);
  }

  const hover = document.createElement('div');
  hover.className = 'relay-pop-image-hover pt-popimage-hover';
  hover.setAttribute('aria-hidden', 'true');
  const max = document.createElement('span');
  max.className = 'relay-pop-image-max pt-popimage-max';
  max.textContent = '⤢';
  hover.appendChild(max);
  inner.appendChild(hover);
  control.appendChild(inner);

  if (caption) {
    const cap = document.createElement('div');
    cap.className = 'relay-pop-image-caption pt-popimage-caption';
    cap.textContent = caption;
    control.appendChild(cap);
  }

  root.appendChild(control);

  const loader = attachSoftRetryImage(img, src, { maxAttempts });

  if (src) {
    gallery.registerGalleryImage({
      src,
      alt,
      highResSrc,
      children: caption || null,
    });
  }

  const open = () => {
    if (!src) return;
    gallery.openGalleryAt(src);
  };
  control.addEventListener('click', open);
  control.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      open();
    }
  });

  // Deep-link open
  try {
    const hashSrc = gallery.readHashImgSrc();
    if (hashSrc && hashSrc === src) {
      gallery.openGalleryAt(src);
      try {
        control.scrollIntoView({ behavior: 'auto', block: 'center' });
      } catch (_) {}
    }
  } catch (_) {}

  root._relaySoftRetry = loader;
  return root;
}

function buildGalleryOverlayElement() {
  ensureGalleryCss();
  const gallery = ensureGalleryApi();
  const root = document.createElement('div');
  root.className = 'relay-gallery pt-gallery';
  root.setAttribute('data-relay-gallery', 'true');
  root.setAttribute('data-pt-gallery', 'true');
  root.setAttribute('role', 'dialog');
  root.setAttribute('aria-modal', 'true');
  root.setAttribute('aria-label', 'Image gallery');
  root.hidden = true;

  const header = document.createElement('div');
  header.className = 'relay-gallery-header pt-gallery-header';
  const count = document.createElement('div');
  count.className = 'relay-gallery-count pt-gallery-count';
  const closeBtn = document.createElement('button');
  closeBtn.type = 'button';
  closeBtn.className = 'relay-gallery-close pt-gallery-close';
  closeBtn.setAttribute('aria-label', 'Close gallery');
  closeBtn.textContent = '✕';
  header.appendChild(count);
  header.appendChild(closeBtn);

  const stage = document.createElement('div');
  stage.className = 'relay-gallery-stage pt-gallery-stage';
  const prevBtn = document.createElement('button');
  prevBtn.type = 'button';
  prevBtn.className = 'relay-gallery-nav relay-gallery-prev pt-gallery-nav pt-gallery-prev';
  prevBtn.setAttribute('aria-label', 'Previous image');
  prevBtn.textContent = '‹';
  const nextBtn = document.createElement('button');
  nextBtn.type = 'button';
  nextBtn.className = 'relay-gallery-nav relay-gallery-next pt-gallery-nav pt-gallery-next';
  nextBtn.setAttribute('aria-label', 'Next image');
  nextBtn.textContent = '›';
  const img = document.createElement('img');
  img.className = 'relay-gallery-img pt-gallery-img';
  img.alt = '';
  stage.appendChild(prevBtn);
  stage.appendChild(img);
  stage.appendChild(nextBtn);

  const captionEl = document.createElement('div');
  captionEl.className = 'relay-gallery-caption pt-gallery-caption';

  root.appendChild(header);
  root.appendChild(stage);
  root.appendChild(captionEl);

  let loader = null;
  let bodyOverflowPrev = '';
  let keyHandler = null;

  function teardownChrome() {
    if (keyHandler) {
      try {
        document.removeEventListener('keydown', keyHandler);
      } catch (_) {}
      keyHandler = null;
    }
    try {
      if (document.body) document.body.style.overflow = bodyOverflowPrev;
    } catch (_) {}
  }

  function render() {
    const open = gallery.galleryIsOpen();
    const images = gallery.galleryImages();
    const index = gallery.galleryIndex();
    const current = open && index >= 0 && index < images.length ? images[index] : null;

    if (!open || !current) {
      root.hidden = true;
      teardownChrome();
      if (loader) {
        loader.cancel();
        loader = null;
      }
      return;
    }

    root.hidden = false;
    const displaySrc = gallery.galleryDisplaySrc(current);
    count.textContent = index + 1 + ' / ' + images.length + ' — ' + current.src;
    const multi = images.length > 1;
    prevBtn.hidden = !multi;
    nextBtn.hidden = !multi;
    img.alt = current.alt || '';
    const cap = captionFor(current);
    captionEl.textContent = cap;
    captionEl.hidden = !cap;

    gallery.setGalleryHash(current.src);

    if (loader) loader.reload(displaySrc);
    else loader = attachSoftRetryImage(img, displaySrc, {});

    if (!keyHandler) {
      keyHandler = (e) => {
        if (e.key === 'Escape') gallery.closeGallery();
        if (e.key === 'ArrowRight') gallery.galleryGoNext();
        if (e.key === 'ArrowLeft') gallery.galleryGoPrev();
      };
      document.addEventListener('keydown', keyHandler);
      try {
        bodyOverflowPrev = document.body.style.overflow;
        document.body.style.overflow = 'hidden';
      } catch (_) {}
    }
  }

  closeBtn.addEventListener('click', () => gallery.closeGallery());
  prevBtn.addEventListener('click', () => gallery.galleryGoPrev());
  nextBtn.addEventListener('click', () => gallery.galleryGoNext());
  root.addEventListener('click', (e) => {
    if (e.target === root) gallery.closeGallery();
  });

  gallery.gallerySubscribe(render);
  render();
  return root;
}

function propsFromVnode(props) {
  const out = {};
  if (!props) return out;
  for (const [k, v] of Object.entries(props)) {
    if (v && typeof v === 'object' && 'value' in v) out[k] = v.value;
    else out[k] = v;
  }
  return out;
}

export function buildRelayPopImage(props) {
  return buildPopImageElement(propsFromVnode(props));
}

export function buildRelayImageGallery(props) {
  void props;
  return buildGalleryOverlayElement();
}

let _galleryApi = null;

export function ensureGalleryApi() {
  if (_galleryApi) return _galleryApi;
  if (typeof globalThis !== 'undefined' && globalThis.__relayGallery) {
    _galleryApi = globalThis.__relayGallery;
    return _galleryApi;
  }
  _galleryApi = createGalleryStore();
  if (typeof globalThis !== 'undefined') {
    globalThis.__relayGallery = _galleryApi;
    globalThis.__relayAttachSoftRetryImage = attachSoftRetryImage;
    globalThis.__relayGalleryCall = function (op, arg) {
      const g = _galleryApi;
      if (!g) return;
      if (op === 'openAt') g.openGalleryAt(arg);
      else if (op === 'close') g.closeGallery();
      else if (op === 'next') g.galleryGoNext();
      else if (op === 'prev') g.galleryGoPrev();
      else if (op === 'register') g.registerGalleryImage({ src: arg });
      else if (op === 'unregister') g.unregisterGalleryImage(arg);
      else if (op === 'setHash') g.setGalleryHash(arg);
    };
  }
  return _galleryApi;
}

export function installRelayGalleryCustomElements() {
  ensureGalleryApi();
  ensureGalleryCss();
  if (typeof customElements === 'undefined') return;

  if (!customElements.get('relay-pop-image')) {
    customElements.define(
      'relay-pop-image',
      class extends HTMLElement {
        connectedCallback() {
          if (this._relayBuilt) return;
          this._relayBuilt = true;
          const props = {
            src: propFromEl(this, 'src', ''),
            alt: propFromEl(this, 'alt', ''),
            className: propFromEl(this, 'class', '') || propFromEl(this, 'className', ''),
            highResSrc: propFromEl(this, 'highResSrc', '') || propFromEl(this, 'high-res-src', ''),
            aspectRatio: propFromEl(this, 'aspectRatio', '') || propFromEl(this, 'aspect-ratio', '3 / 2'),
            retry: propFromEl(this, 'retry', ''),
            children: this.textContent || '',
          };
          const built = buildPopImageElement(props);
          this.replaceChildren(built);
        }
      },
    );
  }

  if (!customElements.get('relay-image-gallery')) {
    customElements.define(
      'relay-image-gallery',
      class extends HTMLElement {
        connectedCallback() {
          // Host must never keep theme overlay classes (e.g. `.pt-gallery` fullscreen).
          // Those styles belong on the inner overlay root built below.
          try {
            this.removeAttribute('class');
            this.style.display = 'contents';
          } catch (_) {}
          if (this._relayBuilt) return;
          this._relayBuilt = true;
          const built = buildGalleryOverlayElement();
          this.replaceChildren(built);
        }
      },
    );
  }
}

/** Register park-native builders + define CEs. Call once from relay-runtime-web boot. */
export function installRelayGalleryHost() {
  ensureGalleryApi();
  installRelayGalleryCustomElements();
  try {
    if (typeof globalThis.__relayRegisterParkNative === 'function') {
      globalThis.__relayRegisterParkNative('relay-pop-image', (props) =>
        buildRelayPopImage(props),
      );
      globalThis.__relayRegisterParkNative('relay-image-gallery', (props) =>
        buildRelayImageGallery(props),
      );
    }
  } catch (_) {}
}
