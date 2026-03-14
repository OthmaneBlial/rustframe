mod capability;
mod error;
mod ipc;

#[cfg(feature = "desktop")]
mod desktop;

pub use capability::{FsCapability, ShellCapability, ShellCommand, ShellOutput};
#[cfg(feature = "desktop")]
pub use desktop::{EmbeddedAssets, RustFrame, RustFrameBuilder, WindowOptions};
pub use error::{Result, RuntimeError};
pub use ipc::{IpcErrorResponse, IpcRequest, IpcResponse};
