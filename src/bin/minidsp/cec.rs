use std::time::Duration;

use arrayvec::ArrayVec;
use cec_rs::{
    CecCommand, CecConnection, CecConnectionCfgBuilder, CecDatapacket, CecDeviceType,
    CecDeviceTypeVec, CecLogicalAddress, CecOpcode, CecUserControlCode,
};
use log::trace;
use tokio::sync::broadcast;
use super::{MiniDSP,Gain};

pub(crate) async fn run_cec(dsp: &MiniDSP<'_>) -> Result<(), anyhow::Error> {

    // If the capacity is too high, we could endlessly queue up a bunch of `volume up` that would be all applied at once
    let (keypress_tx, mut keypress_rx) = broadcast::channel::<cec_rs::CecKeypress>(2);

    let on_key_press = move |keypress: cec_rs::CecKeypress| {
        let _ = keypress_tx.send(keypress);
    };

    let cfg = CecConnectionCfgBuilder::default()
        .port("RPI".into())
        .device_name("MiniDSP".into())
        .key_press_callback(Box::new(on_key_press))
        .command_received_callback(Box::new(on_command_received))
        .device_types(CecDeviceTypeVec::new(CecDeviceType::AudioSystem))
        .activate_source(false)
        .build()
        .unwrap();
    let connection: CecConnection = cfg.open().unwrap();
    println!("Active source: {:?}", connection.get_active_source());

    let report_status = |mute: bool, vol: u8| {
        let mut parameters = ArrayVec::new();
        let mut b: u8 = vol & 0x7F;
        if mute {
            b |= 1 << 7;
        }

        parameters.push(b);

        let audio_report = CecCommand {
            initiator: CecLogicalAddress::Audiosystem,
            destination: CecLogicalAddress::Tv,
            ack: false,
            eom: true,
            opcode: CecOpcode::ReportAudioStatus,
            parameters: CecDatapacket(parameters.clone()),
            opcode_set: true,
            transmit_timeout: Duration::from_secs(1),
        };
        connection.transmit(audio_report.into()).unwrap();
    };

    let mut vol_slider = VolumeSlider::new(-55., -5., 0);
    if let Ok(ms) = dsp.get_master_status().await {
        vol_slider.set_gain(ms.volume);
    }

    loop {
        if let Ok(keypress) = keypress_rx.recv().await {
            trace!("keypress: {:?}", keypress);
            match keypress.keycode {
                CecUserControlCode::VolumeUp => {
                    if let Ok(ms) = dsp.get_master_status().await {
                        vol_slider.set_gain(ms.volume);
                        vol_slider.inc();
                        let _ = dsp.set_master_volume(vol_slider.to_gain()).await;
                        report_status(ms.mute, vol_slider.percent);
                    }
                }
                CecUserControlCode::VolumeDown => {
                    if let Ok(ms) = dsp.get_master_status().await {
                        vol_slider.set_gain(ms.volume);
                        vol_slider.dec();
                        let _ = dsp.set_master_volume(vol_slider.to_gain()).await;
                        report_status(ms.mute, vol_slider.percent);
                    }
                }
                CecUserControlCode::Mute => {
                    if let Ok(ms) = dsp.get_master_status().await {
                        vol_slider.set_gain(ms.volume);
                        let _ = dsp.set_master_mute(!ms.mute).await;
                        report_status(ms.mute, vol_slider.percent);
                    }
                }
                _ => {}
            }
        }
    }
}

fn on_command_received(command: cec_rs::CecCommand) {
    trace!(
        "initiator={:?} destination={:?} op={:?} params={:?}",
        command.initiator, command.destination, command.opcode, command.parameters
    );
}

struct VolumeSlider {
    pub min: f32,
    pub max: f32,

    pub percent: u8,
}

impl VolumeSlider {
    pub fn new(min: f32, max: f32, percent: u8) -> Self {
        return VolumeSlider{min,max,percent}
    }
    pub fn set_gain(&mut self, gain: Gain) {
        self.percent = (100.*(gain.0 - self.min)/(self.max - self.min)) as u8
    }

    pub fn to_gain(&self) -> Gain {
        Gain((self.percent as f32 / 100.) * (self.max-self.min) + self.min)
    }

    pub fn inc(&mut self) {
        self.percent += 1;
        if self.percent > 100 {
            self.percent = 100
        }
    }

    pub fn dec(&mut self) {
        if self.percent > 0 {
            self.percent -= 1;
        }
    }
}
impl Into<Gain> for VolumeSlider {
    fn into(self) -> Gain {
        self.to_gain()
    }
}

#[cfg(test)]
mod test {
    use assert_approx_eq::assert_approx_eq;
    use super::*;

    #[test]
    fn test_slider() {
        let mut s = VolumeSlider::new(0., 100.,0);
        s.set_gain(Gain(50.));
        assert_eq!(50, s.percent);
        assert_approx_eq!(50., s.to_gain().0);

        let mut s = VolumeSlider::new(-30., 0.,0);
        s.set_gain(Gain(-15.));
        assert_eq!(50, s.percent);
        assert_approx_eq!(-15., s.to_gain().0);

        s.percent = 0;
        assert_eq!(-30., s.to_gain().0);

        s.percent = 100;
        assert_eq!(0., s.to_gain().0);
    }
}