# Baked Pages POC: Patchable Ticket Status Slot

This is a proof-of-concept design note, not implemented behavior.

## Goal

Prove one small vertical slice of the Baked Page model:

- One lazy baked route: `/tickets/:id`
- One explicit text slot: `ticket_status`
- One explicit dependency key: `TicketStatus:ticket_id=123`
- One manual or generated slot recompute function: `recompute_ticket_status_slot(ticket_id) -> String`
- Filesystem baked HTML
- Marker-boundary patching with temp-file write and atomic rename
- Next `GET /tickets/123` serves patched baked HTML without full SSR/load

The proof should establish the serving-path shift:

- Request path: read baked HTML.
- Update path: recompute and patch baked HTML outside the request.

Baked Page is the main model for this POC. ISR and SSG are not the conceptual center; they are future compatibility policies that can later map into Baked timing and freshness behavior.

## POC Scope

Included:

- Route: `/tickets/:id`, demonstrated with `/tickets/123`.
- Slot: `ticket_status`, text only.
- Dependency key: `TicketStatus:ticket_id=123`.
- Recompute function: `recompute_ticket_status_slot(ticket_id) -> String`.
- Storage: baked HTML on filesystem.
- Manifest: per-page JSON.
- Reverse index: JSON or memory-only for the POC.
- Patching: only inside explicit Pilcrow-owned slot boundaries.
- Write safety: write temp file, then atomic rename over the old baked HTML file.

Explicitly deferred:

- Broad cache redesign.
- ISR/SSG redesign.
- Browser/SSE live patching.
- SQL dependency inference.
- Final public API design.
- Typed `Dep`, `BakedSlot<T>`, or `BakedHtml`.
- Closure serialization from `load()`.
- Generic cache providers.
- Arbitrary HTML regex patching.
- Multi-process storage and coordination.

## Request Flow

First request:

```text
GET /tickets/123
-> baked HTML missing
-> run normal SSR/load once
-> render HTML containing the owned ticket_status slot boundary
-> write baked HTML to filesystem
-> write page manifest for /tickets/123
-> add reverse-index entry for TicketStatus:ticket_id=123
-> serve the rendered response
```

Second and later requests:

```text
GET /tickets/123
-> baked HTML exists
-> read baked HTML from filesystem
-> serve it directly
-> do not run normal SSR/load
```

The proof must log or otherwise prove when SSR/load runs so the first request and baked-file hit are distinguishable.

## Update Flow

When ticket `123` changes status:

```text
ticket 123 status changes
-> app emits dependency key: TicketStatus:ticket_id=123
-> Pilcrow finds all page/slot targets depending on that key
-> call recompute_ticket_status_slot("123")
-> validate exactly one ticket_status start/end marker pair in each baked HTML file
-> patch only the content inside the owned boundary
-> write a temp HTML file
-> atomic rename over the old baked HTML file
-> next GET /tickets/123 serves the patched HTML
-> normal SSR/load does not run for that GET
```

If another baked page also depends on `TicketStatus:ticket_id=123`, the same dependency event must patch both baked files.

## Marker And Manifest Contract

The POC must use this explicit Pilcrow-owned marker boundary:

```html
<!--pilcrow-slot:start ticket_status kind=text-->
<span data-pilcrow-slot="ticket_status">Open</span>
<!--pilcrow-slot:end ticket_status-->
```

Only the content inside this owned boundary may be patched. The outer start/end comments are the durable filesystem patch boundary. The visible `data-pilcrow-slot` attribute is the stable slot identity that future browser-side patching can reuse, but browser patching is out of scope for this POC.

Minimum per-page manifest JSON:

```json
{
  "page_key": "/tickets/123",
  "route": "/tickets/:id",
  "params": { "id": "123" },
  "slots": {
    "ticket_status": {
      "kind": "text",
      "depends_on": ["TicketStatus:ticket_id=123"]
    }
  }
}
```

Minimum reverse index, as JSON or memory-only:

```json
{
  "TicketStatus:ticket_id=123": [
    { "page_key": "/tickets/123", "slot": "ticket_status" }
  ]
}
```

## Validation And Fallback

Patch validation rules:

- The baked HTML must contain exactly one matching start marker for `ticket_status`.
- The baked HTML must contain exactly one matching end marker for `ticket_status`.
- The start marker must appear before the end marker.
- The slot kind must be `text`.
- Patching must not search or mutate arbitrary HTML outside the owned boundary.

Default POC fallback:

- If validation fails, perform a full-page rebake for that page and rewrite the baked HTML, manifest, and reverse-index data.
- If full-page rebake also fails, return or log an internal error for the dependency update.

This keeps the proof robust without introducing broad cache invalidation semantics.

## Success Criteria

1. First `GET /tickets/123` logs or proves SSR/load ran.
2. Baked HTML file is written to disk.
3. Second `GET /tickets/123` serves the baked file without SSR/load.
4. A test action or function changes ticket status and emits `TicketStatus:ticket_id=123`.
5. Pilcrow patches only the `ticket_status` slot in the baked file.
6. Next `GET /tickets/123` returns the new status without full SSR/load.
7. If another page also depends on `TicketStatus:ticket_id=123`, both baked files are patched from the same dependency event.

## Deferred Generalization

After this vertical slice works, generalize in this order:

1. Replace hard-coded dependency strings with typed `Dep`.
2. Replace manual slot markers with generated `BakedSlot<T>` text slots.
3. Add `BakedHtml` for owned HTML fragment slots.
4. Add build-time, startup, background warm, and lazy bake policies.
5. Map existing ISR/SSG behavior into Baked timing and freshness policies.
6. Add browser-side slot patching through SSE using the same slot identity.
7. Add durable multi-process storage and coordination.
