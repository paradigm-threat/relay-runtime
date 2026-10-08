/**
 * Soft-retry + gallery store unit tests (no browser CE required for core helpers).
 * Run: node js/relay-gallery.test.mjs
 */
import assert from 'node:assert/strict'
import {
  cacheBustSrc,
  backoffDelay,
  attachSoftRetryImage,
  ensureGalleryApi,
} from './relay-gallery.js'

assert.equal(backoffDelay(0), 250)
assert.equal(backoffDelay(1), 500)
assert.equal(backoffDelay(2), 1000)
assert.equal(backoffDelay(3), 2000)
assert.equal(backoffDelay(4), 4000)
assert.equal(backoffDelay(99), 4000)

assert.equal(cacheBustSrc('/poster.jpg', 1), '/poster.jpg?_rg=1')
assert.equal(cacheBustSrc('/poster.jpg?w=800', 2), '/poster.jpg?w=800&_rg=2')
assert.equal(cacheBustSrc('/poster.jpg?_rg=1&w=800', 3), '/poster.jpg?w=800&_rg=3')
assert.equal(cacheBustSrc('/poster.jpg#frag', 1), '/poster.jpg?_rg=1#frag')

// Minimal img stub for soft-retry (no DOM)
function makeImg() {
  const listeners = { load: [], error: [] }
  return {
    src: '',
    attrs: {},
    setAttribute(k, v) {
      this.attrs[k] = v
    },
    addEventListener(type, fn) {
      listeners[type].push(fn)
    },
    removeEventListener(type, fn) {
      listeners[type] = listeners[type].filter((f) => f !== fn)
    },
    _emit(type) {
      for (const fn of listeners[type]) fn()
    },
  }
}

await new Promise((resolve, reject) => {
  const img = makeImg()
  const delays = []
  const realSetTimeout = globalThis.setTimeout
  const realClearTimeout = globalThis.clearTimeout
  globalThis.setTimeout = (fn, ms) => {
    delays.push(ms)
    return realSetTimeout(fn, 0)
  }
  globalThis.clearTimeout = realClearTimeout

  const handle = attachSoftRetryImage(img, '/missing.jpg', { maxAttempts: 3 })
  assert.equal(img.attrs['data-relay-img-state'], 'loading')
  assert.equal(img.src, '/missing.jpg')

  img._emit('error')
  assert.equal(img.attrs['data-relay-img-state'], 'retrying')

  realSetTimeout(() => {
    try {
      assert.ok(delays.length >= 1, 'scheduled backoff')
      assert.equal(delays[0], 250)
      assert.ok(String(img.src).includes('_rg=1'), img.src)
      img._emit('error')
      realSetTimeout(() => {
        try {
          assert.equal(delays[1], 500)
          img._emit('error')
          realSetTimeout(() => {
            try {
              // attempt is now 3; next error exhausts maxAttempts=3
              img._emit('error')
              assert.equal(img.attrs['data-relay-img-state'], 'failed')
              handle.cancel()
              globalThis.setTimeout = realSetTimeout
              resolve()
            } catch (e) {
              reject(e)
            }
          }, 5)
        } catch (e) {
          reject(e)
        }
      }, 5)
    } catch (e) {
      reject(e)
    }
  }, 5)
})

await new Promise((resolve, reject) => {
  const img = makeImg()
  const handle = attachSoftRetryImage(img, '/ok.jpg', { maxAttempts: 6 })
  img._emit('load')
  try {
    assert.equal(img.attrs['data-relay-img-state'], 'ok')
    handle.cancel()
    resolve()
  } catch (e) {
    reject(e)
  }
})

const g = ensureGalleryApi()
g.registerGalleryImage({ src: '/a.png', alt: 'A' })
g.registerGalleryImage({ src: '/b.png', alt: 'B' })
g.openGalleryAt('/b.png')
assert.equal(g.galleryIsOpen(), true)
assert.equal(g.galleryIndex(), 1)
g.galleryGoNext()
assert.equal(g.galleryIndex(), 0)
g.closeGallery()
assert.equal(g.galleryIsOpen(), false)

console.log('relay-gallery.test.mjs: ok')
