/// Implemented by structs that represent a keyed list row with live-updatable fields.
///
/// Derive via `#[derive(PilcrowListRow)]` and annotate fields:
/// - `#[pilcrow(key)]`  — exactly one field; value serialised to `String` as the row key.
/// - `#[pilcrow(live)]` — zero or more fields whose current values are included in
///   `list-patch` SSE events when this row changes.
///
/// ```rust,ignore
/// #[derive(PilcrowListRow)]
/// pub struct TicketRow {
///     #[pilcrow(key)]
///     pub id: i64,
///     #[pilcrow(live)]
///     pub status: String,
///     pub title: String,  // static — rendered once, not live-updated
/// }
/// ```
pub trait ListRow: Send + Sync + 'static {
    /// Unique string key for this row. Used as `data-pilcrow-key` on the DOM element
    /// and as the `key` field in `list-patch` SSE events.
    fn pilcrow_key(&self) -> String;

    /// Live-updatable field values as `(field_name, json_value)` pairs.
    /// Only fields annotated `#[pilcrow(live)]` are included.
    fn pilcrow_live_fields(&self) -> Vec<(&'static str, serde_json::Value)>;
}
