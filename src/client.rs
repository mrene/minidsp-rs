use crate::{
    commands::{self, Commands, FloatView, MemoryView, Value},
    transport::{MiniDSPError, SharedService},
};

pub struct Client {
    transport: SharedService,
}

impl Client {
    pub fn new(transport: SharedService) -> Self {
        Self { transport }
    }

    pub async fn roundtrip(
        &self,
        cmd: commands::Commands,
    ) -> Result<commands::Responses, MiniDSPError> {
        self.transport.lock().await.call(cmd).await
    }

    /// Wrapper for common commands
    pub async fn read_memory(&self, addr: u16, size: u8) -> Result<MemoryView, MiniDSPError> {
        self.roundtrip(Commands::ReadMemory { addr, size })
            .await?
            .into_memory_view()
    }

    pub async fn read_floats(&self, addr: u16, len: u8) -> Result<FloatView, MiniDSPError> {
        self.roundtrip(Commands::ReadFloats { addr, len })
            .await?
            .into_float_view()
    }

    pub async fn write_dsp<T: Into<Value>>(&self, addr: u16, value: T) -> Result<(), MiniDSPError> {
        self.roundtrip(Commands::Write {
            addr,
            value: value.into(),
        })
        .await?
        .into_ack()
    }
}
