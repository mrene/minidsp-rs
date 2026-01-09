//! FFI bindings for ALSA control element creation
//!
//! The alsa crate doesn't expose control creation APIs, so we use
//! direct FFI to alsa-sys for creating virtual controls.

#[cfg(target_os = "linux")]
use alsa_sys::*;

#[cfg(target_os = "linux")]
use std::ffi::CString;

/// Virtual ALSA control element marker
///
/// Once created via create(), the control exists in ALSA and can be accessed
/// through the normal mixer interface. This is just a marker type.
#[cfg(target_os = "linux")]
pub struct VirtualControl;

#[cfg(target_os = "linux")]
impl VirtualControl {
    /// Create a new virtual ALSA control element
    ///
    /// # Arguments
    /// * `card` - ALSA card name (e.g., "hw:0", "default")
    /// * `name` - Control name (e.g., "MiniDSP")
    /// * `min` - Minimum value in centibels (-12700 for -127dB)
    /// * `max` - Maximum value in centibels (0 for 0dB)
    /// * `step` - Step size in centibels (10 for 0.1dB)
    ///
    /// # Returns
    /// Ok(Self) if control created successfully or already exists, Err otherwise
    ///
    /// # Safety
    /// This function uses FFI to call ALSA C API functions. It properly manages
    /// memory and handles errors, but relies on correct ALSA library behavior.
    pub fn create(
        card: &str,
        name: &str,
        min: i64,
        max: i64,
        step: i64,
    ) -> anyhow::Result<Self> {
        unsafe {
            let card_cstr = CString::new(card)?;
            let name_cstr = CString::new(name)?;

            let mut handle: *mut snd_ctl_t = std::ptr::null_mut();
            let mut elem_info: *mut snd_ctl_elem_info_t = std::ptr::null_mut();

            // Open control device
            let ret = snd_ctl_open(&mut handle, card_cstr.as_ptr(), 0);
            if ret < 0 {
                return Err(anyhow::anyhow!(
                    "Failed to open ALSA control device '{}': {}",
                    card,
                    std::io::Error::from_raw_os_error(-ret)
                ));
            }

            // Allocate element info
            let ret = snd_ctl_elem_info_malloc(&mut elem_info);
            if ret < 0 {
                snd_ctl_close(handle);
                return Err(anyhow::anyhow!("Failed to allocate element info"));
            }

            // Set element interface and name
            snd_ctl_elem_info_set_interface(elem_info, SND_CTL_ELEM_IFACE_MIXER);
            snd_ctl_elem_info_set_name(elem_info, name_cstr.as_ptr());

            // Check if element already exists
            let info_ret = snd_ctl_elem_info(handle, elem_info);
            if info_ret >= 0 {
                log::info!("Virtual control '{}' already exists, reusing it", name);
                snd_ctl_elem_info_free(elem_info);
                snd_ctl_close(handle);
                return Ok(Self);
            }

            // Create the control element
            // Parameters: handle, info, count (1 element), member_count (2 channels), min, max, step
            let ret = snd_ctl_add_integer_elem_set(
                handle,
                elem_info,
                1, // 1 element (mono control set)
                2, // 2 channels (L/R)
                min,
                max,
                step,
            );

            if ret < 0 {
                snd_ctl_elem_info_free(elem_info);
                snd_ctl_close(handle);
                return Err(anyhow::anyhow!(
                    "Failed to create virtual control '{}': {}",
                    name,
                    std::io::Error::from_raw_os_error(-ret)
                ));
            }

            log::info!(
                "Successfully created virtual ALSA control '{}' (range: {}dB to {}dB)",
                name,
                min as f64 / 100.0,
                max as f64 / 100.0
            );

            snd_ctl_elem_info_free(elem_info);
            snd_ctl_close(handle);

            Ok(Self)
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for VirtualControl {
    fn drop(&mut self) {
        // Resources are cleaned up in create() after control creation
        // This is just a marker struct
    }
}

// Stub implementations for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct VirtualControl;

#[cfg(not(target_os = "linux"))]
impl VirtualControl {
    pub fn create(_card: &str, _name: &str, _min: i64, _max: i64, _step: i64) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
