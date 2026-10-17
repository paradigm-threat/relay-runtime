/**
 * Tip-local string helpers — avoid `@relay/util.underscoresToSpaces` / String#split
 * (missing or unsafe on some park hosts; caused blank DirNav labels).
 */

/** Replace `_` with space (CleverScript-safe char loop). */
export function underscoresToSpaces(raw) {
  if (typeof raw !== 'string') return ''
  let out = ''
  for (let i = 0; i < raw.length; i++) {
    const ch = raw.charAt(i)
    if (ch === '_') {
      out += ' '
    } else {
      out += ch
    }
  }
  return out
}
