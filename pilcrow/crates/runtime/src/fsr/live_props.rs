use serde::{Deserialize, Serialize};

/// Typed dependency key. Serialises to `"table:column=value"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyKey {
    pub table: &'static str,
    pub column: &'static str,
    pub value: String,
}

impl DependencyKey {
    pub fn as_dep_string(&self) -> String {
        format!("{}:{}={}", self.table, self.column, self.value)
    }
}

impl std::fmt::Display for DependencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}={}", self.table, self.column, self.value)
    }
}

/// A field whose value is tracked, cached, and live-patched by Pilcrow FSR.
///
/// `T` must implement `serde::Serialize + serde::de::DeserializeOwned + Default`.
/// For scalar types (`String`, `i64`, `bool`, etc.) these bounds are satisfied
/// automatically. For struct fields, add the derives explicitly:
///
/// ```rust,ignore
/// #[derive(Serialize, Deserialize, Default)]
/// pub struct TicketBadge { pub label: String, pub color: String }
///
/// pub ticket_badge: LiveProp<TicketBadge>,
/// ```
///
/// On SSE patch, object values are published to the Silcrow atom `"fsr.<slot_name>"`.
/// Bind with `s-use="fsr.ticket_badge"` and `:text="label"` in the template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProp<T> {
    pub value: T,
    pub depends_on: Vec<String>, // stored as "table:column=value" strings
    pub patch_debounce: Option<u32>,
}

impl<T: Serialize + Clone> LiveProp<T> {
    pub fn new(value: T, depends_on: Vec<DependencyKey>) -> Self {
        Self {
            value,
            depends_on: depends_on.iter().map(|d| d.as_dep_string()).collect(),
            patch_debounce: None,
        }
    }

    pub fn patch_debounce(mut self, seconds: u32) -> Self {
        self.patch_debounce = Some(seconds);
        self
    }
}
