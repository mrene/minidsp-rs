use anyhow::Result;
use futures::{pin_mut, StreamExt};
use log::warn;
use minidsp::transport;
use std::time;

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

pub async fn net_discovery_task(register: impl Fn(&str)) -> Result<()> 
{
    let stream = transport::net::discover().await?;
    pin_mut!(stream);
    while let Some(device) = stream.next().await {
        register(device.to_url().as_str());
    }
    Ok(())
}
