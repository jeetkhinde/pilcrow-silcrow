/// Construct an FSR `DependencyKey` from table name, column name, and a runtime value.
///
/// Macro arguments mean: table, column, runtime value.
///
/// # Examples
///
/// ```rust,ignore
/// use pilcrow::live::*;
///
/// let key = fsr_dep!(tickets, id, params.id);
/// // Produces: DependencyKey { table: "tickets", column: "id", value: "123" }
/// ```
#[macro_export]
macro_rules! fsr_dep {
    ($table:ident, $col:ident, $val:expr) => {
        $crate::fsr::DependencyKey {
            table: ::std::stringify!($table),
            column: ::std::stringify!($col),
            value: ::std::string::ToString::to_string(&$val),
        }
    };
}

/// Construct a `LiveQuery` for use in `PilcrowLive::query()` implementations.
///
/// # Examples
///
/// ```rust,ignore
/// use pilcrow::live::*;
///
/// fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
///     live_query!("SELECT status, priority FROM tickets WHERE id = $1", 42)
/// }
/// ```
#[macro_export]
macro_rules! live_query {
    ($sql:expr $(, $param:expr)*) => {
        $crate::fsr::LiveQuery {
            sql: $sql,
            params: ::std::vec![$( ::serde_json::json!($param) ),*],
        }
    };
}
