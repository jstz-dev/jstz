use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse, parse_macro_input, Ident, ItemEnum, ItemStruct, LitInt, Type, TypePath,
};

fn is_option(typepath: &TypePath) -> bool {
    let p = typepath
        .path
        .segments
        .iter()
        .fold(String::new(), |mut acc, v| {
            acc.push_str(&v.ident.to_string());
            acc.push(':');
            acc
        });
    return vec!["Option:", "std:option:Option:", "core:option:Option:"]
        .into_iter()
        .find(|s| p == *s)
        .is_some();
}

#[proc_macro_attribute]
pub fn api_map_to(attr: TokenStream, item: TokenStream) -> TokenStream {
    let other_type = parse_macro_input!(attr as Ident);
    if let Ok(input) = parse::<ItemStruct>(item.clone()) {
        let mut tokens = vec![];
        for (idx, f) in input.fields.iter().enumerate() {
            if let Some(v) = &f.ident {
                let mut q = None;
                match &f.ty {
                    Type::Path(typepath) if typepath.qself.is_none() => {
                        if is_option(typepath) {
                            q.replace(quote! {self.#v.map(|r| r.into())});
                        }
                    }
                    _ => (),
                };
                let rhs = q.unwrap_or(quote! {self.#v.into()});
                tokens.push(quote! {
                    #v: #rhs,
                });
            } else {
                let v = LitInt::new(&idx.to_string(), proc_macro2::Span::call_site());
                tokens.push(quote! {#v: self.#v.into(),});
            }
        }
        let type_name = input.ident.clone();
        TokenStream::from(quote! {
                #input

                impl Into<#other_type> for #type_name {
                    fn into(self) -> #other_type {
                        #other_type {
                            #(#tokens)*
                        }
                    }
                }

                impl Into<#type_name> for #other_type {
                    fn into(self) -> #type_name {
                        #type_name {
                            #(#tokens)*
                        }
                    }
                }
        })
    } else if let Ok(input) = parse::<ItemEnum>(item.clone()) {
        let mut tokens = vec![];
        let mut tokens_rev = vec![];
        let type_name = input.ident.clone();
        for f in &input.variants {
            let mut vs = vec![];
            let mut vs2 = vec![];
            for idx in 0..f.fields.len() {
                let ident =
                    Ident::new(&format!("v{idx}"), proc_macro2::Span::call_site());
                vs.push(quote! {#ident,});
                vs2.push(quote! {#ident.into(),});
            }
            let name = f.ident.clone();
            tokens.push(quote! {
                #type_name::#name(#(#vs)*) => #other_type::#name(#(#vs2)*),
            });
            tokens_rev.push(quote! {
                #other_type::#name(#(#vs)*) => #type_name::#name(#(#vs2)*),
            });
        }
        TokenStream::from(quote! {
            #input

            impl Into<#other_type> for #type_name {
                fn into(self) -> #other_type {
                    match self {
                        #(#tokens)*
                    }
                }
            }

            impl Into<#type_name> for #other_type {
                fn into(self) -> #type_name {
                    match self {
                        #(#tokens_rev)*
                    }
                }
            }
        })
    } else {
        item
    }
}
