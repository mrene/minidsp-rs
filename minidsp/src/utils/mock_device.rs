//! Provides a mock device for testing purposes

use std::convert::TryInto;

use bytes::{Buf, BufMut, BytesMut};
use minidsp_protocol::{
    commands::{BytesWrap, FloatView, MemoryView, Responses},
    device::{Device, DeviceKind},
    eeprom, Commands,
};

pub struct MockDevice {
    pub hw_id: u8,
    pub dsp_version: u8,
    pub firmware_version: u8,

    /// Emulated device kind
    pub kind: DeviceKind,

    /// Specifications for the device kind being emulated
    pub spec: &'static Device,

    /// Device EEPROM memory
    pub eeprom: Vec<u8>,

    /// DSP Settings and meter values
    pub settings: Vec<u32>,

    // Delay before each response
    pub response_delay: Option<std::time::Duration>,
}

impl Default for MockDevice {
    fn default() -> MockDevice {
        MockDevice {
            // FIXME: hardcoded values for the 2x4hd
            hw_id: 10,
            dsp_version: 100,
            firmware_version: 13,

            kind: DeviceKind::default(),
            spec: &crate::device::m2x4hd::DEVICE,
            eeprom: vec![],
            settings: vec![],
            response_delay: Some(std::time::Duration::from_millis(50)),
        }
    }
}

impl MockDevice {
    pub fn new(hw_id: u8, dsp_version: u8, kind: DeviceKind) -> Self {
        let mut device = Self {
            hw_id,
            dsp_version,
            kind,
            eeprom: {
                let mut v = Vec::new();
                v.resize(u16::MAX as usize, 0xFF);
                v
            },
            settings: {
                let mut v = Vec::new();
                v.resize(u16::MAX as usize, 0);
                v
            },
            ..Default::default()
        };

        device.write_eeprom_u8(eeprom::FIRMWARE_VERSION, dsp_version);
        device.write_eeprom_u32(eeprom::TIMESTAMP, 42424242u32);
        device.write_eeprom_u16(eeprom::SOURCE, 0);
        device.write_eeprom_u8(eeprom::MASTER_VOLUME, 7);
        device.write_eeprom_u8(eeprom::MUTE, 0);
        device.write_eeprom_u16(eeprom::PRESET, 0);
        device.write_eeprom_u16(eeprom::SERIAL, 12345);

        // FIXME: what does this mean? restoring without it says the dsp is corrupted, probably some sort of version check
        device.write_eeprom_u8(0xFFA3, 0x03);

        let meters = {
            let inputs = device.spec.inputs.iter().filter_map(|i| i.meter);
            let outputs = device.spec.outputs.iter().map(|i| i.meter);
            let compressors = device
                .spec
                .outputs
                .iter()
                .filter_map(|i| i.compressor.as_ref().map(|c| c.meter).flatten());

            inputs.chain(outputs).chain(compressors)
        };

        for addr in meters {
            device.settings[addr as usize] = u32::from_le_bytes((-127f32).to_le_bytes());
        }

        device
    }

    pub fn write_eeprom_u8(&mut self, addr: u16, value: u8) {
        self.eeprom[addr as usize] = value;
    }

    pub fn write_eeprom_u16(&mut self, addr: u16, value: u16) {
        self.eeprom[addr as usize..addr as usize + 2].copy_from_slice(&value.to_be_bytes());
    }

    pub fn write_eeprom_u32(&mut self, addr: u16, value: u32) {
        self.eeprom[addr as usize..addr as usize + 4].copy_from_slice(&value.to_be_bytes());
    }

    pub fn set_serial(&mut self, value: u32) {
        let value = value.saturating_sub(900000);
        let value: u16 = value.try_into().unwrap();

        self.write_eeprom_u16(eeprom::SERIAL, value);
    }

    // Executes a command and response with the appropriate response, while updating
    // the internal state.
    pub fn execute(&mut self, cmd: &Commands) -> Responses {
        match cmd {
            Commands::ReadHardwareId => Responses::HardwareId {
                payload: {
                    let mut b = BytesMut::new();
                    b.put_u8(0x1);
                    b.put_u8(self.firmware_version);
                    b.put_u8(self.hw_id);
                    BytesWrap(b.freeze())
                },
            },
            &Commands::ReadMemory { addr, size } => {
                let addr = addr as usize;
                let size = size as usize;
                Responses::MemoryData(MemoryView {
                    base: addr as u16,
                    data: {
                        let effective_size = if addr + size > u16::MAX as usize {
                            u16::MAX as usize - addr
                        } else {
                            size
                        };

                        let mut data = BytesMut::from(&self.eeprom[addr..addr + effective_size]);
                        data.resize(size, 0xFF);
                        data.freeze()
                    },
                })
            }
            &Commands::WriteMemory { addr, ref data } => {
                let addr = addr as usize;
                let mut data = data.clone();
                let len = data.len();
                data.copy_to_slice(&mut self.eeprom[addr..addr + len]);
                Responses::Ack
            }
            &Commands::ReadFloats { addr, len } => {
                let addr = addr as usize;
                let len = len as usize;

                let view = FloatView {
                    base: addr as u16,
                    data: self.settings[addr..addr + len]
                        .iter()
                        .map(|&x| f32::from_le_bytes(x.to_le_bytes()))
                        .collect(),
                };
                Responses::FloatData(view)
            }
            &Commands::Write { addr, ref value } => {
                let addr = addr.val as usize;
                let data = value.clone().into_bytes();
                assert!(data.len() == 4);
                self.settings[addr] = u32::from_le_bytes(data.as_ref().try_into().unwrap());

                Responses::Ack
            }
            &Commands::SetConfig { config, .. } => {
                self.write_eeprom_u8(eeprom::PRESET, config);
                Responses::ConfigChanged
            }
            &Commands::SetSource { source } => {
                self.write_eeprom_u8(eeprom::SOURCE, source);
                Responses::Ack
            }
            &Commands::SetMute { value } => {
                self.write_eeprom_u8(eeprom::MUTE, value as u8);
                Responses::Ack
            }
            &Commands::SetVolume { value } => {
                self.write_eeprom_u8(eeprom::MASTER_VOLUME, value.into());
                Responses::Ack
            }
            &Commands::FirLoadStart { .. } => {
                // TODO:: Capture Fir state
                Responses::FirLoadSize {
                    size: self.spec.fir_max_taps,
                }
            }
            &Commands::Unk07 { .. } => Responses::Unk02,
            _ => Responses::Ack,
        }
    }
}
