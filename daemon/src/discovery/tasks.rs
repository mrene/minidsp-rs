use std::{net::IpAddr, time};

use anyhow::Result;
use futures::{pin_mut, StreamExt};
use log::warn;
use minidsp::transport;

pub async fn hid_discovery_task(register: impl Fn(&str)) -> Result<()> {
    let api = transport::hid::initialize_api()?;
    loop {
        match transport::hid::discover(&api) {
            Ok(devices) => {
                for device in devices {
                    register(device.to_url().as_str());
                }
            }
            Err(e) => {
                warn!("failed to enumerate hid devices: {}", e);
            }
        }

        tokio::time::sleep(time::Duration::from_secs(5)).await;
    }
}

pub async fn net_discovery_task(register: impl Fn(&str), this_ip: Option<IpAddr>) -> Result<()> {
    let stream = transport::net::discover().await?;
    pin_mut!(stream);

    while let Some(device) = stream.next().await {
        if let Some(this_ip) = this_ip {
            if device.ip.ip() == this_ip {
                // Don't register ourselves if we're advertising
                continue;
            }
        }
        let url = device.to_url();
        register(url.as_str());
    }
    Ok(())
}
