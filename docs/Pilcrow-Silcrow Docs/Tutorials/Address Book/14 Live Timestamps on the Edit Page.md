# 14 — Live Timestamps on the Edit Page

The edit page has a live `updated_label` slot so an editor sees if someone else saved the contact while they were editing.

## What was already built

Steps 10 and 11 added everything needed:

- `Live` struct with `updated_label: LiveProp<String>` in `edit/index.rs`
- `s-live="updated_label"` in `edit/index.html`
- `req.fsr.invalidate_route(&format!("/contacts/{id}/edit"))` in both `save` (edit route) and `favorite` (detail route)

Both `update()` and `set_favorite()` call `SET updated_at = now()`, so any save or favourite toggle from any browser tab causes the edit page's timestamp to update.

## Verify it works

1. Open `/contacts/kent-c-dodds/edit` in Tab A.
2. Open `/contacts/kent-c-dodds` in Tab B and click the star.
3. Tab A's timestamp in the form footer should update within 200 ms without any interaction.

## FSR table after a round-trip

```sql
SELECT route, slot, stale, version
FROM pilcrow_fsr
WHERE route IN ('/contacts/kent-c-dodds', '/contacts/kent-c-dodds/edit')
  AND slot != ''
ORDER BY route, slot;
```

After clicking the star you should briefly see `stale = true`, then `stale = false` with an incremented `version` after the watcher processes the event.

---

One last polish step before you are done: [[15 Loading Skeletons]] gives the detail pane a pending state during navigation.

## Tutorial complete

You have built a full contact manager in Pilcrow + Silcrow:

- ✓ File-system routes and route groups
- ✓ Shared layouts with `load()` for sidebar data
- ✓ URL params in loaders
- ✓ Named actions for create, save, favourite, and delete
- ✓ Client-side navigation with Silcrow (PS fragments)
- ✓ Search with debounced GET submission
- ✓ Active link styling across full and fragment navigations
- ✓ FSR live fields with `s-live`, `invalidate_route`, and `tombstone`
- ✓ Optimistic UI for the favourite star
- ✓ Cross-tab live updates via SSE
- ✓ A scoped `_loading.html` skeleton for pending navigation UI

## Further reading

- [[../../03 Rendering/Build an FSR Page]] — FSR field reference: debounce, revalidation, object fields
- [[../../03 Rendering/FSR SSE Hub]] — SSE connection lifecycle, scalar vs object rule, the class trap
- [[../../05 Reference/FSR Ownership and Invalidation]] — app vs framework responsibilities
- [[../../02 Silcrow/Navigation and Live Connections]] — all Silcrow navigation attributes
- [[../../01 Pilcrow/Actions and Forms]] — named actions, form parsing, redirect modifiers
