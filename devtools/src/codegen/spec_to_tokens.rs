use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

use super::spec::*;

macro_rules! resolve_sym {
    ($self:tt, $resolve:tt, $($name:tt),+) => {
        $(
        let $name = $self.$name.to_symbol_tokens(|s| $resolve(s));
        )+
    };
}

pub trait ToSymbolTokens {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, resolve: F) -> TokenStream;
}

impl ToSymbolTokens for Device {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        let product_name = Literal::string(&self.product_name);
        let sources = self
            .sources
            .iter()
            .map(|name| Ident::new(name, Span::call_site()));
        let fir_max_taps = Literal::usize_unsuffixed(self.fir_max_taps as usize);
        let internal_sampling_rate = Literal::u32_unsuffixed(self.internal_sampling_rate);
        resolve_sym!(self, resolve, inputs, outputs);

        quote! {
            pub const DEVICE: Device = Device {
                product_name: #product_name,
                sources: &[#(#sources),*],
                inputs: #inputs,
                outputs: #outputs,
                fir_max_taps: #fir_max_taps,
                internal_sampling_rate: #internal_sampling_rate,
                #[cfg(feature="symbols")]
                symbols: SYMBOLS,
            };
        }
    }
}

impl ToSymbolTokens for Input {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(self, resolve, gate, meter, routing, peq);
        quote! {
            Input {
                gate: #gate,
                meter: #meter,
                routing: #routing,
                peq: #peq,
            }
        }
    }
}

impl ToSymbolTokens for Gate {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(self, resolve, enable, gain);
        quote! {
            Gate {
                enable: #enable,
                gain: #gain,
            }
        }
    }
}

impl<T> ToSymbolTokens for Vec<T>
where
    T: ToSymbolTokens,
{
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        let it = self.iter().map(|s| s.to_symbol_tokens(|s| resolve(s)));
        quote! {
            &[ #(#it),* ]
        }
    }
}

impl<T> ToSymbolTokens for Option<T>
where
    T: ToSymbolTokens,
{
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, resolve: F) -> TokenStream {
        match self {
            None => quote! { None },
            Some(x) => {
                let tokens = x.to_symbol_tokens(resolve);
                quote! { Some(#tokens) }
            }
        }
    }
}

impl ToSymbolTokens for &str {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve(self)
    }
}

impl ToSymbolTokens for String {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve(self)
    }
}

impl ToSymbolTokens for Output {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(
            self,
            resolve,
            gate,
            meter,
            delay_addr,
            invert_addr,
            peq,
            xover,
            compressor,
            fir
        );
        quote! {
            Output {
                gate: #gate,
                meter: #meter,
                delay_addr: #delay_addr,
                invert_addr: #invert_addr,
                peq: #peq,
                xover: #xover,
                compressor: #compressor,
                fir: #fir,
            }
        }
    }
}

impl ToSymbolTokens for Crossover {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(self, resolve, peqs);
        quote! {
            Crossover { peqs: #peqs }
        }
    }
}

impl ToSymbolTokens for Compressor {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(self, resolve, bypass, threshold, ratio, attack, release, meter);

        quote! {
            Compressor {
                bypass: #bypass,
                threshold: #threshold,
                ratio: #ratio,
                attack: #attack,
                release: #release,
                meter: #meter,
            }
        }
    }
}

impl ToSymbolTokens for Fir {
    fn to_symbol_tokens<F: FnMut(&str) -> TokenStream>(&self, mut resolve: F) -> TokenStream {
        resolve_sym!(self, resolve, bypass, num_coefficients);
        let index = Literal::isize_unsuffixed(self.index as isize);
        let max_coefficients = Literal::isize_unsuffixed(self.max_coefficients as isize);

        quote! {
            Fir {
                index: #index,
                bypass: #bypass,
                num_coefficients: #num_coefficients,
                max_coefficients: #max_coefficients,
            }
        }
    }
}
