//! Room Eq Wizard utilities
//! Provides a way to read exported biquad filter files

use crate::Biquad;
use std::str::FromStr;
use thiserror::Error;

pub trait FromRew: Sized {
    fn from_rew_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Self>;
}

pub trait ToRew {
    fn to_rew(&self) -> String;
}

impl FromRew for Biquad {
    /// Reads a single filter from the given line iterator
    fn from_rew_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Biquad> {
        let mut lines = lines.filter(|s| !s.trim().is_empty());

        // The first line contains the index
        let index = lines.next()?.strip_prefix("biquad")?;
        let index = u16::from_str(index.trim_end_matches(',')).ok()?;

        let parse_component = |line: &str, prefix: &str| -> Option<f32> {
            let line = line.strip_prefix(prefix)?;
            let line = line.trim_end_matches(',');
            f32::from_str(line).ok()
        };

        let b0 = parse_component(lines.next()?, "b0=")?;
        let b1 = parse_component(lines.next()?, "b1=")?;
        let b2 = parse_component(lines.next()?, "b2=")?;
        let a1 = parse_component(lines.next()?, "a1=")?;
        let a2 = parse_component(lines.next()?, "a2=")?;

        Some(Biquad {
            index: Some(index),
            b0,
            b1,
            b2,
            a1,
            a2,
        })
    }
}

impl ToRew for Biquad {
    fn to_rew(&self) -> String {
        format!(
            "biquad{},\nb0={},\nb1={},\nb2={},\na1={},\na2={},\n",
            self.index.unwrap_or_default(),
            self.b0,
            self.b1,
            self.b2,
            self.a1,
            self.a2
        )
    }
}

#[derive(Error, Debug)]
pub enum RewParseError {
    #[error("The filter text data was not in the expected format")]
    MalformedFilter,
}

#[cfg(test)]
mod test {
    use super::*;

    const REW_DATA: &'static str = include_str!("test_fixtures/rew-filters.txt");

    #[test]
    fn test_parse() {
        let mut filters = Vec::new();
        let mut it = REW_DATA.lines();
        while let Some(filter) = Biquad::from_rew_lines(&mut it) {
            filters.push(filter);
        }

        assert_eq!(filters.len(), 10);
        assert_eq!(filters[0].index, 1);
        assert_eq!(filters[0].b0, 0.999194903557377f32);
        assert_eq!(filters[0].b1, -1.9973354686174658);
        assert_eq!(filters[0].b2, 0.9981886333563846);
        assert_eq!(filters[0].a1, 1.9973354686174658);
        assert_eq!(filters[0].a2, -0.9973835369137615);

        assert_eq!(filters[9].index, 10);
        assert_eq!(filters[9].b0, 0.8784231224481471);
        assert_eq!(filters[9].b1, -1.2978927199484762);
        assert_eq!(filters[9].b2, 0.7320089079645271);
        assert_eq!(filters[9].a1, 1.2978927199484762);
        assert_eq!(filters[9].a2, -0.6104320304126742);
    }
    #[test]
    fn test_string() {
        let b = Biquad {
            index: 1,
            b0: 1.,
            b1: 2.,
            b2: 3.,
            a1: 4.,
            a2: 5.,
        };

        let s = b.to_rew();
        let b2 = Biquad::from_rew_lines(s.lines()).unwrap();
        assert_eq!(b, b2);
    }
}
