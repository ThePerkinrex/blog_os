use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{Ident, ItemTrait, Type, parse2, spanned::Spanned};

fn rust_to_iface_type(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::BareFn(type_bare_fn) => todo!(),
        Type::Reference(type_reference) => todo!(),
        Type::Array(type_array) => todo!(),
        Type::Group(type_group) => todo!(),
        Type::ImplTrait(type_impl_trait) => {
            quote_spanned! {type_impl_trait.span() => compile_error!("impl traits not supported in C interfaces")}
        }
        Type::Infer(type_infer) => {
            quote_spanned! {type_infer.span() => compile_error!("inferred types not supported in C interfaces")}
        }
        Type::Macro(type_macro) => {
            quote_spanned! {type_macro.span() => compile_error!("macro types not supported in C interfaces")}
        }
        Type::Never(type_never) => {
            quote_spanned! {type_never.span() => compile_error!("never types not supported in C interfaces")}
        }
        Type::Paren(type_paren) => todo!(),
        Type::Path(type_path) => todo!(),
        Type::Ptr(type_ptr) => todo!(),
        Type::Slice(type_slice) => {
            quote_spanned! {type_slice.span() => compile_error!("Slices not supported in C interfaces")}
        }
        Type::TraitObject(type_trait_object) => {
            quote_spanned! {type_trait_object.span() => compile_error!("Trait objects not supported in C interfaces")}
        }
        Type::Tuple(type_tuple) => {
            quote_spanned! {type_tuple.span() => compile_error!("Tuples not supported in C interfaces")}
        }
        Type::Verbatim(token_stream) => {
            quote_spanned! {token_stream.span() => compile_error!("Unknown type")}
        }
        x => x.to_token_stream(),
    }
}

fn testable_c_interface(
    _: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    // Parse the input tokens into a syntax tree
    let input = match parse2::<ItemTrait>(item) {
        Ok(x) => x,
        Err(e) => return e.into_compile_error(),
    };

    if input.generics.const_params().count() > 0 {
        let span = input.generics.span();
        return quote_spanned! {span => compile_error!("const params not supported")};
    }

    if input.generics.type_params().count() > 0 {
        let span = input.generics.span();
        return quote_spanned! {span => compile_error!("type params not supported")};
    }

    let vis = &input.vis;

    let name = &input.ident;

    let mut consts = Vec::new();
    let mut fns = Vec::new();

    for item in &input.items {
        match item {
            syn::TraitItem::Const(trait_item_const) => {
                consts.push(trait_item_const);
            }
            syn::TraitItem::Fn(trait_item_fn) => {
                fns.push(trait_item_fn);
            }
            syn::TraitItem::Type(trait_item_type) => {
                return quote_spanned! {trait_item_type.span() => compile_error!("trait types not allowed")};
            }
            syn::TraitItem::Macro(trait_item_macro) => {
                return quote_spanned! {trait_item_macro.span() => compile_error!("trait macros not allowed")};
            }
            syn::TraitItem::Verbatim(token_stream) => {
                return quote_spanned! {token_stream.span() => compile_error!("unknown tokens")};
            }
            _ => return quote_spanned! {item.span() => compile_error!("unknown item type")},
        }
    }

    let data_name = format_ident!("{name}Data");
    let ops_name = format_ident!("{name}Ops");

    let data = quote! {
        #[repr(C)]
        #vis struct #data_name {
            _data: (),
            _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
        }
    };

    let ops_constants: proc_macro2::TokenStream = consts
        .iter()
        .map(|x| {
            let name = Ident::new(&x.ident.to_string().to_snake_case(), x.ident.span());
            let ty = rust_to_iface_type(&x.ty);
            quote! {#vis #name: #ty,}
        })
        .collect();

    let ops = quote! {
        #[repr(C)]
        #vis struct #ops_name {
            #ops_constants

            #vis free: extern "C" fn(*mut #data_name),
        }
    };

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        #data

        #ops

        // ...
        #input
    };

    // Hand the output tokens back to the compiler
    expanded
}

#[proc_macro_attribute]
pub fn c_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    TokenStream::from(testable_c_interface(attr.into(), item.into()))
}

#[cfg(test)]
mod test {
    use quote::quote;

    use crate::testable_c_interface;

    #[test]
    fn example() {
        let res = testable_c_interface(
            quote! {},
            quote! {
                pub trait A {
                    const NAME: &'static CStr;
                }
            },
        );

        println!("{res}")
    }
}
