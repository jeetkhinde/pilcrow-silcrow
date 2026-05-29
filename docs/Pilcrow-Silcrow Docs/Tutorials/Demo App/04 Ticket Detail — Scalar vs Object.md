# 04 — Ticket Detail — Scalar vs Object

The ticket detail page demonstrates two `LiveProp<T>` field patterns on one page and explains when to use each.

## `pages/tickets/[id]/index.rs`

```rust
use pilcrow_web::live::*;
use serde::{Deserialize, Serialize};

/// Object live field — text + class bundled together.
/// FSR publishes this as a JSON object to the Silcrow atom "fsr.status".
/// s-use="fsr.status" spreads text → textContent and class → className.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StatusBadge {
    pub text: String,
    pub class: String,
}

/// Object live field with an extra `raw` field for select option matching.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PriorityBadge {
    pub text: String,
    pub class: String,
    pub raw: String,
}

pub struct Props {
    pub id: i64,
    pub title: String,
    pub live: Live,
}

#[pilcrow::depends_on_route(tickets, id)]
pub struct Live {
    #[pilcrow::allow_unused]
    pub status: LiveProp<StatusBadge>,

    #[pilcrow::allow_unused]
    pub priority: LiveProp<PriorityBadge>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("0");
        live_query!(
            "SELECT
               json_build_object(
                 'text',  status,
                 'class', 'badge badge-' || status
               ) AS status,
               json_build_object(
                 'text', CASE priority
                   WHEN 'normal'    THEN 'Normal'
                   WHEN 'high'      THEN 'High'
                   WHEN 'very_high' THEN 'Very High'
                   WHEN 'critical'  THEN 'Critical'
                   ELSE initcap(priority) END,
                 'class', 'badge badge-' || priority,
                 'raw',   priority
               ) AS priority
             FROM tickets WHERE id = $1::bigint",
            id
        )
    }
}
```

## Why both fields use the object path

**The class trap:** `s-live` patches only `textContent`. If you wrote:

```html
<!-- ❌ class is frozen at SSR time — badge colour never updates -->
<span class="badge badge-{{ live.status.value }}">{{ live.status.value }}</span>
```

the badge text would update on each SSE event but the class would stay as `badge-open` even after a status change to `closed`.

Both `status` and `priority` control both the displayed text and the badge CSS class. Both use the object path so that both text and class update atomically on each SSE event.

## Template

```html
<div class="field">
  <label>Status</label>
  <!-- s-use="fsr.status" binds to the Silcrow atom "fsr.status"
       On each SSE patch: text → textContent, class → className -->
  <span class="{{ live.status.value.class }}" s-use="fsr.status">
    {{ live.status.value.text }}
  </span>
</div>

<div class="field">
  <label>Priority</label>
  <span class="{{ live.priority.value.class }}" s-use="fsr.priority">
    {{ live.priority.value.text }}
  </span>
</div>

<!-- Edit form — initial select option uses .raw for value comparison -->
<select id="sel-status">
  <option value="open"   {% if live.status.value.text   == "open"   %}selected{% endif %}>Open</option>
  <option value="closed" {% if live.status.value.text   == "closed" %}selected{% endif %}>Closed</option>
</select>

<select id="sel-priority">
  <option value="normal"    {% if live.priority.value.raw == "normal"    %}selected{% endif %}>Normal</option>
  <option value="high"      {% if live.priority.value.raw == "high"      %}selected{% endif %}>High</option>
  <option value="very_high" {% if live.priority.value.raw == "very_high" %}selected{% endif %}>Very High</option>
  <option value="critical"  {% if live.priority.value.raw == "critical"  %}selected{% endif %}>Critical</option>
</select>
```

**`#[pilcrow::allow_unused]`** — both fields use `s-use` (atom binding) rather than `s-live` (text-node binding). Without `allow_unused`, routekit would require either a `{{ live.status.value }}` text-node use or an explicit `s-live="status"` slot.

## Scalar vs object — decision rule

| Situation | Pattern |
|---|---|
| Live field drives text only | Scalar `LiveProp<String>`. Auto `s-live` from routekit. |
| Live field drives text + CSS class | Object `LiveProp<Badge { text, class }>`. `#[allow_unused]`. `s-use="fsr.slot"`. |
| Live field drives multiple attributes | Object + `s-use`. |
| Live field feeds a Silcrow atom | Object + `s-use`. |

When in doubt: if the template has `class="something-{{ live.field.value }}"` anywhere, use the object path.

---

Next: [[05 API Routes with FSR]]
