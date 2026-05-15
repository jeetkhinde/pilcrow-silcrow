use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, parse::Parse, parse_macro_input};

struct InvalidateInput {
    dep_key_expr: Expr,
}

impl Parse for InvalidateInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            dep_key_expr: input.parse()?,
        })
    }
}

/// Invalidate a dep key from inside a `#[pilcrow::handler(live)]` handler.
///
/// Marks all `pilcrow_cache` rows whose `depends_on` contains the key as stale,
/// then broadcasts an `InvalidationEvent` to all active SSE subscribers.
///
/// # Example
///
/// ```rust,ignore
/// #[pilcrow::handler(live)]
/// async fn update_ticket(params: TicketParams) -> TicketProps {
///     // ... update DB ...
///     // dep! args mean: table, column, runtime value.
///     pilcrow::invalidate!(dep!(tickets, id, params.id));
///     TicketProps { /* ... */ }
/// }
/// ```
pub fn expand(input: TokenStream) -> TokenStream {
    let InvalidateInput { dep_key_expr } = parse_macro_input!(input as InvalidateInput);
    let expanded: TokenStream2 = quote! {
        {
            let __dep_key_val = #dep_key_expr;
            let __affected_routes = __pilcrow_live_store
                .invalidate_dep_key(&__dep_key_val)
                .await
                .map_err(::pilcrow_web::AppError::from)?;
            __pilcrow_live_broadcast.send(
                ::runtime::live_props::InvalidationEvent {
                    dep_key: __dep_key_val.as_str().to_string(),
                    affected_routes: __affected_routes,
                }
            );
        }
    };
    expanded.into()
}
