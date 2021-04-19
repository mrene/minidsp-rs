use std::path::Path;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum WavReadError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("The file didn't match the device's internal sampling rate")]
    InvalidSampleRate,
    #[error("The file didn't contain any samples")]
    NoData,
}

pub fn read_wav_filter<T: AsRef<Path>>(
    filename: T,
    sample_rate: u32,
) -> Result<Vec<f32>, WavReadError> {
    let mut inp_file = std::fs::File::open(filename)?;
    let (header, data) = ::wav::read(&mut inp_file)?;

    if header.sampling_rate != sample_rate {
        return Err(WavReadError::InvalidSampleRate);
    }

    convert_data(data)
}

fn convert_data(data: wav::BitDepth) -> Result<Vec<f32>, WavReadError> {
    let samples: Vec<f32>;

    match data {
        wav::BitDepth::Eight(data) => {
            samples = data.iter().map(|x| (*x as f32 - 128.) / 128f32).collect();
        }
        wav::BitDepth::Sixteen(data) => {
            samples = data.iter().map(|x| *x as f32 / (i16::MAX as f32)).collect();
        }
        wav::BitDepth::TwentyFour(data) => {
            samples = data
                .iter()
                .map(|x| *x as f32 / ((1 << 23) as f32))
                .collect();
        }
        wav::BitDepth::Empty => {
            return Err(WavReadError::NoData);
        }
    }

    Ok(samples)
}

#[cfg(test)]
mod test {
    use std::vec;

    use wav::BitDepth;

    use super::convert_data;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn test_u8() {
        let data = BitDepth::Eight(vec![0, 255, 128]);
        let expected: Vec<f32> = vec![-1.0, 1.0, 0.0];
        let samples = convert_data(data).unwrap();

        for (sample, expected) in samples.iter().zip(expected.iter()) {
            assert_approx_eq!(*sample, *expected, 1e-2);
        }
    }

    #[test]
    fn test_i16() {
        let data = BitDepth::Sixteen(vec![i16::MIN, 0, i16::MAX]);
        let expected: Vec<f32> = vec![-1.0, 0.0, 1.0];
        let samples = convert_data(data).unwrap();

        for (sample, expected) in samples.iter().zip(expected.iter()) {
            assert_approx_eq!(*sample, *expected, 1e-2);
        }
    }

    #[test]
    fn test_i24() {
        let data = BitDepth::TwentyFour(vec![-1 << 23, 0, 1 << 23]);
        let expected: Vec<f32> = vec![-1.0, 0.0, 1.0];
        let samples = convert_data(data).unwrap();

        for (sample, expected) in samples.iter().zip(expected.iter()) {
            assert_approx_eq!(*sample, *expected, 1e-2);
        }
    }
}
