//! ALSA card detection for MiniDSP devices
//!
//! This module detects which ALSA sound card corresponds to a MiniDSP device
//! by matching the USB device information with ALSA card information.

#[cfg(target_os = "linux")]
use std::fs;

/// Detect the ALSA card name (e.g., "hw:2") for a MiniDSP device
///
/// Searches /proc/asound/cards for a card matching the device product name
#[cfg(target_os = "linux")]
pub fn detect_card_for_device(device_product_name: &str) -> Option<String> {
    // Read /proc/asound/cards
    let cards_content = fs::read_to_string("/proc/asound/cards").ok()?;

    // Parse each line looking for the device name
    // Format:
    //  0 [PCH            ]: HDA-Intel - HDA Intel PCH
    //  2 [DDRC24         ]: USB-Audio - DDRC-24
    for line in cards_content.lines() {
        // Check if this line contains the device name
        // Lines with card info start with a space and card number
        if line.trim_start().starts_with(char::is_numeric) {
            // Extract card number and name
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                // Get card number from first part (e.g., " 2 [DDRC24")
                let card_num_part = parts[0].trim();
                if let Some(card_num_str) = card_num_part.split_whitespace().next() {
                    // Get description from second part (e.g., " USB-Audio - DDRC-24")
                    let description = parts[1].trim();

                    // Check if description contains the device name
                    // Handle variations like "DDRC-24", "DDRC24", "miniDSP DDRC-24"
                    let device_name_normalized = device_product_name.replace('-', "").replace(' ', "").to_lowercase();
                    let description_normalized = description.replace('-', "").replace(' ', "").to_lowercase();

                    if description_normalized.contains(&device_name_normalized) {
                        log::info!(
                            "Detected MiniDSP device '{}' on ALSA card {} ({})",
                            device_product_name,
                            card_num_str,
                            description
                        );
                        return Some(format!("hw:{}", card_num_str));
                    }
                }
            }
        }
    }

    log::warn!(
        "Could not detect ALSA card for device '{}', will use default",
        device_product_name
    );
    None
}

/// Parse card number from ALSA card string format
///
/// Extracts the numeric card number from strings like "hw:0", "hw:2", etc.
///
/// # Examples
/// ```
/// # use minidsp_daemon::alsa_card_detect::parse_card_number;
/// assert_eq!(parse_card_number("hw:0"), Some(0));
/// assert_eq!(parse_card_number("hw:2"), Some(2));
/// assert_eq!(parse_card_number("default"), None);
/// ```
pub fn parse_card_number(card_string: &str) -> Option<u32> {
    card_string
        .strip_prefix("hw:")
        .and_then(|s| s.parse::<u32>().ok())
}

#[cfg(not(target_os = "linux"))]
pub fn detect_card_for_device(_device_product_name: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_detection() {
        // This is a unit test that would work if /proc/asound/cards exists
        // In practice, this is tested manually on Linux systems
        let result = detect_card_for_device("DDRC-24");
        // No assertion needed - this is a smoke test for systems with hardware
        let _ = result; // Suppress unused variable warning
    }

    #[test]
    fn test_parse_card_number() {
        assert_eq!(parse_card_number("hw:0"), Some(0));
        assert_eq!(parse_card_number("hw:1"), Some(1));
        assert_eq!(parse_card_number("hw:2"), Some(2));
        assert_eq!(parse_card_number("hw:123"), Some(123));
        assert_eq!(parse_card_number("default"), None);
        assert_eq!(parse_card_number("hw:"), None);
        assert_eq!(parse_card_number("hw:abc"), None);
        assert_eq!(parse_card_number(""), None);
        assert_eq!(parse_card_number("plughw:0"), None); // Only hw: prefix supported
    }
}
