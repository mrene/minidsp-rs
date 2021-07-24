use bytes::Buf;
use thiserror::Error;

macro_rules! try_def {
    ($meth:ident, $ty:ty) => {
        fn $meth(&mut self) -> Result<$ty, TryBufError>;
    };
}

macro_rules! try_impl {
    ($meth:ident, $inner:ident, $ty:ty, $len:literal) => {
        fn $meth(&mut self) -> Result<$ty, TryBufError> {
            if self.remaining() < $len {
                Err(TryBufError::InvalidLength {
                    remaining: self.remaining(),
                    required: $len,
                })
            } else {
                Ok(self.$inner())
            }
        }
    };
}

#[cfg_attr(feature = "debug", derive(Debug, Error))]
#[derive(Clone)]
pub enum TryBufError {
    #[cfg_attr(
        feature = "debug",
        error("invalid read length, remaining: {remaining}, required: {required}")
    )]
    InvalidLength { remaining: usize, required: usize },
}

pub trait TryBuf {
    try_def!(try_get_u8, u8);
    try_def!(try_get_i8, i8);

    try_def!(try_get_u16, u16);
    try_def!(try_get_u16_le, u16);
    try_def!(try_get_i16, i16);
    try_def!(try_get_i16_le, i16);

    try_def!(try_get_u32, u32);
    try_def!(try_get_u32_le, u32);
    try_def!(try_get_i32, i32);
    try_def!(try_get_i32_le, i32);

    try_def!(try_get_u64, u64);
    try_def!(try_get_u64_le, u64);
    try_def!(try_get_i64, i64);
    try_def!(try_get_i64_le, i64);

    try_def!(try_get_f32, f32);
    try_def!(try_get_f32_le, f32);

    try_def!(try_get_f64, f64);
    try_def!(try_get_f64_le, f64);
}

impl<T> TryBuf for T
where
    T: Buf,
{
    try_impl!(try_get_u8, get_u8, u8, 1);
    try_impl!(try_get_i8, get_i8, i8, 1);

    try_impl!(try_get_u16, get_u16, u16, 2);
    try_impl!(try_get_u16_le, get_u16_le, u16, 2);
    try_impl!(try_get_i16, get_i16, i16, 2);
    try_impl!(try_get_i16_le, get_i16_le, i16, 2);

    try_impl!(try_get_u32, get_u32, u32, 4);
    try_impl!(try_get_u32_le, get_u32_le, u32, 4);
    try_impl!(try_get_i32, get_i32, i32, 4);
    try_impl!(try_get_i32_le, get_i32_le, i32, 4);

    try_impl!(try_get_u64, get_u64, u64, 8);
    try_impl!(try_get_u64_le, get_u64_le, u64, 8);
    try_impl!(try_get_i64, get_i64, i64, 8);
    try_impl!(try_get_i64_le, get_i64_le, i64, 8);

    try_impl!(try_get_f32, get_f32, f32, 4);
    try_impl!(try_get_f32_le, get_f32_le, f32, 4);

    try_impl!(try_get_f64, get_f64, f64, 8);
    try_impl!(try_get_f64_le, get_f64_le, f64, 8);
}
