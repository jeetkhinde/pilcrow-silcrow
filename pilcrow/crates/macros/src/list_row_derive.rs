use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(s) => &s.fields,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "PilcrowListRow can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let named = match fields {
        Fields::Named(f) => &f.named,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "PilcrowListRow requires a struct with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut key_field: Option<(&syn::Ident, &syn::Type)> = None;
    let mut live_fields: Vec<(&syn::Ident, &syn::Type)> = Vec::new();
    let mut key_count = 0usize;

    for field in named {
        let ident = match field.ident.as_ref() {
            Some(i) => i,
            None => continue,
        };

        for attr in &field.attrs {
            if !attr.path().is_ident("pilcrow") {
                continue;
            }
            // Parse `#[pilcrow(key)]` or `#[pilcrow(live)]`.
            let Ok(meta_list) = attr.meta.require_list() else { continue };
            let tokens_str = meta_list.tokens.to_string();
            let trimmed = tokens_str.trim();
            if trimmed == "key" {
                key_count += 1;
                key_field = Some((ident, &field.ty));
            } else if trimmed == "live" {
                live_fields.push((ident, &field.ty));
            }
        }
    }

    if key_count > 1 {
        return syn::Error::new_spanned(
            &input.ident,
            "PilcrowListRow: exactly one field may be annotated #[pilcrow(key)]",
        )
        .to_compile_error()
        .into();
    }

    let key_impl: TokenStream2 = match key_field {
        Some((ident, _ty)) => quote! {
            fn pilcrow_key(&self) -> ::std::string::String {
                self.#ident.to_string()
            }
        },
        None => quote! {
            fn pilcrow_key(&self) -> ::std::string::String {
                ::std::string::String::new()
            }
        },
    };

    let live_impls: TokenStream2 = {
        let pushes: Vec<TokenStream2> = live_fields
            .iter()
            .map(|(ident, _ty)| {
                let name_str = ident.to_string();
                quote! {
                    __fields.push((
                        #name_str,
                        ::serde_json::to_value(&self.#ident).unwrap_or(::serde_json::Value::Null),
                    ));
                }
            })
            .collect();
        quote! {
            fn pilcrow_live_fields(&self) -> ::std::vec::Vec<(&'static str, ::serde_json::Value)> {
                let mut __fields = ::std::vec::Vec::new();
                #(#pushes)*
                __fields
            }
        }
    };

    quote! {
        impl #impl_generics ::runtime::live_props::ListRow
            for #struct_name #ty_generics #where_clause
        {
            #key_impl
            #live_impls
        }
    }
    .into()
}
