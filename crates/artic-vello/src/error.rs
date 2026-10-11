use vello::wgpu::{RequestAdapterError, RequestDeviceError};

/// Why a [`Renderer`](crate::Renderer) couldn't start.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// No GPU adapter supports the chosen API.
  #[error("no GPU adapter is available: {0}")]
  Adapter(#[from] RequestAdapterError),
  /// The adapter refused to open a device.
  #[error("couldn't open the GPU device: {0}")]
  Device(#[from] RequestDeviceError),
  /// vello couldn't build its GPU pipelines.
  #[error("couldn't prepare vello: {0}")]
  Vello(#[from] vello::Error),
}
