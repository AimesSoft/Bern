//! Expansion of `#[bern::value]` enums and records.

use crate::config::Languages;
use bern_syntax::{ValueItem, ValueKind, is_bern_attr};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Item;

/// Remove Bern helper attributes and add the derives values need.
pub fn clean_item(item: &Item) -> Item {
    let mut item = item.clone();
    let derive: syn::Attribute = syn::parse_quote!(#[derive(Clone, Debug, PartialEq)]);
    match &mut item {
        Item::Enum(item) => {
            item.attrs.retain(|attr| !is_bern_attr(attr, "value"));
            item.attrs.insert(0, derive);
            for variant in &mut item.variants {
                variant.attrs.retain(|attr| !is_bern_attr(attr, "label"));
                strip_content(&mut variant.fields);
            }
        }
        Item::Struct(item) => {
            item.attrs.retain(|attr| !is_bern_attr(attr, "value"));
            item.attrs.insert(0, derive);
            strip_content(&mut item.fields);
        }
        _ => {}
    }
    item
}

/// Remove `#[content]`, which only the schema reads, from fields.
pub fn strip_content(fields: &mut syn::Fields) {
    for field in fields.iter_mut() {
        field.attrs.retain(|attr| !is_bern_attr(attr, "content"));
    }
}

/// Check that a label translates every configured language. Its
/// placeholders are the variant's fields, checked with the schema.
pub fn check_label(tr: &bern_syntax::Tr, languages: Option<&Languages>) -> syn::Result<()> {
    if !tr.args.is_empty() {
        return Err(syn::Error::new(
            tr.span,
            "a label shows its variant's fields by name, such as `{title}`; it takes no `; name: value` arguments",
        ));
    }
    let config = languages.map(|languages| (languages.supported.as_slice(), languages.strict));
    let mut errors = bern_syntax::lower::check_languages(tr, config)
        .into_iter()
        .map(|(span, message)| syn::Error::new(span, message));
    match errors.next() {
        None => Ok(()),
        Some(mut first) => {
            for error in errors {
                first.combine(error);
            }
            Err(first)
        }
    }
}

/// Trait implementations shared by standalone and protocol value types.
pub fn impls(value: &ValueItem, vis: &syn::Visibility) -> TokenStream {
    let name = &value.name;
    let p = quote!(::bern::__private::protocol);
    let b = quote!(::bern::bind);
    match &value.kind {
        ValueKind::Enum(variants) => {
            let to_arms = variants.iter().enumerate().map(|(index, variant)| {
                let variant_name = &variant.name;
                let index = index as u32;
                let fields: Vec<_> = variant.fields.iter().map(|field| &field.name).collect();
                quote! {
                    #name::#variant_name { #(#fields),* } => #p::Value::Enum {
                        variant: #index,
                        fields: ::std::vec![#(#b::BernValue::to_value(#fields)),*],
                    }
                }
            });
            let from_arms = variants.iter().enumerate().map(|(index, variant)| {
                let variant_name = &variant.name;
                let index = index as u32;
                let count = variant.fields.len();
                let fields = variant.fields.iter().enumerate().map(|(position, field)| {
                    let field_name = &field.name;
                    quote!(#field_name: #b::BernValue::from_value(&fields[#position])?)
                });
                quote! {
                    #p::Value::Enum { variant: #index, fields } if fields.len() == #count => {
                        ::std::result::Result::Ok(#name::#variant_name { #(#fields),* })
                    }
                }
            });
            let labeled = variants.iter().all(|variant| variant.label.is_some());
            let labeled_impl = labeled.then(|| quote!(impl #b::LabeledEnum for #name {}));
            let expected = format!("expected a `{name}` value");
            quote! {
                impl #b::BernValue for #name {
                    type Ref = #b::Binding<#name>;
                    fn reference() -> Self::Ref {
                        #b::Binding::declared()
                    }
                    fn to_value(&self) -> #p::Value {
                        match self {
                            #(#to_arms,)*
                        }
                    }
                    fn from_value(value: &#p::Value) -> ::std::result::Result<Self, #p::SchemaError> {
                        match value {
                            #(#from_arms,)*
                            _ => ::std::result::Result::Err(#p::SchemaError::new(#expected)),
                        }
                    }
                }
                impl #b::BernEnum for #name {}
                #labeled_impl
            }
        }
        ValueKind::Record(fields) => {
            let ref_name = format_ident!("{}Ref", name);
            let names: Vec<_> = fields.iter().map(|field| &field.name).collect();
            let types: Vec<_> = fields.iter().map(|field| &field.syn_ty).collect();
            let positions: Vec<_> = (0..fields.len()).collect();
            let count = fields.len();
            let expected = format!("expected a `{name}` record");
            let doc = format!("Typed reference to a `{name}` inside declarations.");
            quote! {
                #[doc = #doc]
                #[derive(Clone)]
                #vis struct #ref_name {
                    #(pub #names: <#types as #b::BernValue>::Ref,)*
                }
                impl #b::BernValue for #name {
                    type Ref = #ref_name;
                    fn reference() -> Self::Ref {
                        #ref_name { #(#names: <#types as #b::BernValue>::reference(),)* }
                    }
                    fn to_value(&self) -> #p::Value {
                        #p::Value::Record(::std::vec![#(#b::BernValue::to_value(&self.#names)),*])
                    }
                    fn from_value(value: &#p::Value) -> ::std::result::Result<Self, #p::SchemaError> {
                        match value {
                            #p::Value::Record(fields) if fields.len() == #count => {
                                ::std::result::Result::Ok(#name {
                                    #(#names: #b::BernValue::from_value(&fields[#positions])?,)*
                                })
                            }
                            _ => ::std::result::Result::Err(#p::SchemaError::new(#expected)),
                        }
                    }
                }
                impl #b::Readable<#name> for #ref_name {}
                impl #b::ToRef for #ref_name {
                    type Ref = #ref_name;
                    fn to_ref(&self) -> #ref_name {
                        self.clone()
                    }
                }
            }
        }
    }
}

pub fn visibility(item: &Item) -> syn::Visibility {
    match item {
        Item::Enum(item) => item.vis.clone(),
        Item::Struct(item) => item.vis.clone(),
        _ => syn::Visibility::Inherited,
    }
}

/// `pages` is the page slot the enum selects, from `pages = "slot"`.
pub fn expand_standalone(tokens: TokenStream, pages: Option<&str>) -> syn::Result<TokenStream> {
    let mut items = vec![syn::parse2(tokens)?];
    crate::config::build_modules()?.items(&mut items)?;
    let Some(mut item) = items.pop() else {
        return Ok(crate::config::track_build());
    };
    let mut track_modules = TokenStream::new();
    if let (Some(slot), Item::Enum(item)) = (pages, &mut item) {
        let (pages, manifests) = crate::config::module_pages(slot)
            .map_err(|message| syn::Error::new(item.ident.span(), message))?;
        add_pages(item, &pages);
        let manifests = manifests.iter().map(|path| path.display().to_string());
        track_modules = quote!(#(const _: &[u8] = ::core::include_bytes!(#manifests);)*);
    }
    let value = bern_syntax::parse_value(&item)?;
    let languages =
        crate::config::languages().map_err(|message| syn::Error::new(value.span, message))?;
    if let ValueKind::Enum(variants) = &value.kind {
        for variant in variants {
            if let Some(label) = &variant.label {
                check_label(label, languages.as_ref())?;
            }
        }
    }
    let cleaned = clean_item(&item);
    let impls = impls(&value, &visibility(&item));
    let track = crate::component::track(languages.as_ref());
    let track_build = crate::config::track_build();
    Ok(quote! {
        #cleaned
        #impls
        #track
        #track_modules
        #track_build
    })
}

/// Add a labeled variant for every page a module adds to the enum's slot,
/// named like the build tool names it: page `new_shows` is `NewShows`. A
/// variant the enum already has stays; the build tool reports it.
fn add_pages(item: &mut syn::ItemEnum, pages: &[crate::config::ModulePage]) {
    for page in pages {
        let name = format_ident!("{}", bern_syntax::pascal_case(&page.id));
        if item.variants.iter().any(|variant| variant.ident == name) {
            continue;
        }
        let languages = page.label.iter().map(|(language, _)| language);
        let texts = page.label.iter().map(|(_, text)| text);
        let variant: syn::Variant = if page.label.is_empty() {
            syn::parse_quote!(#name)
        } else {
            syn::parse_quote!(#[label(tr! { #(#languages => #texts),* })] #name)
        };
        item.variants.push(variant);
    }
}
