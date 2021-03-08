//! Device discovery
//! Maintains an up-to-date list of devices that can be reached,
//! clears up devices after they haven't been seen in 5 minutes

mod registry;
pub mod tasks;

pub use registry::{DiscoveryEvent, Registry};

pub trait DeviceMediator {}
