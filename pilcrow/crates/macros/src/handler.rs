use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, PatType, parse_macro_input, visit::Visit};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        let msg = format!(
            "#[handler({attr})] is no longer supported — `#[handler]` takes no arguments. \
             The `live` argument was removed; use FSR `LiveProp<T>` fields instead."
        );
        return quote! { compile_error!(#msg); }.into();
    }
    let func = parse_macro_input!(item as ItemFn);

    let uses_client = body_uses_client(&func);

    let mut extra_params: Vec<TokenStream2> = vec![];

    if uses_client {
        extra_params.push(quote! {
            __pilcrow_client: ::pilcrow_client::PilcrowClient
        });
    }

    // Rewrite known params
    let mut rewritten: Vec<TokenStream2> = vec![];

    for param in &func.sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = param
            && let Pat::Ident(ident) = pat.as_ref()
        {
            let name = ident.ident.to_string();
            match name.as_str() {
                "form" => {
                    rewritten.push(quote! {
                        ::axum::Form(#pat): ::axum::Form<#ty>
                    });
                    continue;
                }
                "json" => {
                    rewritten.push(quote! {
                        ::axum::Json(#pat): ::axum::Json<#ty>
                    });
                    continue;
                }
                "path" => {
                    rewritten.push(quote! {
                        ::axum::extract::Path(#pat): ::axum::extract::Path<#ty>
                    });
                    continue;
                }
                _ => {}
            }
        }
        rewritten.push(quote! { #param });
    }

    let all_params = extra_params.iter().chain(rewritten.iter());

    // Inject `let client = __pilcrow_client;` at top of body if needed
    let client_binding = if uses_client {
        quote! { let client = __pilcrow_client; }
    } else {
        quote! {}
    };

    let vis = &func.vis;
    let sig_ident = &func.sig.ident;
    let body = &func.block;

    let expanded = quote! {
        #vis async fn #sig_ident(#(#all_params),*) -> ::pilcrow_web::AppResult<::axum::response::Response> {
            use ::axum::response::IntoResponse;
            #client_binding
            let __result = (|| async move {
                #body
            })().await;
            match __result {
                Ok(__r) => {
                    Ok(__r.into_response())
                }
                Err(e) => Err(e),
            }
        }
    };

    expanded.into()
}

// ── Client detection ──────────────────────────────────────────────────────────

struct ClientVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ClientVisitor {
    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        if expr.path.is_ident("client") {
            self.found = true;
        }
        syn::visit::visit_expr_path(self, expr);
    }
}

fn body_uses_client(func: &ItemFn) -> bool {
    let mut visitor = ClientVisitor { found: false };
    visitor.visit_block(&func.block);
    visitor.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_body_uses_client_true() {
        let func: ItemFn = parse_quote! {
            fn my_route() {
                client.get("/api/users").await;
            }
        };
        assert!(body_uses_client(&func));
    }

    #[test]
    fn test_body_uses_client_false() {
        let func: ItemFn = parse_quote! {
            fn my_route() {
                let x = 1 + 1;
            }
        };
        assert!(!body_uses_client(&func));
    }

    #[test]
    fn test_body_uses_client_no_false_positive_struct_field() {
        // `db.client` is a field access, not a bare `client` expression — must not trigger
        let func: ItemFn = parse_quote! {
            fn my_route() {
                let c = db.client;
            }
        };
        assert!(!body_uses_client(&func));
    }

    #[test]
    fn test_body_uses_client_no_false_positive_local_var() {
        // `let client = ...` declares a local; the right-hand side has no bare `client` expr
        let func: ItemFn = parse_quote! {
            fn my_route() {
                let client = some_fn();
            }
        };
        assert!(!body_uses_client(&func));
    }

}
