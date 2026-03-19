/**
 * Test hook for CleverScript features: while, ??, ?.
 */
export default function CleverScriptFeatures() {
  // ?? nullish coalescing
  const a = null ?? "default";
  const b = 0 ?? "fallback"; // 0 is not null, so stays 0
  const c = "" ?? "empty-fallback"; // "" is not null, so stays ""

  // ?. optional chaining
  const obj = { x: 1, nested: { y: 2 } };
  const d = obj?.x;
  const e = obj?.nested?.y;
  const f = null?.prop ?? "null"; // optional chain returns null, ?? yields "null"

  // while loop
  let sum = 0;
  let i = 0;
  while (i < 5) {
    sum = sum + i;
    i++;
  }

  return (
    <div className="cleverscript-features">
      <div>?? nullish: a={a}, b={b}, c={c}</div>
      <div>?. optional: d={d}, e={e}, f={f}</div>
      <div>while sum(0..4)={sum}</div>
    </div>
  );
}
