//! ALSA mixer integration for MiniDSP volume control
//!
//! MiniDSP devices expose master volume (-127 to 0 dB) ONLY through the
//! minidsp-rs daemon's proprietary USB protocol. The USB audio interface
//! provides audio streaming but NO writable volume controls.
//!
//! This module creates an ALSA integration layer that:
//! - Creates a writable ALSA control (softvol or virtual)
//! - Syncs bidirectionally with MiniDSP hardware volume
//! - Enables system audio control (desktop mixers, media keys)
//!
//! Note: Main gain is a separate DSP parameter not exposed through USB.

#[cfg(target_os = "linux")]
use std::sync::Arc;

#[cfg(target_os = "linux")]
use alsa::mixer::{MilliBel, Mixer, Selem, SelemChannelId, SelemId};

#[cfg(target_os = "linux")]
use minidsp::Gain;

#[cfg(target_os = "linux")]
use tokio::sync::Mutex;

#[cfg(target_os = "linux")]
use crate::alsa_ctl_ffi::VirtualControl;

/// ALSA Mixer Manager - handles bidirectional volume synchronization
#[cfg(target_os = "linux")]
pub struct AlsaMixerManager {
    mixer: Arc<Mutex<Option<Mixer>>>,
    card_name: String,
    control_name: String,
    use_virtual: bool,
    sync_interval_ms: u64,
}

#[cfg(target_os = "linux")]
impl AlsaMixerManager {
    /// Create a new ALSA mixer manager
    ///
    /// # Arguments
    /// * `card_name` - ALSA card name (e.g., "default", "hw:0")
    /// * `control_name` - Name for the control (virtual if created, or existing to map to)
    /// * `use_virtual` - Whether to attempt virtual control creation
    /// * `sync_interval_ms` - Sync interval in milliseconds (defaults to 100ms)
    pub fn new(
        card_name: Option<String>,
        control_name: Option<String>,
        use_virtual: bool,
        sync_interval_ms: Option<u64>,
    ) -> Self {
        Self {
            mixer: Arc::new(Mutex::new(None)),
            card_name: card_name.unwrap_or_else(|| "default".to_string()),
            control_name: control_name.unwrap_or_else(|| "MiniDSP".to_string()),
            use_virtual,
            sync_interval_ms: sync_interval_ms.unwrap_or(100),
        }
    }

    /// Initialize the ALSA mixer connection
    pub fn initialize(&mut self) -> anyhow::Result<()> {
        // Try to create virtual control if enabled
        if self.use_virtual {
            log::info!(
                "Attempting to create virtual ALSA control '{}'",
                self.control_name
            );

            // MiniDSP volume range: -127dB to 0dB
            // ALSA wants centibels (hundredths of dB)
            let min_cb = -12700i64; // -127.00 dB
            let max_cb = 0i64;       // 0.00 dB
            let step_cb = 10i64;     // 0.10 dB steps

            match VirtualControl::create(
                &self.card_name,
                &self.control_name,
                min_cb,
                max_cb,
                step_cb,
            ) {
                Ok(_vc) => {
                    // Virtual control created successfully
                    // It will remain in ALSA even after _vc is dropped
                    log::info!(
                        "Virtual ALSA control '{}' created successfully",
                        self.control_name
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Could not create virtual ALSA control '{}': {}",
                        self.control_name,
                        e
                    );
                    log::info!("Falling back to mapping to existing controls");
                }
            }
        } else {
            log::info!("Virtual control creation disabled, using existing controls");
        }

        // Open the mixer (works for both virtual and existing controls)
        let mixer = Mixer::new(&self.card_name, false)?;

        log::info!(
            "ALSA mixer initialized on card '{}' with control '{}'",
            self.card_name,
            self.control_name
        );

        // Verify we can find a suitable control
        if let Err(e) = self.ensure_control_exists(&mixer) {
            log::warn!("Control verification: {}", e);
        }

        // Store the mixer (this is safe because we only access it from the sync task)
        futures::executor::block_on(async {
            *self.mixer.lock().await = Some(mixer);
        });
        Ok(())
    }

    /// Get configured sync interval in milliseconds
    pub fn sync_interval_ms(&self) -> u64 {
        self.sync_interval_ms
    }

    /// Ensure the MiniDSP control exists, create if necessary
    fn ensure_control_exists(&self, mixer: &Mixer) -> anyhow::Result<()> {
        // Check if control already exists
        let selem_id = SelemId::new(&self.control_name, 0);

        if let Some(_selem) = mixer.find_selem(&selem_id) {
            log::debug!("Found existing ALSA control: {}", self.control_name);
            return Ok(());
        }

        // Control doesn't exist
        // Virtual control creation is attempted in initialize() if enabled
        // This method just verifies that a suitable control exists
        log::warn!(
            "Control '{}' not found, will use default playback control",
            self.control_name
        );

        Ok(())
    }

    /// Set ALSA mixer volume from MiniDSP gain value
    ///
    /// Uses ALSA's native dB API for direct conversion without percentage intermediate
    /// Sets both L/R channels to the same value (mono)
    pub async fn set_volume_from_minidsp(&self, gain: Gain) -> anyhow::Result<()> {
        let mixer_guard = self.mixer.lock().await;
        let mixer = mixer_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ALSA mixer not initialized"))?;

        // Find a suitable playback control
        let selem = self.find_playback_control(mixer)?;

        // Get volume range
        let (vol_min, vol_max) = selem.get_playback_volume_range();

        // Try to get dB range to check if dB is actually supported
        let (db_min, db_max) = selem.get_playback_db_range();

        log::debug!(
            "ALSA control ranges: vol={}..{}, dB={}..{}",
            vol_min, vol_max, db_min.0, db_max.0
        );

        // Check if dB range is valid (not 0 to 0, and min < max)
        let has_valid_db_range = db_min.0 != db_max.0 && db_min.0 < db_max.0;

        if has_valid_db_range {
            // ALSA uses MilliBel (1/100 dB), MiniDSP uses dB
            // Convert MiniDSP dB to MilliBel
            let target_mb = MilliBel::from_db(gain.0);

            // Clamp to ALSA's range
            let clamped_mb = MilliBel(target_mb.0.clamp(db_min.0, db_max.0));

            // Calculate what raw volume corresponds to our target dB
            let db_range = (db_max.0 - db_min.0) as f64;
            let vol_range = (vol_max - vol_min) as f64;
            let db_offset = (clamped_mb.0 - db_min.0) as f64;
            let vol_value_unclamped = vol_min + ((db_offset / db_range) * vol_range) as i64;

            // IMPORTANT: Clamp to actual raw volume range, not just dB range
            // Some controls have raw ranges that extend beyond their reported dB range
            let vol_value = vol_value_unclamped.clamp(vol_min, vol_max);

            log::debug!(
                "Setting ALSA volume: {}dB -> {}mB -> raw {} (clamped to {}..{})",
                gain.0, clamped_mb.0, vol_value, vol_min, vol_max
            );

            // Set volume on both stereo channels
            Self::set_stereo_volume(&selem, vol_value)?;

            log::debug!(
                "Set ALSA volume to {}dB (raw: {}, both channels)",
                gain.0, vol_value
            );
        } else {
            // Fallback to percentage-based method if dB not supported
            let percentage = self.db_to_percentage(gain.0);
            let target_value = vol_min + ((vol_max - vol_min) as f32 * percentage / 100.0) as i64;

            // Set volume on both stereo channels
            Self::set_stereo_volume(&selem, target_value)?;

            log::debug!(
                "Set ALSA volume: {}dB -> {}% (raw: {}, both channels)",
                gain.0, percentage, target_value
            );
        }

        Ok(())
    }

    /// Set ALSA mixer mute state
    pub async fn set_mute_from_minidsp(&self, muted: bool) -> anyhow::Result<()> {
        let mixer_guard = self.mixer.lock().await;
        let mixer = mixer_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ALSA mixer not initialized"))?;

        let selem = self.find_playback_control(mixer)?;

        // Set mute state on both stereo channels
        Self::set_stereo_mute(&selem, muted)?;
        log::debug!("Set ALSA mute state to {}", muted);

        Ok(())
    }

    /// Get current ALSA volume and convert to MiniDSP gain
    ///
    /// Uses ALSA's native dB API when available for accurate conversion
    /// Reads both L/R channels and averages them to preserve balance
    pub async fn get_volume_as_minidsp(&self) -> anyhow::Result<Gain> {
        let mixer_guard = self.mixer.lock().await;
        let mixer = mixer_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ALSA mixer not initialized"))?;

        // Refresh mixer state to get latest values
        if let Err(e) = mixer.handle_events() {
            log::debug!("handle_events error: {}", e);
            // Continue anyway - we'll use cached state
        }

        let selem = self.find_playback_control(mixer)?;

        // Try to get volume in dB directly - average both channels
        match (
            selem.get_playback_vol_db(SelemChannelId::FrontLeft),
            selem.get_playback_vol_db(SelemChannelId::FrontRight),
        ) {
            (Ok(left_mb), Ok(right_mb)) => {
                // Average both channels to get overall volume (preserves balance)
                let avg_mb = (left_mb.0 + right_mb.0) / 2;
                let db = avg_mb as f32 / 100.0;

                log::debug!("Read ALSA volume: L={}mB R={}mB avg={}dB", left_mb.0, right_mb.0, db);

                Ok(Gain(db))
            }
            _ => {
                // Fallback to percentage-based conversion if dB not supported
                let (min, max) = selem.get_playback_volume_range();

                // Get average volume from both stereo channels
                let avg_volume = Self::get_stereo_volume(&selem)?;

                let percentage = ((avg_volume - min) as f32 / (max - min) as f32) * 100.0;
                let db = self.percentage_to_db(percentage);

                log::debug!(
                    "Read ALSA volume: avg={} = {}% = {}dB (no dB support)",
                    avg_volume,
                    percentage as i32,
                    db
                );

                Ok(Gain(db))
            }
        }
    }

    /// Get current ALSA mute state
    pub async fn get_mute_state(&self) -> anyhow::Result<bool> {
        let mixer_guard = self.mixer.lock().await;
        let mixer = mixer_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ALSA mixer not initialized"))?;

        let selem = self.find_playback_control(mixer)?;

        if selem.has_playback_switch() {
            let switch_state = selem.get_playback_switch(SelemChannelId::FrontLeft)?;
            Ok(switch_state == 0)
        } else {
            Ok(false)
        }
    }

    /// Find the first suitable playback control
    fn find_playback_control<'a>(&self, mixer: &'a Mixer) -> anyhow::Result<Selem<'a>> {
        // Try to find our named control first
        let selem_id = SelemId::new(&self.control_name, 0);
        if let Some(selem) = mixer.find_selem(&selem_id) {
            return Ok(selem);
        }

        // Fall back to common control names
        for name in &["Master", "PCM", "Speaker", "Headphone"] {
            let selem_id = SelemId::new(name, 0);
            if let Some(selem) = mixer.find_selem(&selem_id) {
                if selem.has_playback_volume() {
                    log::debug!("Using ALSA control: {}", name);
                    return Ok(selem);
                }
            }
        }

        // Last resort: iterate through all possible simple element controls
        // Since we can't directly iterate, we'll just return an error
        // and rely on the fallback names above
        Err(anyhow::anyhow!(
            "No suitable playback control found. Available controls can be listed with 'amixer scontrols'"
        ))
    }

    /// Set volume on both stereo channels
    fn set_stereo_volume(selem: &Selem, value: i64) -> anyhow::Result<()> {
        // Try set_playback_volume_all first (for joined channels)
        if let Err(_) = selem.set_playback_volume_all(value) {
            // Fallback to individual channels
            for channel in &[SelemChannelId::FrontLeft, SelemChannelId::FrontRight] {
                if selem.has_playback_channel(*channel) {
                    selem.set_playback_volume(*channel, value)?;
                }
            }
        }
        Ok(())
    }

    /// Set mute state on both stereo channels
    fn set_stereo_mute(selem: &Selem, muted: bool) -> anyhow::Result<()> {
        if !selem.has_playback_switch() {
            return Ok(()); // No mute control available
        }

        let switch_value = if muted { 0 } else { 1 };
        for channel in &[SelemChannelId::FrontLeft, SelemChannelId::FrontRight] {
            if selem.has_playback_channel(*channel) {
                selem.set_playback_switch(*channel, switch_value)?;
            }
        }
        Ok(())
    }

    /// Get average volume from both stereo channels
    fn get_stereo_volume(selem: &Selem) -> anyhow::Result<i64> {
        let (min, _max) = selem.get_playback_volume_range();
        let left = selem.get_playback_volume(SelemChannelId::FrontLeft).unwrap_or(min);
        let right = selem.get_playback_volume(SelemChannelId::FrontRight).unwrap_or(left);
        Ok((left + right) / 2)
    }

    /// Convert dB to percentage (0-100)
    /// MiniDSP range: -127 dB (min) to 0 dB (max)
    fn db_to_percentage(&self, db: f32) -> f32 {
        // Clamp to valid range
        let db = db.clamp(Gain::MIN, Gain::MAX);

        // Linear mapping: -127dB = 0%, 0dB = 100%
        ((db - Gain::MIN) / (Gain::MAX - Gain::MIN)) * 100.0
    }

    /// Convert percentage (0-100) to dB
    fn percentage_to_db(&self, percentage: f32) -> f32 {
        // Clamp to valid range
        let percentage = percentage.clamp(0.0, 100.0);

        // Linear mapping: 0% = -127dB, 100% = 0dB
        Gain::MIN + (percentage / 100.0) * (Gain::MAX - Gain::MIN)
    }
}

/// Bidirectional synchronization task
///
/// Monitors ALSA mixer for changes and updates MiniDSP accordingly,
/// and monitors MiniDSP for changes and updates ALSA.
#[cfg(target_os = "linux")]
pub async fn sync_task(
    mixer: Arc<AlsaMixerManager>,
    device_manager: Arc<crate::device_manager::DeviceManager>,
) -> anyhow::Result<()> {
    use tokio::time::{interval, Duration};

    let sync_interval = mixer.sync_interval_ms();
    log::info!("ALSA sync task starting with {}ms interval", sync_interval);
    let mut poll_interval = interval(Duration::from_millis(sync_interval));
    let mut last_minidsp_volume: Option<Gain> = None;
    let mut last_minidsp_mute: Option<bool> = None;
    let mut last_alsa_volume: Option<Gain> = None;
    let mut last_alsa_mute: Option<bool> = None;
    let mut retry_delay = Duration::from_millis(100);
    let max_retry_delay = Duration::from_secs(10);

    loop {
        poll_interval.tick().await;

        // Get device with exponential backoff on failure
        let dsp = match device_manager.get_minidsp(0).await {
            Some(d) => {
                retry_delay = Duration::from_millis(100); // Reset on success
                d
            }
            None => {
                if retry_delay == Duration::from_millis(100) {
                    log::debug!("MiniDSP device not available, retrying...");
                }
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(max_retry_delay);
                continue;
            }
        };

        // Get current MiniDSP status
        let minidsp_status = match dsp.get_master_status().await {
            Ok(status) => status,
            Err(e) => {
                log::warn!("Failed to get MiniDSP status: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        let current_minidsp_volume = minidsp_status.volume;
        let current_minidsp_mute = minidsp_status.mute;

        // Get current ALSA state
        let current_alsa_volume = match mixer.get_volume_as_minidsp().await {
            Ok(v) => Some(v),
            Err(e) => {
                log::debug!("Failed to read ALSA volume: {}", e);
                None
            }
        };
        let current_alsa_mute = match mixer.get_mute_state().await {
            Ok(m) => Some(m),
            Err(e) => {
                log::debug!("Failed to read ALSA mute: {}", e);
                None
            }
        };

        // Determine which direction to sync based on what changed
        // Priority: MiniDSP changes take precedence over ALSA changes
        let minidsp_volume_changed = current_minidsp_volume != last_minidsp_volume && current_minidsp_volume.is_some();
        let alsa_volume_changed = current_alsa_volume != last_alsa_volume && current_alsa_volume.is_some();
        let minidsp_mute_changed = current_minidsp_mute != last_minidsp_mute && current_minidsp_mute.is_some();
        let alsa_mute_changed = current_alsa_mute != last_alsa_mute && current_alsa_mute.is_some();

        // Sync MiniDSP -> ALSA volume (takes priority)
        if minidsp_volume_changed {
            let volume = current_minidsp_volume.unwrap();
            // Only sync if the difference is significant (> 0.3 dB to account for rounding)
            let should_sync = match current_alsa_volume {
                Some(alsa_vol) => (volume.0 - alsa_vol.0).abs() > 0.3,
                None => true,
            };

            if should_sync {
                if let Err(e) = mixer.set_volume_from_minidsp(volume).await {
                    log::warn!("Failed to sync volume to ALSA: {}", e);
                } else {
                    log::debug!("Synced MiniDSP -> ALSA volume: {}dB", volume.0);
                    // Read back actual ALSA value after sync (may differ due to quantization)
                    if let Ok(actual_alsa) = mixer.get_volume_as_minidsp().await {
                        last_alsa_volume = Some(actual_alsa);
                        log::debug!("  Actual ALSA value after sync: {}dB", actual_alsa.0);
                    }
                }
            }
            // Always update MiniDSP last value
            last_minidsp_volume = current_minidsp_volume;
            // Update ALSA last value if we didn't sync (if we did, it's already updated above)
            if !should_sync && current_alsa_volume.is_some() {
                last_alsa_volume = current_alsa_volume;
            }
        }
        // Sync ALSA -> MiniDSP volume (only if MiniDSP didn't change)
        else if alsa_volume_changed {
            let volume = current_alsa_volume.unwrap();
            // Only sync if the difference is significant (> 0.3 dB to account for rounding)
            let should_sync = match current_minidsp_volume {
                Some(minidsp_vol) => (volume.0 - minidsp_vol.0).abs() > 0.3,
                None => true,
            };

            if should_sync {
                if let Err(e) = dsp.set_master_volume(volume).await {
                    log::warn!("Failed to sync volume to MiniDSP: {}", e);
                } else {
                    log::debug!("Synced ALSA -> MiniDSP volume: {}dB", volume.0);
                    // Read back actual MiniDSP value after sync (may differ due to quantization)
                    if let Ok(status) = dsp.get_master_status().await {
                        if let Some(actual_minidsp) = status.volume {
                            last_minidsp_volume = Some(actual_minidsp);
                            log::debug!("  Actual MiniDSP value after sync: {}dB", actual_minidsp.0);
                        }
                    }
                }
            }
            // Always update ALSA last value
            last_alsa_volume = current_alsa_volume;
            // Update MiniDSP last value if we didn't sync (if we did, it's already updated above)
            if !should_sync && current_minidsp_volume.is_some() {
                last_minidsp_volume = current_minidsp_volume;
            }
        }
        // If neither changed significantly, just update to current values
        else {
            if current_minidsp_volume.is_some() {
                last_minidsp_volume = current_minidsp_volume;
            }
            if current_alsa_volume.is_some() {
                last_alsa_volume = current_alsa_volume;
            }
        }

        // Sync MiniDSP -> ALSA mute (takes priority)
        if minidsp_mute_changed {
            let mute = current_minidsp_mute.unwrap();
            if Some(mute) != current_alsa_mute {
                if let Err(e) = mixer.set_mute_from_minidsp(mute).await {
                    log::warn!("Failed to sync mute to ALSA: {}", e);
                    // Don't update last_alsa_mute - will retry next cycle
                } else {
                    log::debug!("Synced MiniDSP -> ALSA mute: {}", mute);
                    last_alsa_mute = current_minidsp_mute; // Only update after success
                }
            }
            // Always update MiniDSP last value
            last_minidsp_mute = current_minidsp_mute;
        }
        // Sync ALSA -> MiniDSP mute (only if MiniDSP didn't change)
        else if alsa_mute_changed {
            let mute = current_alsa_mute.unwrap();
            if Some(mute) != current_minidsp_mute {
                if let Err(e) = dsp.set_master_mute(mute).await {
                    log::warn!("Failed to sync mute to MiniDSP: {}", e);
                    // Don't update last_minidsp_mute - will retry next cycle
                } else {
                    log::debug!("Synced ALSA -> MiniDSP mute: {}", mute);
                    last_minidsp_mute = current_alsa_mute; // Only update after success
                }
            }
            // Always update ALSA last value
            last_alsa_mute = current_alsa_mute;
        }
        // If neither changed, just update to current values
        else {
            if current_minidsp_mute.is_some() {
                last_minidsp_mute = current_minidsp_mute;
            }
            if current_alsa_mute.is_some() {
                last_alsa_mute = current_alsa_mute;
            }
        }
    }
}

// Stub implementations for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct AlsaMixerManager;

#[cfg(not(target_os = "linux"))]
impl AlsaMixerManager {
    pub fn new(_card_name: Option<String>, _control_name: Option<String>) -> Self {
        Self
    }

    pub async fn initialize(&self) -> anyhow::Result<()> {
        log::info!("ALSA mixer support not available on this platform");
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn sync_task(
    _mixer: std::sync::Arc<AlsaMixerManager>,
    _minidsp: std::sync::Arc<tokio::sync::Mutex<Option<minidsp::MiniDSP<'static>>>>,
) -> anyhow::Result<()> {
    // No-op on non-Linux platforms
    std::future::pending().await
}
