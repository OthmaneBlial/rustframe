mod capability;
mod database;
mod error;
mod ipc;

#[cfg(feature = "desktop")]
mod desktop;

pub use capability::{
    FsBinaryContents, FsCapability, FsEntry, FsGrant, FsGrantAccess, FsGrantKind, FsWalkOptions,
    ShellCapability, ShellCommand, ShellOutput,
};
pub use database::{
    DatabaseBatchOperation, DatabaseCapability, DatabaseColumnType, DatabaseFilter,
    DatabaseFilterOp, DatabaseInfo, DatabaseListQuery, DatabaseMigrationFile, DatabaseOpenConfig,
    DatabaseOrder, DatabaseOrderDirection, DatabaseSchema, DatabaseSearchQuery, DatabaseSeedFile,
    backup_database_file, restore_database_file,
};
#[cfg(feature = "desktop")]
pub use desktop::{
    EmbeddedAssets, FrontendSecurity, FrontendTrust, RustFrame, RustFrameBuilder, WindowOptions,
};
pub use error::{Result, RuntimeError};
pub use ipc::{
    DEFAULT_MAX_IPC_REQUEST_BYTES, IpcErrorResponse, IpcRequest, IpcResponse, decode_request,
};
