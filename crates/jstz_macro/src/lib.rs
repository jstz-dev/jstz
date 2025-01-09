use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, DeriveInput, Ident};

/// Derive macro to generate serde::Serialize and serde::Deserialize for cryptos in
/// jstz_cryptos. The generated serde impl act as a thin wrapper that ensures
/// JSON serialization are serialized untagged while binary serializations are tagged.
/// This is required because JSON serialization for cryptos are canonically untagged
/// while the bincode requires a tagged serde data model.
///
/// During JSON serialization, the implementation dispatches to the appropriate string
/// conversion functions. During binary serialization, the implementation dispatches to
/// serde derived impls of a generated auxiliary enum (same variants, different name)
/// which is required to ensure the bincode deserializer is driven over the enum correctly.
///
/// SAFETY: The crypto type must implement `to_base58()` and `FromStr` for compilation to succeed
#[proc_macro_derive(SerdeCrypto)]
pub fn internal_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_data = if let syn::Data::Enum(data_enum) = &input.data {
        data_enum
    } else {
        return syn::Error::new_spanned(
            input.ident,
            "SerdeCrypto can only be derived for crypto enums",
        )
        .to_compile_error()
        .into();
    };
    let original_name = &input.ident;
    let new_name = Ident::new(format!("Aux{}", original_name).as_str(), input.span());
    let variants = enum_data.variants.iter().map(|variant| {
        let ident = &variant.ident;
        let fields = &variant.fields;
        quote! {
            #ident #fields
        }
    });
    let internal_crypto = quote! {
        #[derive(Serialize, Deserialize)]
        enum #new_name {
            #(#variants),*
        }
    };
    let variant_idents = enum_data.variants.iter().map(|variant| &variant.ident);
    let serialize = quote! {
        impl serde::Serialize for #original_name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                if serializer.is_human_readable() {
                    let base58 = self.to_base58();
                    serializer.serialize_str(&base58)
                } else {
                    let cloned = match self.clone() {
                        #(Self::#variant_idents(v) => #new_name::#variant_idents(v)),*
                    };

                    #new_name::serialize(&cloned, serializer)
                }
            }
        }
    };

    let variant_idents = enum_data.variants.iter().map(|variant| &variant.ident);
    let deserialize = quote! {
        impl<'de> serde::Deserialize<'de> for #original_name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                if deserializer.is_human_readable() {
                    let string = String::deserialize(deserializer)?;
                    Self::from_str(&string).map_err(serde::de::Error::custom)
                } else {
                    match #new_name::deserialize(deserializer)? {
                        #(#new_name::#variant_idents(v) => Ok(Self::#variant_idents(v))),*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #internal_crypto
        #serialize
        #deserialize
    };

    TokenStream::from(expanded)
}
