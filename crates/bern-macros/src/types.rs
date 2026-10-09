use bern_protocol::Type;
use proc_macro2::TokenStream;
use quote::quote;

pub fn type_tokens(ty: &Type) -> TokenStream {
    let p = quote!(::bern::__private::protocol);
    match ty {
        Type::Unit => quote!(#p::Type::Unit),
        Type::Bool => quote!(#p::Type::Bool),
        Type::Int => quote!(#p::Type::Int),
        Type::Float => quote!(#p::Type::Float),
        Type::Text => quote!(#p::Type::Text),
        Type::Enum { name } => quote!(#p::Type::Enum { name: ::std::string::String::from(#name) }),
        Type::Record { name } => {
            quote!(#p::Type::Record { name: ::std::string::String::from(#name) })
        }
        Type::Optional { inner } => {
            let inner = type_tokens(inner);
            quote!(#p::Type::optional(#inner))
        }
        Type::List { inner } => {
            let inner = type_tokens(inner);
            quote!(#p::Type::list(#inner))
        }
    }
}
