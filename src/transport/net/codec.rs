//! A codec allowing length-delimited encoding, while preserving the pre-included length in sent messages
use anyhow::Result;
use bytes::Bytes;
use std::io;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

/// Wraps a length delimited codec but do not prepend the length when sending
/// as it is done by the upper layers
pub struct Codec {
    length_delimited: LengthDelimitedCodec,
}

impl Codec {
    pub fn new() -> Self {
        Self {
            length_delimited: LengthDelimitedCodec::builder()
                .length_field_length(1)
                .num_skip(0)
                .new_codec(),
        }
    }
}

impl Decoder for Codec {
    type Item = Bytes;
    type Error = io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.length_delimited.decode(src) {
            Ok(Some(x)) => Ok(Some(x.freeze())),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Encoder<Bytes> for Codec {
    type Error = io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        dst.extend(item);
        Ok(())
    }
}
