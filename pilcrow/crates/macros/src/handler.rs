use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, PatType, parse_macro_input, visit::Visit};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let is_live = is_live_attr(TokenStream2::from(attr));
    let func = parse_macro_input!(item as ItemFn);

    let uses_client = body_uses_client(&func);

    let mut extra_params: Vec<TokenStream2> = vec![];

    if uses_client {
        extra_params.push(quote! {
            __pilcrow_client: ::pilcrow_client::PilcrowClient
        });
    }

    // When `#[handler(live)]` is used, inject matched path + live-props extensions.
    if is_live {
        extra_params.push(quote! {
            __pilcrow_matched_path: ::axum::extract::MatchedPath
        });
        extra_params.push(quote! {
            ::axum::Extension(__pilcrow_live_store): ::axum::Extension<
                ::std::sync::Arc<::runtime::live_props::LivePageStore>
            >
        });
        extra_params.push(quote! {
            ::axum::Extension(__pilcrow_live_broadcast): ::axum::Extension<
                ::runtime::live_props::LiveBroadcast
            >
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

    // Generate the live-props write code inserted after the handler closure returns.
    let live_write = if is_live {
        quote! {
            {
                use ::runtime::live_props::LivePropExtract as _;
                let __live_fields = __r.live_fields();
                if !__live_fields.is_empty() {
                    let __route = __pilcrow_matched_path.as_str().to_string();
                    let __params = ::serde_json::json!({});
                    let __promote_after = __live_fields.iter().find_map(|f| f.promote_after);
                    let _ = __pilcrow_live_store
                        .write_live_fields(&__route, &__params, &__live_fields)
                        .await;
                    let __just_promoted = __pilcrow_live_store
                        .increment_hit(&__route, __promote_after)
                        .await
                        .unwrap_or(false);
                    let _ = __just_promoted;
                }
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #vis async fn #sig_ident(#(#all_params),*) -> ::pilcrow_web::AppResult<::axum::response::Response> {
            use ::axum::response::IntoResponse;
            #client_binding
            let __result = (|| async move {
                #body
            })().await;
            match __result {
                Ok(__r) => {
                    #live_write
                    Ok(__r.into_response())
                }
                Err(e) => Err(e),
            }
        }
    };

    expanded.into()
}

// ── Attribute parsing ─────────────────────────────────────────────────────────

fn is_live_attr(attr: TokenStream2) -> bool {
    attr.to_string().trim() == "live"
}

// ── Client detection ──────────────────────────────────────────────────────────

struct ClientVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ClientVisitor {
    fn visit_ident(&mut self, ident: &'ast Ident) {
        if ident == "client" {
            self.found = true;
        }
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
    fn is_live_attr_parses_live_keyword() {
        let ts: proc_macro2::TokenStream = quote! { live };
        assert!(is_live_attr(ts));
    }

    #[test]
    fn is_live_attr_empty_is_false() {
        assert!(!is_live_attr(proc_macro2::TokenStream::new()));
    }
}
