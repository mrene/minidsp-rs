//! Device discovery
//! Maintains an up-to-date list of devices that can be reached,
//! clears up devices after they haven't been seen in 5 minutes

pub mod tasks;
mod registry;
use std::time;

pub use registry::{Registry, DiscoveryEvent};

pub trait DeviceMediator {

}
