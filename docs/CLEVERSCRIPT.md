# CleverScript — Language Specification

CleverScript is the JS-like language executed by **relay-runtime** for client hooks. It is intentionally a **small, well-specified subset** designed for JSX rendering—not full JavaScript. This document defines what is supported and what is explicitly out of scope.

**Design principle:** Keep the surface small and predictable. Fail fast on unsupported constructs. Prefer host-provided APIs over runtime emulation.

---

## 1. Supported — Must Have

### 1.1 Literals

- `null`, `true`, `false`
- Numbers: `0`, `1.5`, `-3`
- Strings: `"hello"`, `'world'`

### 1.2 Objects

- Object literals with **string keys only**: `{ a: 1, b: "x" }`
- Member access: `obj.key`, `obj["key"]`, `obj?.prop`, `obj?.[i]`
- No computed keys in literals

### 1.3 Arrays

- Array literals: `[]`, `[1, 2, 3]`
- Length: `arr.length`
- Index access: `arr[i]` (where `i` is a numeric variable or literal)
- Building arrays: use `concat` or host-provided helpers (e.g. `arrayConcat(a, b)`); **no** `arr.push()` in core

### 1.4 Operators

- Arithmetic: `+`, `-`, `*`, `/`
- Comparison: `===`, `!==`, `<`, `<=`, `>`, `>=`
- Logical: `&&`, `||`, `!`
- Unary: `!`, `-` (negation), `++`, `--` (postfix)
- Nullish coalescing: `??` — `a ?? b` returns `b` when `a` is `null`
- Optional chaining: `?.` — `obj?.prop`, `obj?.[i]` returns `null` when `obj` is `null`

### 1.5 Control Flow

- `if` / `else if` / `else`
- `for (let i = 0; i < n; i++)` — classic for loops
- `while (cond) { ... }`

### 1.6 Functions

- Arrow functions: `() => x`, `(a, b) => a + b`
- `async` for top-level component when needed
- `return` statements

### 1.7 Imports

- `import x from 'source'` — default imports from host modules
- Host provides: theme, styler, relayRuntime, etc.

### 1.8 React Hooks

- `useState(initial)` — supported
- Hooks must be called in a **static order** (no hooks in conditionals or loops)

### 1.9 JSX

- Elements: `<div>`, `<span>`, etc.
- Fragments: `<>...</>` or `<Fragment>...</>`
- Children: `{expr}` expressions
- Props: `className`, `key`, `data-*`, etc.
- No JSX spread: `<Foo {...props}` — pass props explicitly

---

## 2. Explicitly Out of Scope

### 2.1 Parser / Language Surface

| Construct | Status |
|-----------|--------|
| `==`, `!=` (loose equality) | Not supported — use `===`, `!==` only |
| Template literals `` `${x}` `` | Not supported (or minimal only) |
| Object/array spread `{...a}`, `[...a]` | Not supported |
| Complex destructuring | Not supported — simple `const {a} = obj` only |
| `typeof`, `void`, `delete` | Not supported |

### 2.2 Control Flow

| Construct | Status |
|-----------|--------|
| `for...of`, `for...in` | Not supported |
| `do...while`, `switch` | Not supported — use `while` or `if/else` |
| `try/catch` | Not supported |

### 2.3 Objects

| Construct | Status |
|-----------|--------|
| Computed keys `[expr]` in object literals | Not supported |
| Prototype chain, `__proto__` | Not supported |
| `this` | Not supported |

### 2.4 Functions and Modules

| Construct | Status |
|-----------|--------|
| Dynamic `import()` in hot path | Avoid — complicates hook analysis |
| Named exports | Single default export per module |
| `arguments` object | Not supported |

### 2.5 Arrays

| Construct | Status |
|-----------|--------|
| `arr.push()`, `arr.pop()`, etc. | Not supported — use `concat` or host helpers |
| Mutable array methods | Not supported — prefer immutable patterns |

### 2.6 JSX

| Construct | Status |
|-----------|--------|
| JSX spread `<Foo {...props}` | Not supported |
| Complex expressions in attributes | Limit to literals, identifiers, `obj.key` |

---

## 3. Host Boundary

- **Heavy APIs from host:** theme, styler, relayRuntime, fetch, etc.
- **JSON in, JSON out:** theme/config as JSON; vnodes as JSON
- **Fail fast:** unsupported syntax or runtime error → clear error, no silent fallback

---

## 4. Hooks and Analysis

- **Static hook count:** analysis must know exact number of hooks per component
- **No hooks in conditionals or loops:** hooks at top level only
- **Dynamic imports:** resolve before hook execution so analysis sees all hooks

---

## 5. Documentation and Testing

- **Supported subset doc:** this document
- **Parse-first validation:** reject unsupported syntax at parse time with clear errors
- **Golden/snapshot tests:** parse + run for representative hooks to catch regressions

---

## 6. Summary Table

| Category | Supported | Not Supported |
|----------|-----------|---------------|
| Equality | `===`, `!==` | `==`, `!=` |
| Optional | `?.`, `??` | — |
| Spread | — | `{...}`, `[...]` |
| Unary | `!`, `-`, `++`, `--` (postfix) | `typeof`, `void` |
| Loops | `for (;;)`, `while` | `for...of`, `for...in`, `do...while` |
| Objects | string keys, `obj.key` | computed keys, prototype, `this` |
| Arrays | `length`, `[i]`, `concat` | `push`, `pop`, mutable methods |
| JSX | elements, fragments, `{expr}` | spread `{...props}` |

---

## Related

- [POLICY.md](POLICY.md) — no legacy, no transpiler
- [PLAN.md](PLAN.md) — strategy and language philosophy
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md) — web embedding
