//! Basic biquad definition

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Biquad {
    pub index: Option<u16>,
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Biquad {
    pub fn to_array(&self) -> [f32; 5] {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
    }
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad {
            index: None,
            b0: 1.,
            b1: 0.,
            b2: 0.,
            a1: 0.,
            a2: 0.,
        }
    }
}

impl From<&Biquad> for [f32; 5] {
    fn from(bq: &Biquad) -> Self {
        [bq.b0, bq.b1, bq.b2, bq.a1, bq.a2]
    }
}
