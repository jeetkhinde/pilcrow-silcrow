# FSR → Silcrow Atom Bridge: Non-Scalar `LiveProp<T>`

**Date:** 2026-05-13
**Status:** Approved

---

## What this solves

`LiveProp<T>` currently only works for scalar types (`String`, `i64`, etc.) because the FSR
inline JS patch handler always writes `textContent`. There is no path for an object-valued
`LiveProp<TicketBadge>` to drive multiple DOM bindings reactively on SSE update.

This spec adds a bridge so that when the FSR watcher pushes an object-valued slot over SSE,
it lands in a Silcrow `scopeAtom` at `"fsr.<slot_name>"`. Existing `s-use` and `:text` / `:class`
bindings on that scope then update automatically — no new JS concepts, no Silcrow changes.

---

## Scope

- One change to the FSR inline JS in `app_module.rs`
- One type constraint on `LiveProp<T>` (`T: Serialize + DeserializeOwned`)
- `from_row()` codegen uses `serde_json::from_value` for struct fields
- No changes to: `pilcrow_fsr` DB schema, SSE event shape, `PilcrowLive` trait, Silcrow

---

## Core mechanism

The FSR inline JS handler in `app_module.rs` is the only runtime change. Currently it always
writes `textContent`. The new version branches on value type:

```javascript
Object.keys(d).forEach(function(k) {
  var v = d[k];
  if (v !== null && typeof v === 'object') {
    // Object value → publish to fsr.* scopeAtom
    if (window.Silcrow && window.Silcrow.publish) {
      window.Silcrow.publish('fsr.' + k, v);
    }
  } else {
    // Scalar value → existing s-live textContent patch (unchanged)
    document.querySelectorAll('[s-live="' + k + '"]').forEach(function(n) {
      n.textContent = v == null ? '' : String(v);
    });
  }
});
```

`window.Silcrow.publish(scope, data)` writes to a named `scopeAtom`. Silcrow's existing
`s-use`, `:text`, `:class`, and other binding directives read from these atoms — no Silcrow
changes are needed.

---

## Developer surface

### Scalar (unchanged)

```rust
// live.rs
pub ticket_status: LiveProp<String>,
```

```html
<span s-live="ticket_status">{{ live.ticket_status.value }}</span>
```

### Object (new)

```rust
// live.rs
pub ticket_badge: LiveProp<TicketBadge>,
// TicketBadge must be: Deserialize + Serialize

// Elsewhere (e.g. models.rs or live.rs itself)
#[derive(Serialize, Deserialize)]
pub struct TicketBadge {
    pub label: String,
    pub color: String,
}
```

```html
<!-- Initial render: SSR-baked from Live struct -->
<div s-use="fsr.ticket_badge">
  <span :text="label">{{ live.ticket_badge.value.label }}</span>
  <span :class="color">{{ live.ticket_badge.value.color }}</span>
</div>
```

On SSE patch the watcher re-executes the query, pushes `{ "ticket_badge": { "label": "...", "color": "..." } }`,
and the JS handler calls `window.Silcrow.publish('fsr.ticket_badge', { label, color })`. Silcrow
re-spreads the atom onto all `s-use="fsr.ticket_badge"` bindings in the current route.

---

## Query convention for object fields

`from_row()` pulls the value for a field by its column name (from `LiveFieldRegistration::column_name`
or the field name as fallback). For an object field the query must return that column as a JSON value.
The standard pattern is `json_build_object`:

```rust
impl Live {
    pub fn query(params: &serde_json::Map<String, Value>) -> LiveQuery {
        live_query!(
            "SELECT
               json_build_object('label', badge_label, 'color', badge_color) AS ticket_badge
             FROM tickets WHERE id = $1",
            params["id"]
        )
    }
}
```

Rationale: The `row_to_json` wrapper that `extract_live_from_parts` and the watcher both use
wraps the entire row. Each column in that row becomes a key. If a column is already a JSON
object (returned from `json_build_object` or a `jsonb` column), `row_to_json` nests it
correctly. No special handling needed.

---

## `from_row()` codegen

For scalar fields the generated code is (as today):

```rust
ticket_status: LiveProp {
    value: row.get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default(),
    ...
}
```

For struct fields (`LiveProp<T>` where `T: DeserializeOwned`), the generated code uses
`serde_json::from_value`:

```rust
ticket_badge: LiveProp {
    value: row.get("ticket_badge")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default(),
    ...
}
```

Codegen detects object fields by checking whether `T` is a primitive or a named struct type.
The simplest heuristic: if `T` is not one of `String | i8 | i16 | i32 | i64 | u8 | u16 | u32 | u64 | f32 | f64 | bool`, use `from_value`. The `T: DeserializeOwned` bound is always added — it's a no-op for scalars.

---

## Type constraint

`LiveProp<T>` gains a bound: `T: Serialize + DeserializeOwned + Default`.

- `Serialize` — needed for initial SSR value serialization (already injected by codegen `#[derive(Serialize)]` on the Live struct)
- `DeserializeOwned` — needed by `from_row()` for object deserialization
- `Default` — already required for the fallback `.unwrap_or_default()`

---

## Data flow (unchanged outside the JS handler)

```
Dep change
→ pilcrow::invalidate!(dep!(tickets, id, 123))
→ UPDATE pilcrow_fsr SET stale=TRUE WHERE depends_on @> ARRAY['tickets:id=123']
→ Watcher polls, sees stale=TRUE
→ Re-executes stored query (same SQL, same params)
→ row_to_json returns { "ticket_badge": { "label": "...", "color": "..." } }
→ Watcher pushes SSE: { "ticket_badge": { "label": "...", "color": "..." } }
→ JS handler: typeof v === 'object' → window.Silcrow.publish('fsr.ticket_badge', v)
→ Silcrow spreads atom onto s-use="fsr.ticket_badge" bindings
→ :text="label" and :class="color" elements update
```

Scalar slots go through the existing `s-live` textContent path, unchanged.

---

## What is NOT in scope

- Nested `LiveProp<Vec<T>>` (list fields) — handled separately by the existing list-row slot naming scheme
- Auto-building JSON from multiple query columns without `json_build_object` — developer is responsible for the query shape
- Silcrow changes — the bridge uses only the public `window.Silcrow.publish()` API

---

## Files changed

| File | Change |
|------|--------|
| `pilcrow/crates/routekit/src/templating/codegen/app_module.rs` | FSR inline JS: branch on `typeof v === 'object'`, call `Silcrow.publish` |
| `pilcrow/crates/runtime/src/fsr/live_prop.rs` | Add `T: DeserializeOwned` bound; update `from_value` deserialization path |
| `pilcrow/crates/routekit/src/fsr.rs` | `from_row()` codegen: emit `from_value` path for non-primitive T |
