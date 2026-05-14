# LiveProp Column Mapping — Design Spec
_Date: 2026-05-13_

## Overview

Add `#[pilcrow::column("sql_col")]` support to `LiveProp<T>` fields in `live.rs` files so a Rust field name can differ from its SQL column name. The generated `from_row()` and the FSR watcher both use the column name for DB lookups; the Rust field name is still used for HTML `s-live` slots.

### Motivating example

```rust
// pages/tickets/[id]/live.rs
pub struct Live {
    #[pilcrow::column("status")]
    pub ticket_status: LiveProp<String>,   // HTML slot: s-live="ticket_status"
                                           // DB column: "status"
}
```

SQL query returns `{ "status": "Open" }`. Without this attribute, both `from_row()` and the watcher look up `"ticket_status"` and silently get null.

---

## Current State

| Location | What exists | Gap |
|----------|-------------|-----|
| `routekit/src/fsr.rs` | `parse_field_defaults` parses `#[pilcrow::column]`; `generate_from_row_impl` uses `column_name.as_ref().unwrap_or(name)` for `row.get(...)` | Complete. One test exists. |
| `runtime/src/fsr/watcher.rs` | `re_execute_query` uses `m.get(&slot.slot)` | `slot.slot` is the field name; SQL column name is not stored → silent null when names differ |
| `runtime/src/fsr/store.rs` | `upsert_slot` exists but is never called | `StaleSlot` has no `column_name` field; `pilcrow_fsr` table has no `column_name` column |
| `crates/macros/src/live_props_derive.rs` | `find_str_attr("column")` matches bare `#[column("x")]` | No test for this in `tests/live_props_derive.rs` |

---

## Design

### 1. DB schema — `pilcrow_fsr` table

Add a nullable `column_name TEXT` column:

```sql
ALTER TABLE pilcrow_fsr ADD COLUMN column_name TEXT;
```

Migration file: `demo/migrations/<timestamp>_fsr_column_name.sql`

Default is NULL, meaning "use slot name as column name".

### 2. `StaleSlot` and `upsert_slot`

**`store.rs`** — add `column_name: Option<String>` to `StaleSlot`.

**`store.rs`** — add `column_name: Option<&str>` param to `upsert_slot`:
```sql
INSERT INTO pilcrow_fsr (route, slot, column_name, query, ...)
VALUES ($1, $2, $3, $4, ...)
ON CONFLICT (route, slot) DO UPDATE SET column_name = EXCLUDED.column_name, ...
```

### 3. Watcher — use column_name for value extraction

**`watcher.rs::re_execute_query`** — resolve the lookup key before the HashMap access:
```rust
let col_key = slot.column_name.as_deref().unwrap_or(&slot.slot);
let value = row
    .as_ref()
    .and_then(|m| m.get(col_key))
    .cloned()
    .unwrap_or(serde_json::Value::Null);
```

### 4. Codegen — wire column_name into slot registration

**`routekit/src/fsr.rs`** — `LiveField` already has `column_name: Option<String>`. No changes needed here.

**`routekit/src/templating/codegen/types.rs`** — change `fsr_live_fields_map` type from `HashMap<String, Vec<String>>` to `HashMap<String, Vec<(String, Option<String>)>>` carrying `(field_name, column_name)` pairs.

**`routekit/src/templating/codegen/templates.rs`** — populate the map with `(field.name.clone(), field.column_name.clone())` tuples.

**`routekit/src/templating/codegen/app_module.rs`** — un-prefix `_fsr_live_fields_map` and emit `fsr_store.upsert_slot(...)` calls with `column_name` arg in the FSR route handler.

### 5. Tests for renamed SQL columns

**`routekit/src/fsr.rs`** — add a multi-field test where one field has `#[pilcrow::column]` and one does not; verify each uses the right key in the generated `from_row()`.

**`runtime/tests/live_props_derive.rs`** — add a test with `#[pilcrow::column("status")]` on a `PilcrowProps` struct; verify `field_name` in `LiveFieldData` uses the Rust field name (since HTML slots are driven by field name, not column name, for the old live-props path).

---

## Key invariant

- **HTML slot name** always matches the Rust field name (`s-live="ticket_status"`)
- **DB lookup key** uses the column name override if present, else falls back to field name (`row.get("status")` or `row.get("ticket_status")`)
- Template slot validation (`validate_live_template_slots`) already uses field names for the HTML side — no change needed

---

## Files touched

| File | Change |
|------|--------|
| `demo/migrations/<ts>_fsr_column_name.sql` | new — ADD COLUMN |
| `runtime/src/fsr/store.rs` | `StaleSlot` + `upsert_slot` + `fetch_stale_slots` SELECT |
| `runtime/src/fsr/watcher.rs` | use `column_name` for value extraction |
| `routekit/src/templating/codegen/types.rs` | `fsr_live_fields_map` type change |
| `routekit/src/templating/codegen/templates.rs` | populate with column names |
| `routekit/src/templating/codegen/app_module.rs` | emit `upsert_slot` with column_name |
| `routekit/src/fsr.rs` | new multi-field column test |
| `runtime/tests/live_props_derive.rs` | new column mapping test |
