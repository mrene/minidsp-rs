//! Basic biquad definition

#[derive(Debug, PartialEq)]
pub struct Biquad {
    pub index: u16,
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}
