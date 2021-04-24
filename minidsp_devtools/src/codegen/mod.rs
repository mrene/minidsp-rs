//! Code to generate device definitions

pub mod m2x4hd;
pub mod spec;
pub mod spec_to_tokens;

use bimap::BiHashMap;
use inflector::Inflector;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

type SymbolMap = BiHashMap<String, usize>;

pub struct FirSpec {
    pub index: usize,
    pub bypass: String,
    pub num_coefficients: String,
    pub max_coefficients: u32,
}

pub struct CompressorSpec {
    pub bypass: String,
    pub threshold: String,
    pub ratio: String,
    pub attack: String,
    pub release: String,
    pub meter: String,
}

pub trait DeviceSpec: Sized {
    fn product_name(&self) -> String;
    fn num_inputs(&self) -> usize;
    fn num_outputs(&self) -> usize;

    /// Name of input sources, must match the Source enum in source.rs
    fn sources_names(&self) -> Vec<String>;

    /// The name of the element controlling routing gate status
    /// Example: MixerNxMSmoothed1_0_0_status
    fn routing_enable(&self, input: usize, output: usize) -> String;

    /// The name of the element controlling the gain between an input and output channel
    fn routing_gain(&self, input: usize, output: usize) -> String;
    fn input_meter(&self, input: usize) -> String;
    fn input_enable(&self, input: usize) -> String;
    fn input_gain(&self, input: usize) -> String;

    fn input_num_peq(&self) -> usize;
    fn input_peq(&self, input: usize, index: usize) -> String;

    fn output_meter(&self, output: usize) -> String;
    fn output_enable(&self, output: usize) -> String;
    fn output_gain(&self, output: usize) -> String;
    fn output_delay(&self, output: usize) -> String;
    fn output_invert(&self, output: usize) -> String;

    fn output_num_peq(&self) -> usize;
    fn output_peq(&self, output: usize, index: usize) -> String;

    fn output_xover(&self, output: usize, group: usize) -> String;

    fn output_compressor(&self, output: usize) -> CompressorSpec;
    fn output_fir(&self, output: usize) -> FirSpec;

    fn fir_max_taps(&self) -> usize;
    fn internal_sampling_rate(&self) -> u32;

    fn symbol_map(&mut self) -> &mut SymbolMap;

    // Code generation methods
    fn generate_static_config(&mut self, name: &str) -> TokenStream {
        let symbols = self.generate_symbols();

        let source_names = self.sources_names();
        let sources = source_names
            .iter()
            .map(|name| Ident::new(name, Span::call_site()));

        let inputs: Vec<_> = (0..self.num_inputs())
            .map(|input| self.generate_input(input))
            .collect();

        let outputs: Vec<_> = (0..self.num_outputs())
            .map(|output| self.generate_output(output))
            .collect();

        let name = Ident::new(name, Span::call_site());

        let fir_max_taps = Literal::usize_unsuffixed(self.fir_max_taps());
        let internal_sampling_rate = Literal::u32_unsuffixed(self.internal_sampling_rate());
        let product_name = Literal::string(self.product_name().as_str());

        quote! {
            use super::*;

            #symbols

            pub const DEVICE: Device = Device {
                product_name: #product_name,
                sources: &[#(#sources),*],
                inputs: &[ #(#inputs),* ],
                outputs: &[ #(#outputs),* ],
                fir_max_taps: #fir_max_taps,
                internal_sampling_rate: #internal_sampling_rate,
            };
        }
    }

    fn generate_symbols(&mut self) -> TokenStream {
        let mut syms: Vec<_> = self
            .symbol_map()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

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
            use sym::*;
        }
    }

    fn resolve_symbol<T: AsRef<str>>(&mut self, name: T) -> TokenStream {
        self.symbol_map()
            .remove_by_left(name.as_ref())
            .unwrap_or_else(|| panic!("Couldn't find config entry {}", name.as_ref()));

        let name = Ident::new(
            name.as_ref().to_screaming_snake_case().as_ref(),
            Span::call_site(),
        );
        quote! { #name }
    }

    fn generate_input(&mut self, input: usize) -> TokenStream {
        let routing = {
            let it = (0..self.num_outputs()).map(|output| {
                self.generate_gate(
                    self.routing_enable(input, output),
                    self.routing_gain(input, output),
                )
            });
            quote! { &[ #(#it),* ] }
        };

        let gate = self.generate_gate(self.input_enable(input), self.input_gain(input));

        let peqs = {
            let it =
                (0..self.input_num_peq()).map(|i| self.resolve_symbol(self.input_peq(input, i)));
            quote! {
                &[ #(#it),* ]
            }
        };

        let meter = self.resolve_symbol(self.input_meter(input));

        quote! {
            Input {
                gate: #gate,
                meter: #meter,
                routing: #routing,
                peq: #peqs,
            }
        }
    }

    fn generate_gate<T: AsRef<str>>(&mut self, enable: T, gain: T) -> TokenStream {
        let enable = self.resolve_symbol(enable);
        let gain = self.resolve_symbol(gain);
        quote! {
            Gate {
                enable: #enable,
                gain: #gain,
            }
        }
    }

    fn generate_output(&mut self, output: usize) -> TokenStream {
        let gate = self.generate_gate(self.output_enable(output), self.output_gain(output));
        let meter = self.resolve_symbol(self.output_meter(output));

        let peqs = {
            let it =
                (0..self.output_num_peq()).map(|i| self.resolve_symbol(self.output_peq(output, i)));
            quote! {
                &[ #(#it),* ]
            }
        };
        let delay_addr = self.resolve_symbol(self.output_delay(output));
        let invert_addr = self.resolve_symbol(self.output_invert(output));
        let xover = {
            let xover0 = self.resolve_symbol(self.output_xover(output, 0));
            let xover1 = self.resolve_symbol(self.output_xover(output, 1));
            quote! {
                Crossover { peqs: &[ #xover0, #xover1 ] }
            }
        };

        let compressor = {
            let spec = self.output_compressor(output);
            let bypass = self.resolve_symbol(spec.bypass);
            let threshold = self.resolve_symbol(spec.threshold);
            let ratio = self.resolve_symbol(spec.ratio);
            let attack = self.resolve_symbol(spec.attack);
            let release = self.resolve_symbol(spec.release);
            let meter = self.resolve_symbol(spec.meter);

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
        };

        let fir = {
            let fir = self.output_fir(output);
            let index = Literal::isize_unsuffixed(fir.index as isize);
            let bypass = self.resolve_symbol(fir.bypass);
            let num_coefficients = self.resolve_symbol(fir.num_coefficients);
            let max_coefficients = Literal::isize_unsuffixed(fir.max_coefficients as isize);

            quote! {
                Fir {
                    index: #index,
                    bypass: #bypass,
                    num_coefficients: #num_coefficients,
                    max_coefficients: #max_coefficients,
                }
            }
        };

        quote! {
            Output {
                gate: #gate,
                meter: #meter,
                peq: #peqs,
                delay_addr: #delay_addr,
                invert_addr: #invert_addr,
                xover: Some(#xover),
                compressor: Some(#compressor),
                fir: Some(#fir),
            }
        }
    }
}
