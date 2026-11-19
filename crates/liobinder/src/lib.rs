pub mod driver;
pub mod ioc;
pub mod parcel;
pub mod servicemanager;

pub use driver::{probe, BinderDevice, BinderError, ProbeOutcome};
pub use parcel::{ParcelReader, ParcelWriter};

pub const BINDERFS_CONTROL: &str = "/dev/binderfs/binder-control";
pub const BINDER_LEGACY: &str = "/dev/binder";
