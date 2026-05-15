use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Ident, LitStr, Token, parse::Parse, parse_macro_input};

enum FsrInvalidateInput {
    DepKey { store: Expr, dep: Expr },
    Route { store: Expr, route: LitStr },
}

impl Parse for FsrInvalidateInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let store: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        // Check if next token is `route` followed by `=`
        if input.peek(Ident) {
            let fork = input.fork();
            let ident: Ident = fork.parse()?;
            if ident == "route" && fork.peek(Token![=]) {
                // Consume the real tokens
                input.parse::<Ident>()?; // "route"
                input.parse::<Token![=]>()?;
                let route: LitStr = input.parse()?;
                return Ok(Self::Route { store, route });
            }
        }

        let dep: Expr = input.parse()?;
        Ok(Self::DepKey { store, dep })
    }
}

/// Proc-macro implementation for `fsr_invalidate!`.
pub fn expand(input: TokenStream) -> TokenStream {
    match parse_macro_input!(input as FsrInvalidateInput) {
        FsrInvalidateInput::DepKey { store, dep } => {
            let expanded: TokenStream2 = quote! {
                {
                    let __dep = #dep;
                    let __key = ::std::format!(
                        "{}:{}={}",
                        __dep.table,
                        __dep.column,
                        __dep.value
                    );
                    #store.invalidate_dep_key(&__key)
                        .await
                        .map_err(|_| ::pilcrow_web::AppError::Internal)?;
                }
            };
            expanded.into()
        }
        FsrInvalidateInput::Route { store, route } => {
            let expanded: TokenStream2 = quote! {
                {
                    #store.invalidate_route(#route)
                        .await
                        .map_err(|_| ::pilcrow_web::AppError::Internal)?;
                }
            };
            expanded.into()
        }
    }
}
