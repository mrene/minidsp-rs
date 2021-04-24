//! Utilities for dealing with xml configuration files
use std::{fmt, str::FromStr};

use bimap::BiMap;
use bytes::Bytes;
use strong_xml::{XmlRead, XmlWrite};

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "setting")]
pub struct Setting {
    #[xml(attr = "version")]
    pub version: String,

    #[xml(flatten_text = "timestamp")]
    pub timestamp: u32,

    #[xml(flatten_text = "dspversion")]
    pub dsp_version: u8,

    #[xml(flatten_text = "master_mute")]
    pub master_mute: u8,

    #[xml(child = "label")]
    pub labels: Vec<Label>,

    #[xml(child = "item", child = "fir", child = "filter")]
    pub items: Vec<AddressableElement>,
}

impl Setting {
    /// Creates a blob that can be sent to the device in order to restore all settings under a
    /// given configuration preset index
    pub fn to_restore_blob(&self) -> Bytes {
        // The configuration contains addresses which refer to _indices_ of floats in the dsp memory
        // We can build the blob by iterating through everything, and writing a little endian f32
        // at each address (which ends up being 4*index since each f32 is 4 bytes).

        let mut buf = RestoreBlob(Vec::with_capacity(65536));
        // buf.0.resize(65536, 0);

        for item in &self.items {
            match item {
                AddressableElement::Item { addr, hex, .. } => {
                    buf.put_slice_at(*addr, &hex.inner);
                }
                AddressableElement::Fir { addr, para, .. } => {
                    if para.is_empty() {
                        continue;
                    }

                    let mut addr = *addr;
                    let para = &para[0];
                    for subpara in &para.subpara {
                        for value in &subpara.data.inner {
                            buf.put_slice_at(addr, &value.inner);
                            addr += 1;
                        }
                    }
                }
                AddressableElement::Filter { addr, hex, .. } => {
                    let mut addr = *addr;
                    for value in &hex.inner {
                        buf.put_slice_at(addr, &value.inner);
                        addr += 1
                    }
                }
            }
        }

        Bytes::from(buf.0)
    }

    /// Sorts all elements by address
    pub fn sort(&mut self) {
        self.items.sort_unstable_by_key(|item| *match item {
            AddressableElement::Item { addr, .. } => addr,
            AddressableElement::Fir { addr, .. } => addr,
            AddressableElement::Filter { addr, .. } => addr,
        });

        // Sort FIR rows so we send them in the right order
        for item in self.items.iter_mut() {
            if let AddressableElement::Fir { para, .. } = item {
                if para.is_empty() {
                    continue;
                }
                para[0].subpara.sort_unstable_by_key(|sp| sp.row);
            }
        }
    }

    /// Returns a BiMap with all names and indices inside this config
    pub fn name_map(&self) -> BiMap<String, usize> {
        let mut map = BiMap::<String, usize>::new();

        for item in &self.items {
            match item {
                AddressableElement::Item { name, addr, .. } => {
                    map.insert(name.clone(), *addr as usize);
                }
                AddressableElement::Fir { name, addr, .. } => {
                    map.insert(name.clone(), *addr as usize);
                }
                AddressableElement::Filter { name, addr, .. } => {
                    map.insert(name.clone(), *addr as usize);
                }
            }
        }

        map
    }
}

pub struct RestoreBlob(pub Vec<u8>);
impl RestoreBlob {
    pub fn put_slice_at(&mut self, at: usize, x: &[u8]) {
        // Make sure the blob is big enough to hold the given address
        let (start, end) = (at * 4, at * 4 + x.len());
        self.ensure_size(end);
        // Reverse the iterator because the config stores hex data in the opposite endianness
        let splice = self.0.splice(start..end, x.iter().rev().cloned());
        debug_assert!(splice.count() == x.len());
    }
    pub fn put_f32_le(&mut self, at: usize, x: f32) {
        // Make sure the blob is big enough to hold the given address
        self.put_slice_at(at, &x.to_le_bytes())
    }

    fn ensure_size(&mut self, size: usize) {
        if self.0.len() < size {
            self.0.resize(size, 0);
        }
    }
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "label")]
pub struct Label {
    #[xml(attr = "name")]
    pub name: String,
    #[xml(attr = "text")]
    pub text: String,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
pub enum AddressableElement {
    #[xml(tag = "item")]
    Item {
        #[xml(attr = "name")]
        name: String,
        #[xml(attr = "addr")]
        addr: usize,
        #[xml(flatten_text = "dec")]
        dec: String,
        #[xml(flatten_text = "hex")]
        hex: HexString,
    },
    #[xml(tag = "fir")]
    Fir {
        #[xml(attr = "name")]
        name: String,
        #[xml(attr = "addr")]
        addr: usize,
        #[xml(child = "para")]
        para: Vec<Para>,
    },
    #[xml(tag = "filter")]
    Filter {
        #[xml(attr = "name")]
        name: String,
        #[xml(attr = "addr")]
        addr: usize,
        #[xml(flatten_text = "freq")]
        freq: u16,
        #[xml(flatten_text = "q")]
        q: f32,
        #[xml(flatten_text = "boost")]
        boost: f32,
        #[xml(flatten_text = "type")]
        typ: String,
        #[xml(flatten_text = "bypass")]
        bypass: u8,
        #[xml(flatten_text = "dec")]
        dec: CommaSeparatedList<f32>,
        #[xml(flatten_text = "hex")]
        hex: CommaSeparatedList<HexString>,
    },
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "para")]
pub struct Para {
    #[xml(attr = "count")]
    pub count: u16,
    #[xml(attr = "rows")]
    pub rows: u16,
    #[xml(child = "subpara")]
    pub subpara: Vec<Subpara>,
}

#[derive(Debug, Clone, XmlRead, XmlWrite, PartialEq)]
#[xml(tag = "subpara")]
pub struct Subpara {
    #[xml(attr = "row")]
    pub row: u8,
    #[xml(text)]
    pub data: CommaSeparatedList<HexString>,
}

/// Wrapper class holding a list of comma-separated values
#[derive(Debug, Clone, PartialEq)]
pub struct CommaSeparatedList<T> {
    pub inner: Vec<T>,
}

impl<T> CommaSeparatedList<T> {
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }
}

impl<T: FromStr> FromStr for CommaSeparatedList<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut vec = Vec::new();
        for part in s.split(',') {
            if !part.is_empty() {
                vec.push(<T as FromStr>::from_str(part.trim())?);
            }
        }
        Ok(CommaSeparatedList { inner: vec })
    }
}

impl<T: fmt::Display> fmt::Display for CommaSeparatedList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.inner
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}

/// Wrapper class to support deserializing bytes using FromStr
#[derive(Debug, Clone, PartialEq)]
pub struct HexString {
    pub inner: Bytes,
}

impl FromStr for HexString {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 2 {
            Ok(HexString {
                inner: Bytes::from_static(&[0, 0, 0, 0]),
            })
        } else if s.len() % 2 != 0 {
            Ok(HexString {
                inner: Bytes::from(hex::decode("0".to_string() + s)?),
            })
        } else {
            Ok(HexString {
                inner: Bytes::from(hex::decode(s)?),
            })
        }
    }
}

impl fmt::Display for HexString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.inner.as_ref()))
    }
}

#[cfg(test)]
mod test {
    use futures::{pin_mut, AsyncReadExt, Future, StreamExt, TryStreamExt};

    use super::*;
    use crate::{commands::Commands, utils::recorder};

    /// Extracts a restore blob from a built-in recorded fixture
    async fn extract_blob<F, Fut>(fixture: &'static [u8], f: F) -> Bytes
    where
        F: FnMut(Commands) -> Fut,
        Fut: Future<Output = Option<Result<Bytes, std::io::Error>>>,
    {
        let stream = recorder::fixtures_reader(fixture)
            .filter_map(recorder::decode_sent_commands)
            .filter_map(f);

        pin_mut!(stream);
        let mut reader = stream.into_async_read();
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).await.unwrap();

        Bytes::from(buffer)
    }

    /// Extracts a restore blob from a built-in recorded fixture
    async fn extract_restore_blob(fixture: &'static [u8]) -> Bytes {
        extract_blob(fixture, |x| async {
            match x {
                Commands::BulkLoad { payload } => Some(Ok::<Bytes, std::io::Error>(payload.0)),
                _ => None,
            }
        })
        .await
        // Skip the 7 bytes header
        .slice(7..)
    }

    /// Extracts a filter block from a built-in recorded fixture
    async fn extract_filter_block(fixture: &'static [u8]) -> Bytes {
        extract_blob(fixture, |x| async {
            match x {
                Commands::BulkLoadFilterData { payload } => {
                    Some(Ok::<Bytes, std::io::Error>(payload.0))
                }
                _ => None,
            }
        })
        .await
        // Skip the 4 bytes header
        .slice(4..)
    }

    #[tokio::test]
    async fn test_restore_blob() {
        struct Fixture {
            xml: &'static str,
            sync: &'static [u8],
        }
        let fixtures = &[
            Fixture {
                xml: include_str!("../../test_fixtures/config1/config.xml"),
                sync: include_bytes!("../../test_fixtures/config1/sync.txt"),
            },
            Fixture {
                xml: include_str!("../../test_fixtures/config2/config.xml"),
                sync: include_bytes!("../../test_fixtures/config2/sync.txt"),
            },
            Fixture {
                xml: include_str!("../../test_fixtures/config3/config.xml"),
                sync: include_bytes!("../../test_fixtures/config3/sync.txt"),
            },
        ];

        for fixture in fixtures.iter() {
            let s = Setting::from_str(fixture.xml).unwrap();
            let cfg = s.to_restore_blob();
            let blob = extract_restore_blob(fixture.sync).await;
            assert_eq!(cfg.as_ref(), blob);
            let _ = extract_filter_block(fixture.sync).await;
            // TODO: Generate and test this block
        }
    }

    #[test]
    fn test_comma_separated() {
        let s = "1.1,2.2,3.3,4.4";
        let expected: &[f32] = &[1.1, 2.2, 3.3, 4.4];
        let parsed = CommaSeparatedList::<f32>::from_str(s).unwrap();
        assert!(parsed.inner.iter().cloned().eq(expected.iter().cloned()));
        assert_eq!(parsed.to_string().as_str(), s);

        let s = "1, 2,, 3, 4";
        let expected: &[f32] = &[1.0, 2.0, 3.0, 4.];
        let parsed = CommaSeparatedList::<f32>::from_str(s).unwrap();
        assert!(parsed.inner.iter().cloned().eq(expected.iter().cloned()));

        let s = "01020304,05060708,090a0b0c";
        let expected: &[&[u8]] = &[
            &[0x01, 0x02, 0x03, 0x04],
            &[0x05, 0x06, 0x07, 0x08],
            &[0x09, 0x0A, 0x0B, 0x0C],
        ];
        let parsed = CommaSeparatedList::<HexString>::from_str(s).unwrap();
        for (e, p) in expected.iter().zip(parsed.inner.iter()) {
            assert!(Bytes::from(*e).eq(&p.inner));
        }
        assert_eq!(parsed.to_string().as_str(), s);
    }
}
