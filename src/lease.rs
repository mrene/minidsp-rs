use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::{Gain, MiniDSP, Source};

pub fn lease_source(minidsp: Arc<Mutex<MiniDSP>>, source: Source) -> Result<SourceLease> {
    {
        let mut minidsp = minidsp.lock().unwrap();
        minidsp.set_source(source)?;
    }

    Ok(SourceLease { minidsp })
}

pub struct SourceLease {
    minidsp: Arc<Mutex<MiniDSP>>,
}

impl Drop for SourceLease {
    fn drop(&mut self) {
        let minidsp = self.minidsp.lock();
        if let Ok(mut minidsp) = minidsp {
            if let Err(e) = minidsp.set_source(Source::Toslink) {
                eprintln!("Failed to set source back: {:?}", e)
            }
            if let Err(e) = minidsp.set_master_volume(Gain(-40.)) {
                eprintln!("Failed to set source back: {:?}", e)
            }
        }
    }
}
