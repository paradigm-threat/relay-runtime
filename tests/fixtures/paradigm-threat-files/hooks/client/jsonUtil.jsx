export function queryIndexBody(query, limit, order) {
  let obj = {
    collection: 'index',
    query: query,
    limit: limit,
  }
  if (order && typeof order === 'string' && order.length > 0) {
    obj = {
      collection: 'index',
      query: query,
      limit: limit,
      order: order,
    }
  }
  return JSON.stringify(obj)
}

/** CleverScript: prefer bare `JSON.parse` (Rust builtin). `globalThis.JSON` is mirrored too. */
export function parseJson(text) {
  try {
    if (!text || text.length === 0) return null
    return JSON.parse(text)
  } catch (e) {
    return null
  }
}
