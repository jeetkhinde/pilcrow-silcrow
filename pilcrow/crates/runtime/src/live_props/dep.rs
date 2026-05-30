/// Constructs a [`DependencyKey`](crate::deferred::DependencyKey) from
/// table name, column name, and a runtime value expression.
///
/// Macro arguments mean: table, column, runtime value.
///
/// The generated key has the shape `"table:column=value"`, which matches the
/// `depends_on @> ARRAY['table:column=value']` Postgres invalidation query.
///
/// # Examples
///
/// ```rust,ignore
/// use runtime::dep;
///
/// let key = dep!(tickets, id, params.id);
/// assert_eq!(key.as_str(), "tickets:id=123");
///
/// let key2 = dep!(orders, order_id, "ord-456");
/// assert_eq!(key2.as_str(), "orders:order_id=ord-456");
/// ```
#[macro_export]
macro_rules! dep {
    ($table:ident, $column:ident, $value:expr) => {
        $crate::deferred::DependencyKey::new(::std::format!(
            "{}:{}={}",
            ::std::stringify!($table),
            ::std::stringify!($column),
            $value
        ))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn dep_with_numeric_literal() {
        let key = dep!(tickets, id, 123);
        assert_eq!(key.as_str(), "tickets:id=123");
    }

    #[test]
    fn dep_with_variable() {
        struct Params {
            id: u32,
        }

        let params = Params { id: 456 };
        let key = dep!(tickets, id, params.id);
        assert_eq!(key.as_str(), "tickets:id=456");
    }

    #[test]
    fn dep_with_string_value() {
        let key = dep!(orders, order_id, "ord-789");
        assert_eq!(key.as_str(), "orders:order_id=ord-789");
    }
}
