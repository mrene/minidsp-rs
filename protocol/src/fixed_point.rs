use std::fmt::{self, Display};

#[derive(Clone, Copy)]
pub struct FixedPoint(u32);

impl FixedPoint {
    pub fn from_db(db: f32) -> Self {
        FixedPoint::from_f32(10f64.powf(db as f64 / 20f64) as f32)
    }

    pub fn to_db(&self) -> f32 {
        let linear = self.to_f32() as f64;
        let db = (20f64 * linear.log10()) as f32;
        (db * 100f32).round() / 100f32
    }

    pub fn from_u32(val: u32) -> Self {
        Self(val)
    }

    pub fn from_f32(val: f32) -> Self {
        let encoded = val as f64 * (1 << 23) as f64;
        let encoded = encoded + ((1 << 27) as f64);
        let encoded = encoded as i64 as u32;
        let mut encoded = encoded ^ 0x0800_0000;

        // if encoded & 0x0800_0000 != 0 {
        //     encoded |= 0xF000_0000;
        // }

        if encoded & 0xF000_0000 == 0xF000_0000 {
            encoded &= 0x0FFF_FFFF
        }

        Self(encoded)
    }

    pub fn to_f32(&self) -> f32 {
        let val = self.0 ^ 0x0800_0000;
        let sub = (val.wrapping_sub(1 << 27)) as i32 as f32;
        let decoded = sub / ((1 << 23) as f32);
        decoded as f32
    }

    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

impl From<f32> for FixedPoint {
    fn from(val: f32) -> Self {
        Self::from_f32(val)
    }
}

impl From<FixedPoint> for f32 {
    fn from(fp: FixedPoint) -> Self {
        fp.to_f32()
    }
}

impl Display for FixedPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f32())
    }
}

impl fmt::Debug for FixedPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FixedPoint").field(&self.to_f32()).finish()
    }
}

#[cfg(test)]
mod test {
    use crate::FixedPoint;

    #[test]
    fn test_codec() {
        use super::*;

        let values: &[(f32, u32)] = &[
            (-64., 0xe0_00_00_00),
            (0.05, 0x00_06_66_66),
            (0.025, 0x00_03_33_33),
            (0.0125, 0x00_01_99_99),
            (0.125, 0x00_10_00_00),
            (0.0625, 0x00_08_00_00),
            (1.0, 0x00_80_00_00),
            (-1.0, 0x0f_80_00_00),
            (-0.05, 0x0f_f9_99_99),
            (-0.025, 0x0f_fc_cc_cc),
            (-0.0125, 0x0f_fe_66_66),
            (-0.125, 0x0f_f0_00_00),
            (-0.062, 0x0f_f8_10_62),
            (64., 0x20_00_00_00),
            (128., 0x40_00_00_00),
            (-128., 0xc0_00_00_00),
            (-111., 0xd8_80_00_00),
        ];

        for &(val, hex) in values {
            let enc = FixedPoint::from(val).to_u32();
            let dec = FixedPoint::from_u32(enc).to_f32();

            // println!("val={} hex={:#x?} enc={:#x?} dec={}", val, hex, enc, dec);
            assert!(
                (hex as i32 - enc as i32).abs() < 2,
                "{:x} and {:x} differ too much",
                hex,
                enc
            );
            assert!(
                (val - dec).abs() < 1e-5,
                "{} and {} differ too much",
                val,
                dec
            )
        }
    }

    #[test]
    fn test_db() {
        let values: &[(f32, u32)] = &[
            (12.0, 0x01_fd_93_c1),
            (6.0, 0x00_ff_64_c1),
            (3.1, 0x00_b6_e5_ff),
            (1.0, 0x00_8f_9e_4c),
            (0.5, 0x00_87_95_a0),
            (0.3, 0x00_84_7f_89),
            (-0.3, 0x00_7b_a7_8e),
            (-0.5, 0x00_78_d6_fc),
            (-3.1, 0x00_59_94_6c),
            (-6.0, 0x00_40_26_e7),
            (-12.0, 0x00_20_26_f3),
            (-72.0, 0x00_00_08_3b),
        ];

        for &(val, hex) in values.iter() {
            let enc = FixedPoint::from_db(val).0;
            let dec = FixedPoint::from_u32(hex).to_db();

            // println!(
            //     "val={} hex={:x?} enc={:x?} dec={}",
            //     val,
            //     hex,
            //     enc,
            //     dec
            // );

            let enc_delta = ((enc as i64) - (hex as i64)).abs();
            let dec_delta = (val - dec).abs();
            assert!(enc_delta < 2 && dec_delta < 1e-5);
        }
    }
}
