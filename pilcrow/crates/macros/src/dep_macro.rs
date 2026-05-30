use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, Ident, Token,
};

struct DepInput {
    table: Ident,
    column: Ident,
    value: Expr,
}

impl Parse for DepInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let table = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let column = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let value = input.parse::<Expr>()?;
        Ok(Self { table, column, value })
    }
}

/// Constructs a [`DependencyKey`] from table name, column name, and a runtime value.
///
/// Macro arguments mean: table, column, runtime value.
///
/// # Example
/// ```rust,ignore
/// let key = pilcrow::dep!(tickets, id, params.id);
/// // key.as_str() == "tickets:id=123"
///
/// let key2 = pilcrow::dep!(tickets, id, 456);
/// // key2.as_str() == "tickets:id=456"
/// ```
pub fn expand(input: TokenStream) -> TokenStream {
    let DepInput { table, column, value } = parse_macro_input!(input as DepInput);
    let table_str = table.to_string();
    let column_str = column.to_string();
    let expanded: TokenStream2 = quote! {
        ::runtime::deferred::DependencyKey::new(
            ::std::format!("{}:{}={}", #table_str, #column_str, #value)
        )
    };
    expanded.into()
}
