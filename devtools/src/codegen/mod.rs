//! Code to generate device definitions

pub mod c8x12v2;
pub mod ddrc24;
pub mod ddrc88bm;
pub mod m10x10hd;
pub mod m2x4;
pub mod m2x4hd;
pub mod m4x10hd;
pub mod msharc4x8;
pub mod nanodigi2x8;
pub mod shd;

pub mod spec;
pub mod spec_to_tokens;

use bimap::BiHashMap;
use inflector::Inflector;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;
use spec_to_tokens::ToSymbolTokens;

type SymbolMap = BiHashMap<String, usize>;

fn generate_symbols(symbols: &SymbolMap) -> TokenStream {
    let mut syms: Vec<_> = symbols.iter().map(|(k, v)| (k.clone(), *v)).collect();

    // Sort symbols by address
    syms.sort_unstable_by_key(|s| s.1);

    let vals = syms.iter().map(|(k, v)| {
        let k = Ident::new(k.to_screaming_snake_case().as_str(), Span::call_site());
        let v = Literal::usize_unsuffixed(*v);
        quote! { pub const #k :u16 = #v; }
    });

    let mapped = syms.iter().map(|(k, _)| {
        let name = k.to_screaming_snake_case();
        let sym_ref = Ident::new(&name, Span::call_site());
        let sym_name = Literal::string(&name);
        quote! { (#sym_name, #sym_ref) }
    });

    quote! {
        pub mod sym {
            #[allow(dead_code)]
            #(#vals)*

            #[cfg(feature="symbols")]
            pub const SYMBOLS: &[(&str, u16)] = &[#(#mapped),*];
        }
        #[allow(unused_imports)]
        use sym::*;
    }
}

fn resolve_symbol<T: AsRef<str> + std::fmt::Debug>(
    symbols: &mut SymbolMap,
    name: T,
) -> TokenStream {
    dbg!(&name);
    if name.as_ref().is_empty() {
        panic!("missing item");
        // return quote! { 0 };
    }

    symbols
        .remove_by_left(name.as_ref())
        .unwrap_or_else(|| panic!("Couldn't find config entry {}", name.as_ref()));

    let name = Ident::new(
        name.as_ref().to_screaming_snake_case().as_ref(),
        Span::call_site(),
    );
    quote! { #name }
}

pub fn generate_static_config(symbol_map: &mut SymbolMap, spec: &spec::Device) -> TokenStream {
    let symbols = generate_symbols(symbol_map);
    let device = spec.to_symbol_tokens(|s| resolve_symbol(symbol_map, s));

    quote! {
        use super::*;

        #symbols
        #device
    }
}
