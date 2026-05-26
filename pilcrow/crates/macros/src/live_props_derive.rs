use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let field_extractions = match &input.data {
        Data::Struct(s) => extract_live_fields(&s.fields),
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "PilcrowProps can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    quote! {
        impl #impl_generics ::runtime::live_props::LivePropExtract
            for #struct_name #ty_generics #where_clause
        {
            fn live_fields(&self) -> ::std::vec::Vec<::runtime::live_props::LiveFieldData> {
                let mut __fields = ::std::vec::Vec::new();
                #field_extractions
                __fields
            }
        }
    }
    .into()
}

fn extract_live_fields(fields: &Fields) -> TokenStream2 {
    let named = match fields {
        Fields::Named(f) => &f.named,
        _ => return quote! {},
    };

    let mut extractions = quote! {};
    for field in named {
        if !is_live_props_type(&field.ty) {
            continue;
        }
        let field_ident = match &field.ident {
            Some(i) => i,
            None => continue,
        };
        let field_name_str = field_ident.to_string();

        let column_name = find_str_attr(&field.attrs, "column");
        let patch_debounce = find_u32_attr(&field.attrs, "patch_debounce");

        extractions = quote! {
            #extractions
            {
                let mut __lp = self.#field_ident.clone();
                let mut __name = #field_name_str.to_string();
                if let ::std::option::Option::Some(__c) = #column_name {
                    __name = __c;
                }
                if let ::std::option::Option::Some(__n) = #patch_debounce {
                    __lp = __lp.patch_debounce(__n);
                }
                __fields.push(__lp.to_field_data(__name));
            }
        };
    }
    extractions
}

fn is_live_props_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
    {
        return seg.ident == "LiveProp";
    }
    false
}

fn find_u32_attr(attrs: &[syn::Attribute], name: &str) -> TokenStream2 {
    for attr in attrs {
        if attr.path().is_ident(name)
            && let Ok(lit) = attr.parse_args::<syn::LitInt>()
            && let Ok(val) = lit.base10_parse::<u32>()
        {
            return quote! { ::std::option::Option::Some(#val as u32) };
        }
    }
    quote! { ::std::option::Option::None::<u32> }
}

fn find_str_attr(attrs: &[syn::Attribute], name: &str) -> TokenStream2 {
    for attr in attrs {
        if attr.path().is_ident(name)
            && let Ok(lit) = attr.parse_args::<syn::LitStr>()
        {
            let val = lit.value();
            return quote! { ::std::option::Option::Some(#val.to_string()) };
        }
    }
    quote! { ::std::option::Option::None::<String> }
}
