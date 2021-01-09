//! Room Eq Wizard utilities
//! Provides a way to read exported biquad filter files

use std::str::FromStr;
use thiserror::Error;

pub struct Biquad {
    pub index: u16,
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Biquad {
    /// Reads a single filter from the given line iterator
    pub fn from_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Biquad> {
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
            index,
            b0,
            b1,
            b2,
            a1,
            a2,
        })
    }
}

impl ToString for Biquad {
    fn to_string(&self) -> String {
        format!(
            "biquad{},\nb0={},\nb1={},\nb2={},\na1={},\na2={},\n",
            self.index, self.b0, self.b1, self.b2, self.a1, self.a2
        )
    }
}

impl FromStr for Biquad {
    type Err = RewParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Biquad::from_lines(s.lines()).ok_or(RewParseError::MalformedFilter)
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
        while let Some(filter) = Biquad::from_lines(&mut it) {
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
}
