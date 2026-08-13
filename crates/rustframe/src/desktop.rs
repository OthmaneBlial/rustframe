use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, VecDeque},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use arboard::Clipboard;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use mime_guess::MimeGuess;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder, WindowId},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use wry::{
    NewWindowResponse, WebView, WebViewBuilder,
    http::{HeaderValue, Request, Response, header::CONTENT_TYPE},
};

use crate::{
    DatabaseBatchOperation, DatabaseCapability, DatabaseListQuery, DatabaseMigrationFile,
    DatabaseOpenConfig, DatabaseSchema, DatabaseSearchQuery, DatabaseSeedFile, FsCapability,
    FsGrantAccess, FsWalkOptions, IpcErrorResponse, IpcRequest, IpcResponse, Result, RuntimeError,
    ShellCapability, ShellCommand, decode_request,
};

const APP_URL: &str = "app://localhost/";
const RUSTFRAME_BRIDGE_SCRIPT: &str = include_str!("bridge.js");
const PRIMARY_WINDOW_ID: &str = "main";
const MAX_OPEN_WINDOWS: usize = 16;
const MAX_IPC_REQUEST_BYTES: usize = crate::DEFAULT_MAX_IPC_REQUEST_BYTES;
const WINDOW_STATE_FILE_NAME: &str = "window-state.json";
const FILESYSTEM_GRANTS_FILE_NAME: &str = "filesystem-grants.json";

#[derive(Default)]
struct SensitiveRateLimiter {
    attempts: HashMap<(String, String), VecDeque<Instant>>,
}

impl SensitiveRateLimiter {
    fn check(&mut self, window_id: &str, method: &str) -> Result<()> {
        let Some((limit, interval)) = sensitive_rate_policy(method) else {
            return Ok(());
        };
        let now = Instant::now();
        let attempts = self
            .attempts
            .entry((window_id.to_string(), method.to_string()))
            .or_default();
        while attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) >= interval)
        {
            attempts.pop_front();
        }
        if attempts.len() >= limit {
            return Err(RuntimeError::RateLimited(format!(
                "window '{window_id}' exceeded the limit for '{method}'; retry after {} seconds",
                interval.as_secs()
            )));
        }
        attempts.push_back(now);
        Ok(())
    }

    fn close_window(&mut self, window_id: &str) {
        self.attempts
            .retain(|(candidate, _), _| candidate != window_id);
    }
}

fn sensitive_rate_policy(method: &str) -> Option<(usize, Duration)> {
    match method {
        "db.backup" | "db.restore" => Some((5, Duration::from_secs(60))),
        "shell.exec" => Some((30, Duration::from_secs(60))),
        "fs.requestGrant"
        | "fs.revokeGrant"
        | "fs.watch"
        | "fs.unwatch"
        | "dialog.openFile"
        | "dialog.openFiles"
        | "dialog.openDirectory"
        | "dialog.saveText"
        | "dialog.saveBinary" => Some((20, Duration::from_secs(60))),
        "window.open" => Some((30, Duration::from_secs(60))),
        _ => None,
    }
}

pub trait EmbeddedAssets {
    fn get(path: &str) -> Option<Cow<'static, [u8]>>;
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowOptions {
    pub title: String,
    pub width: f64,
    pub height: f64,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "RustFrame".into(),
            width: 980.0,
            height: 720.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FrontendTrust {
    #[default]
    LocalFirst,
    Networked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSecurity {
    pub model: FrontendTrust,
    pub database: bool,
    pub filesystem: bool,
    pub shell: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub permissions: BTreeMap<String, Vec<String>>,
}

impl Default for FrontendSecurity {
    fn default() -> Self {
        Self::local_first()
    }
}

impl FrontendSecurity {
    pub fn local_first() -> Self {
        Self {
            model: FrontendTrust::LocalFirst,
            database: true,
            filesystem: true,
            shell: true,
            permissions: BTreeMap::new(),
        }
    }

    pub fn networked() -> Self {
        Self {
            model: FrontendTrust::Networked,
            database: false,
            filesystem: false,
            shell: false,
            permissions: BTreeMap::new(),
        }
    }

    pub fn database(mut self, allowed: bool) -> Self {
        self.database = allowed;
        self
    }

    pub fn filesystem(mut self, allowed: bool) -> Self {
        self.filesystem = allowed;
        self
    }

    pub fn shell(mut self, allowed: bool) -> Self {
        self.shell = allowed;
        self
    }

    pub fn allow_window_permissions<I, S>(
        mut self,
        window: impl Into<String>,
        permissions: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions.insert(
            window.into(),
            permissions.into_iter().map(Into::into).collect(),
        );
        self
    }

    fn resolve(
        &self,
        _fs_capability: &FsCapability,
        shell_capability: &ShellCapability,
        database_capability: Option<&DatabaseCapability>,
    ) -> ResolvedFrontendSecurity {
        ResolvedFrontendSecurity {
            model: self.model,
            database: self.database && database_capability.is_some(),
            filesystem: self.filesystem,
            shell: self.shell && !shell_capability.command_names().is_empty(),
            permissions: self.permissions.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedFrontendSecurity {
    model: FrontendTrust,
    database: bool,
    filesystem: bool,
    shell: bool,
    permissions: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct RuntimeSmokeConfig {
    output_path: Option<PathBuf>,
    data_dir: Option<PathBuf>,
}

impl RuntimeSmokeConfig {
    fn from_env() -> Option<Self> {
        let enabled = env::var("RUSTFRAME_SMOKE_TEST")
            .ok()
            .map(|value| value != "0")
            .unwrap_or(false);

        enabled.then(|| Self {
            output_path: env::var_os("RUSTFRAME_SMOKE_OUTPUT").map(PathBuf::from),
            data_dir: env::var_os("RUSTFRAME_SMOKE_DATA_DIR").map(PathBuf::from),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeSmokeReport {
    app_id: Option<String>,
    launch_mode: String,
    active_dev_url: Option<String>,
    window: WindowOptions,
    security: ResolvedFrontendSecurity,
    has_index_html: bool,
    bridge_injected: bool,
    fs_roots: Vec<String>,
    shell_commands: Vec<String>,
    database: Option<crate::DatabaseInfo>,
}

pub struct RustFrame;

impl RustFrame {
    pub fn builder() -> RustFrameBuilder {
        RustFrameBuilder::default()
    }
}

#[derive(Clone, Copy)]
struct EmbeddedAssetRouter {
    fetch: fn(&str) -> Option<Cow<'static, [u8]>>,
}

impl EmbeddedAssetRouter {
    fn from_type<A: EmbeddedAssets>() -> Self {
        Self { fetch: A::get }
    }

    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        (self.fetch)(path)
    }
}

#[derive(Clone, Debug)]
struct EmbeddedDatabaseConfig {
    schema_path: String,
    seed_paths: Vec<String>,
    migration_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowRecord {
    id: String,
    title: String,
    route: String,
    width: f64,
    height: f64,
    is_primary: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedWindowState {
    title: String,
    route: String,
    width: f64,
    height: f64,
    is_primary: bool,
}

#[derive(Default)]
struct WindowStateStore {
    path: Option<PathBuf>,
    windows: BTreeMap<String, PersistedWindowState>,
}

impl WindowStateStore {
    fn load(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };

        let windows = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| {
                serde_json::from_str::<BTreeMap<String, PersistedWindowState>>(&contents).ok()
            })
            .unwrap_or_default();

        Self {
            path: Some(path),
            windows,
        }
    }

    fn get(&self, id: &str) -> Option<&PersistedWindowState> {
        self.windows.get(id)
    }

    fn update(&mut self, record: &WindowRecord) -> Result<()> {
        self.windows.insert(
            record.id.clone(),
            PersistedWindowState {
                title: record.title.clone(),
                route: record.route.clone(),
                width: record.width,
                height: record.height,
                is_primary: record.is_primary,
            },
        );
        self.save()
    }

    fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, serde_json::to_vec_pretty(&self.windows)?)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowOpenParams {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
}

struct ManagedWindow {
    record: WindowRecord,
    window: Window,
    webview: WebView,
}

struct WindowManager {
    assets: EmbeddedAssetRouter,
    content_security_policy: Option<String>,
    security: ResolvedFrontendSecurity,
    ipc_proxy: EventLoopProxy<UserEvent>,
    windows: HashMap<WindowId, ManagedWindow>,
    default_window: WindowOptions,
    dev_url: Option<String>,
    next_window_index: u64,
    state_store: WindowStateStore,
    rate_limiter: SensitiveRateLimiter,
}

impl WindowManager {
    fn new(
        assets: EmbeddedAssetRouter,
        content_security_policy: Option<String>,
        security: ResolvedFrontendSecurity,
        ipc_proxy: EventLoopProxy<UserEvent>,
        default_window: WindowOptions,
        dev_url: Option<String>,
        state_store: WindowStateStore,
    ) -> Self {
        Self {
            assets,
            content_security_policy,
            security,
            ipc_proxy,
            windows: HashMap::new(),
            default_window,
            dev_url,
            next_window_index: 2,
            state_store,
            rate_limiter: SensitiveRateLimiter::default(),
        }
    }

    fn open_primary(&mut self, target: &EventLoopWindowTarget<UserEvent>) -> Result<WindowRecord> {
        self.open_window(
            target,
            WindowOpenParams {
                id: Some(PRIMARY_WINDOW_ID.to_string()),
                route: Some("/".into()),
                title: None,
                width: None,
                height: None,
            },
            true,
        )
    }

    fn open_window(
        &mut self,
        target: &EventLoopWindowTarget<UserEvent>,
        params: WindowOpenParams,
        is_primary: bool,
    ) -> Result<WindowRecord> {
        let id = normalize_window_label(params.id, self.next_window_index)?;
        if !is_primary {
            self.next_window_index += 1;
        }

        if !is_primary
            && !self.security.permissions.is_empty()
            && !self
                .security
                .permissions
                .keys()
                .any(|pattern| window_pattern_matches(pattern, &id))
        {
            return Err(RuntimeError::PermissionDenied(format!(
                "window '{id}' is not declared by a security permission pattern"
            )));
        }

        if let Some(existing) = self
            .windows
            .values()
            .find(|managed| managed.record.id == id)
        {
            existing.window.set_focus();
            return Ok(existing.record.clone());
        }

        if self.windows.len() >= MAX_OPEN_WINDOWS {
            return Err(RuntimeError::PermissionDenied(format!(
                "window.open is limited to {MAX_OPEN_WINDOWS} windows per app"
            )));
        }

        let persisted = self.state_store.get(&id).cloned();
        let route = normalize_window_route(
            params
                .route
                .as_deref()
                .or_else(|| persisted.as_ref().map(|state| state.route.as_str()))
                .unwrap_or("/"),
        )?;
        let title = params
            .title
            .or_else(|| persisted.as_ref().map(|state| state.title.clone()))
            .map(|value| validate_window_title(&value))
            .transpose()?
            .unwrap_or_else(|| self.default_window.title.clone());
        let width = params
            .width
            .or_else(|| persisted.as_ref().map(|state| state.width))
            .map(validate_window_dimension)
            .transpose()?
            .unwrap_or(self.default_window.width);
        let height = params
            .height
            .or_else(|| persisted.as_ref().map(|state| state.height))
            .map(validate_window_dimension)
            .transpose()?
            .unwrap_or(self.default_window.height);

        let record = WindowRecord {
            id,
            title: title.clone(),
            route: route.clone(),
            width,
            height,
            is_primary,
        };

        let window = WindowBuilder::new()
            .with_title(&title)
            .with_inner_size(tao::dpi::LogicalSize::new(width, height))
            .build(target)?;
        let native_window_id = window.id();
        let bridge_config_script = bridge_config_script(&self.security, &record)?;
        let ipc_proxy = self.ipc_proxy.clone();
        let assets = self.assets;
        let content_security_policy = self.content_security_policy.clone();
        let url = window_url(self.dev_url.as_deref(), &route);
        let builder = WebViewBuilder::new()
            .with_background_color((6, 9, 18, 255))
            .with_clipboard(true)
            .with_initialization_script(&bridge_config_script)
            .with_initialization_script(RUSTFRAME_BRIDGE_SCRIPT)
            .with_custom_protocol("app".into(), move |_id, request| {
                asset_response(assets, request, content_security_policy.as_deref())
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_ipc_handler(move |request| {
                let _ = ipc_proxy.send_event(UserEvent::Ipc {
                    window_id: native_window_id,
                    body: request.body().clone(),
                });
            })
            .with_url(&url);
        let webview = build_webview(builder, &window)?;

        self.windows.insert(
            native_window_id,
            ManagedWindow {
                record: record.clone(),
                window,
                webview,
            },
        );
        self.persist_window_state(native_window_id);

        Ok(record)
    }

    fn current(&self, window_id: WindowId) -> Result<WindowRecord> {
        self.windows
            .get(&window_id)
            .map(|managed| managed.record.clone())
            .ok_or_else(|| RuntimeError::InvalidParameter("window is no longer available".into()))
    }

    fn list(&self) -> Vec<WindowRecord> {
        let mut windows = self
            .windows
            .values()
            .map(|managed| managed.record.clone())
            .collect::<Vec<_>>();
        windows.sort_by(|left, right| left.id.cmp(&right.id));
        windows
    }

    fn minimize(&self, window_id: WindowId) -> Result<()> {
        let window = self.window(window_id)?;
        window.set_minimized(true);
        Ok(())
    }

    fn maximize(&self, window_id: WindowId) -> Result<()> {
        let window = self.window(window_id)?;
        window.set_maximized(true);
        Ok(())
    }

    fn set_title(&mut self, window_id: WindowId, title: String) -> Result<()> {
        let title = validate_window_title(&title)?;
        let managed = self.windows.get_mut(&window_id).ok_or_else(|| {
            RuntimeError::InvalidParameter("window is no longer available".into())
        })?;
        managed.window.set_title(&title);
        managed.record.title = title;
        let _ = self.state_store.update(&managed.record);
        Ok(())
    }

    fn resolve_response(&self, window_id: WindowId, response: &IpcResponse) {
        if let Some(managed) = self.windows.get(&window_id) {
            resolve_ipc_response(&managed.webview, response);
        }
    }

    fn window_label(&self, window_id: WindowId) -> String {
        self.windows
            .get(&window_id)
            .map(|managed| managed.record.id.clone())
            .unwrap_or_else(|| "unknown".into())
    }

    fn emit_database_change(&self, mut event: Value, source_window_id: String) {
        if let Some(object) = event.as_object_mut() {
            object.insert("sourceWindowId".into(), Value::String(source_window_id));
        }
        let Ok(serialized) = serde_json::to_string(&event) else {
            return;
        };
        let script = format!(
            "if (window.RustFrame && typeof window.RustFrame.__emitDatabaseChange === 'function') {{ window.RustFrame.__emitDatabaseChange({serialized}); }}"
        );
        let is_restore = event
            .get("operations")
            .and_then(Value::as_array)
            .is_some_and(|operations| operations.iter().any(|value| value == "restore"));
        for managed in self.windows.values() {
            let _ = managed.webview.evaluate_script(&script);
            if is_restore {
                let _ = managed.webview.evaluate_script(
                    "if (window.RustFrame && typeof window.RustFrame.__emitRestore === 'function') { window.RustFrame.__emitRestore(); }",
                );
            }
        }
    }

    fn emit_filesystem_change(&self, window_id: WindowId, event: &Value) {
        let Some(managed) = self.windows.get(&window_id) else {
            return;
        };
        let Ok(serialized) = serde_json::to_string(event) else {
            return;
        };
        let script = format!(
            "if (window.RustFrame && typeof window.RustFrame.__emitFilesystemChange === 'function') {{ window.RustFrame.__emitFilesystemChange({serialized}); }}"
        );
        let _ = managed.webview.evaluate_script(&script);
    }

    fn emit_file_drop(&self, window_id: WindowId, paths: &[PathBuf], fs_capability: &FsCapability) {
        let Some(managed) = self.windows.get(&window_id) else {
            return;
        };

        let files = paths
            .iter()
            .filter_map(|path| {
                let record = external_path_record(path).ok()?;
                let grant = fs_capability
                    .grant_path(path, FsGrantAccess::Read, false)
                    .ok()?;
                Some(json!({
                    "id": grant.id,
                    "uri": grant.uri,
                    "name": record.name,
                    "isDir": record.is_dir,
                    "isFile": record.is_file,
                    "size": record.size,
                    "extension": record.extension,
                    "modifiedAt": record.modified_at,
                    "access": grant.access,
                    "persistent": false
                }))
            })
            .collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }

        if let Ok(serialized) = serde_json::to_string(&json!({ "files": files })) {
            let script = format!(
                "if (window.RustFrame && typeof window.RustFrame.__emitFileDrop === 'function') {{ window.RustFrame.__emitFileDrop({serialized}); }}"
            );
            let _ = managed.webview.evaluate_script(&script);
        }
    }

    fn close_window(&mut self, window_id: WindowId) -> bool {
        self.persist_window_state(window_id);
        let label = self.window_label(window_id);
        self.rate_limiter.close_window(&label);
        self.windows.remove(&window_id);
        self.windows.is_empty()
    }

    fn window(&self, window_id: WindowId) -> Result<&Window> {
        self.windows
            .get(&window_id)
            .map(|managed| &managed.window)
            .ok_or_else(|| RuntimeError::InvalidParameter("window is no longer available".into()))
    }

    fn persist_window_state(&mut self, window_id: WindowId) {
        let Some(managed) = self.windows.get_mut(&window_id) else {
            return;
        };
        let size = managed.window.inner_size();
        managed.record.width = f64::from(size.width);
        managed.record.height = f64::from(size.height);
        let _ = self.state_store.update(&managed.record);
    }
}

#[derive(Default)]
pub struct RustFrameBuilder {
    window: WindowOptions,
    security: FrontendSecurity,
    app_id: Option<String>,
    data_dir: Option<PathBuf>,
    dev_url: Option<String>,
    assets: Option<EmbeddedAssetRouter>,
    content_security_policy: Option<String>,
    database: Option<EmbeddedDatabaseConfig>,
    fs_roots: Vec<PathBuf>,
    persist_fs_grants: Option<bool>,
    shell_commands: BTreeMap<String, ShellCommand>,
}

impl RustFrameBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
        self
    }

    pub fn app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    pub fn frontend_security(mut self, security: FrontendSecurity) -> Self {
        self.security = security;
        self
    }

    pub fn networked_frontend(self) -> Self {
        self.frontend_security(FrontendSecurity::networked())
    }

    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(path.into());
        self
    }

    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.window.width = width;
        self.window.height = height;
        self
    }

    pub fn dev_url(mut self, url: impl Into<String>) -> Self {
        self.dev_url = Some(url.into());
        self
    }

    pub fn embedded_assets<A: EmbeddedAssets>(mut self) -> Self {
        self.assets = Some(EmbeddedAssetRouter::from_type::<A>());
        self
    }

    /// Applies a Content Security Policy header to embedded production assets.
    pub fn content_security_policy(mut self, policy: impl Into<String>) -> Self {
        self.content_security_policy = Some(policy.into());
        self
    }

    pub fn embedded_database<I, S>(mut self, schema_path: impl Into<String>, seed_paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.database = Some(EmbeddedDatabaseConfig {
            schema_path: schema_path.into(),
            seed_paths: seed_paths
                .into_iter()
                .map(|path| path.as_ref().to_string())
                .collect(),
            migration_paths: Vec::new(),
        });
        self
    }

    pub fn embedded_database_with_migrations<I, S, J, T>(
        mut self,
        schema_path: impl Into<String>,
        seed_paths: I,
        migration_paths: J,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        J: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        self.database = Some(EmbeddedDatabaseConfig {
            schema_path: schema_path.into(),
            seed_paths: seed_paths
                .into_iter()
                .map(|path| path.as_ref().to_string())
                .collect(),
            migration_paths: migration_paths
                .into_iter()
                .map(|path| path.as_ref().to_string())
                .collect(),
        });
        self
    }

    pub fn allow_fs_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.fs_roots.push(root.into());
        self
    }

    /// Controls whether user-selected filesystem grants survive application restarts.
    /// Persistence is enabled by default.
    pub fn persist_fs_grants(mut self, persist: bool) -> Self {
        self.persist_fs_grants = Some(persist);
        self
    }

    pub fn allow_shell_command<I, S>(
        self,
        name: impl Into<String>,
        program: impl Into<String>,
        args: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allow_shell_command_configured(name, ShellCommand::new(program, args))
    }

    pub fn allow_shell_command_configured(
        mut self,
        name: impl Into<String>,
        command: ShellCommand,
    ) -> Self {
        self.shell_commands.insert(name.into(), command);
        self
    }

    pub fn run(self) -> Result<()> {
        let RustFrameBuilder {
            window,
            security,
            app_id,
            data_dir,
            dev_url,
            assets,
            content_security_policy,
            database,
            fs_roots,
            persist_fs_grants,
            shell_commands,
        } = self;

        let assets = assets.ok_or(RuntimeError::MissingAssets)?;
        if content_security_policy
            .as_deref()
            .is_some_and(|policy| policy.trim().is_empty())
        {
            return Err(RuntimeError::InvalidConfiguration(
                "content security policy must not be empty".into(),
            ));
        }
        if let Some(policy) = content_security_policy.as_deref() {
            HeaderValue::from_str(policy).map_err(|error| {
                RuntimeError::InvalidConfiguration(format!(
                    "content security policy is not a valid HTTP header: {error}"
                ))
            })?;
        }
        let smoke = RuntimeSmokeConfig::from_env();
        let database_data_dir = smoke
            .as_ref()
            .and_then(|config| config.data_dir.clone())
            .or(data_dir);
        let mut fs_capability = FsCapability::new(fs_roots)?;
        if persist_fs_grants.unwrap_or(true) {
            if let Some(path) = resolve_runtime_data_path(
                app_id.as_deref(),
                database_data_dir.as_deref(),
                FILESYSTEM_GRANTS_FILE_NAME,
            ) {
                fs_capability = fs_capability.with_persistence(path)?;
            }
        }
        let shell_capability = ShellCapability::try_new(shell_commands)?;
        let database_capability =
            load_database_capability(assets, app_id.clone(), database_data_dir.clone(), database)?;
        let dev_url = active_dev_url(dev_url);
        let security = security.resolve(
            &fs_capability,
            &shell_capability,
            database_capability.as_ref(),
        );

        if let Some(smoke) = smoke {
            return run_runtime_smoke_check(
                smoke,
                RuntimeSmokeInputs {
                    window: &window,
                    app_id: app_id.as_deref(),
                    dev_url: dev_url.as_deref(),
                    security: &security,
                    assets,
                    fs_capability: &fs_capability,
                    shell_capability: &shell_capability,
                    database_capability: database_capability.as_ref(),
                },
            );
        }

        prepare_linux_runtime()?;

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let ipc_proxy = event_loop.create_proxy();
        let worker = IpcWorker::spawn(
            ipc_proxy.clone(),
            fs_capability.clone(),
            shell_capability,
            database_capability,
        )?;
        let mut window_manager = WindowManager::new(
            assets,
            content_security_policy,
            security.clone(),
            ipc_proxy,
            window.clone(),
            dev_url,
            WindowStateStore::load(resolve_window_state_path(
                app_id.as_deref(),
                database_data_dir.as_deref(),
            )),
        );
        window_manager.open_primary(&event_loop)?;

        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::DroppedFile(path),
                    ..
                } => {
                    if security.filesystem {
                        window_manager.emit_file_drop(window_id, &[path], &fs_capability);
                    }
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    window_manager.persist_window_state(window_id);
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    worker.close_window(window_id);
                    if window_manager.close_window(window_id) {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::UserEvent(UserEvent::Ipc { window_id, body }) => {
                    if let Some(outcome) = dispatch_ipc_message(
                        &body,
                        window_id,
                        &worker,
                        &security,
                        &fs_capability,
                        &mut window_manager,
                        target,
                    ) {
                        window_manager.resolve_response(window_id, &outcome.response);
                        if let Some(close_window_id) = outcome.close_window {
                            worker.close_window(close_window_id);
                            if window_manager.close_window(close_window_id) {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                    }
                }
                Event::UserEvent(UserEvent::IpcResponse {
                    window_id,
                    response,
                    database_change,
                }) => {
                    window_manager.resolve_response(window_id, &response);
                    if let Some(event) = database_change {
                        let source = window_manager.window_label(window_id);
                        window_manager.emit_database_change(event, source);
                    }
                }
                Event::UserEvent(UserEvent::FilesystemChange { window_id, event }) => {
                    window_manager.emit_filesystem_change(window_id, &event);
                }
                Event::MainEventsCleared => {
                    if window_manager.windows.is_empty() {
                        *control_flow = ControlFlow::Exit;
                    }
                    pump_linux_events();
                }
                _ => {}
            }
        });

        #[allow(unreachable_code)]
        Ok(())
    }
}

struct RuntimeSmokeInputs<'a> {
    window: &'a WindowOptions,
    app_id: Option<&'a str>,
    dev_url: Option<&'a str>,
    security: &'a ResolvedFrontendSecurity,
    assets: EmbeddedAssetRouter,
    fs_capability: &'a FsCapability,
    shell_capability: &'a ShellCapability,
    database_capability: Option<&'a DatabaseCapability>,
}

fn run_runtime_smoke_check(
    smoke: RuntimeSmokeConfig,
    inputs: RuntimeSmokeInputs<'_>,
) -> Result<()> {
    let RuntimeSmokeInputs {
        window,
        app_id,
        dev_url,
        security,
        assets,
        fs_capability,
        shell_capability,
        database_capability,
    } = inputs;
    let report = RuntimeSmokeReport {
        app_id: app_id.map(ToOwned::to_owned),
        launch_mode: if dev_url.is_some() {
            "dev-server".to_string()
        } else {
            "embedded".to_string()
        },
        active_dev_url: dev_url.map(ToOwned::to_owned),
        window: window.clone(),
        security: security.clone(),
        has_index_html: assets.get("index.html").is_some(),
        bridge_injected: true,
        fs_roots: fs_capability
            .roots()
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        shell_commands: shell_capability
            .command_names()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        database: database_capability.map(|database| database.info().clone()),
    };

    let contents = serde_json::to_string_pretty(&report)
        .map_err(|error| RuntimeError::InvalidConfiguration(error.to_string()))?;

    if let Some(output_path) = smoke.output_path {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, contents)?;
    } else {
        println!("{contents}");
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeConfig<'a> {
    #[serde(flatten)]
    security: &'a ResolvedFrontendSecurity,
    current_window: &'a WindowRecord,
}

fn bridge_config_script(
    security: &ResolvedFrontendSecurity,
    current_window: &WindowRecord,
) -> Result<String> {
    let serialized = serde_json::to_string(&BridgeConfig {
        security,
        current_window,
    })?;
    Ok(format!(
        "window.__RUSTFRAME_BRIDGE_CONFIG__ = Object.freeze({serialized});"
    ))
}

enum UserEvent {
    Ipc {
        window_id: WindowId,
        body: String,
    },
    IpcResponse {
        window_id: WindowId,
        response: IpcResponse,
        database_change: Option<Value>,
    },
    FilesystemChange {
        window_id: WindowId,
        event: Value,
    },
}

struct IpcOutcome {
    response: IpcResponse,
    close_window: Option<WindowId>,
}

struct BackgroundIpcRequest {
    window_id: WindowId,
    request: IpcRequest,
}

enum IpcWorkerMessage {
    Request(BackgroundIpcRequest),
    CloseWindow(WindowId),
}

struct IpcWorker {
    sender: mpsc::Sender<IpcWorkerMessage>,
}

impl IpcWorker {
    fn spawn(
        proxy: EventLoopProxy<UserEvent>,
        fs_capability: FsCapability,
        shell_capability: ShellCapability,
        database_capability: Option<DatabaseCapability>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<IpcWorkerMessage>();
        thread::Builder::new()
            .name("rustframe-ipc-worker".into())
            .spawn(move || {
                let mut watchers = WatcherRegistry::new(proxy.clone(), fs_capability.clone());
                while let Ok(message) = receiver.recv() {
                    let background_request = match message {
                        IpcWorkerMessage::Request(request) => request,
                        IpcWorkerMessage::CloseWindow(window_id) => {
                            watchers.stop_window(window_id);
                            continue;
                        }
                    };
                    let outcome = match background_request.request.method.as_str() {
                        "fs.watch" => {
                            watchers.watch(background_request.window_id, background_request.request)
                        }
                        "fs.unwatch" => watchers
                            .unwatch(background_request.window_id, background_request.request),
                        "fs.revokeGrant" => watchers.revoke_grant(background_request.request),
                        _ => execute_background_request(
                            background_request.request,
                            &fs_capability,
                            &shell_capability,
                            database_capability.as_ref(),
                        ),
                    };

                    if proxy
                        .send_event(UserEvent::IpcResponse {
                            window_id: background_request.window_id,
                            response: outcome.response,
                            database_change: outcome.database_change,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;

        Ok(Self { sender })
    }

    fn dispatch(&self, window_id: WindowId, request: IpcRequest) -> Result<()> {
        self.sender
            .send(IpcWorkerMessage::Request(BackgroundIpcRequest {
                window_id,
                request,
            }))
            .map_err(|_| {
                RuntimeError::InvalidConfiguration("background IPC worker is unavailable".into())
            })
    }

    fn close_window(&self, window_id: WindowId) {
        let _ = self.sender.send(IpcWorkerMessage::CloseWindow(window_id));
    }
}

struct ActiveWatcher {
    stop: Arc<AtomicBool>,
    window_id: WindowId,
    uri: String,
}

struct WatcherRegistry {
    proxy: EventLoopProxy<UserEvent>,
    fs_capability: FsCapability,
    watchers: HashMap<String, ActiveWatcher>,
    next_id: u64,
}

#[derive(Debug, Deserialize)]
struct FsWatchParams {
    uri: String,
    #[serde(default)]
    recursive: bool,
}

#[derive(Debug, Deserialize)]
struct FsUnwatchParams {
    id: String,
}

impl WatcherRegistry {
    fn new(proxy: EventLoopProxy<UserEvent>, fs_capability: FsCapability) -> Self {
        Self {
            proxy,
            fs_capability,
            watchers: HashMap::new(),
            next_id: 1,
        }
    }

    fn watch(&mut self, window_id: WindowId, request: IpcRequest) -> BackgroundIpcOutcome {
        let result = (|| -> Result<Value> {
            let params: FsWatchParams = parse_params(&request.params)?;
            let root = self.fs_capability.resolve(Path::new(&params.uri))?;
            if !root.is_dir() {
                return Err(RuntimeError::InvalidParameter(
                    "fs.watch requires a directory URI".into(),
                ));
            }
            let watcher_id = format!("watcher-{}", self.next_id);
            self.next_id += 1;
            let stop = Arc::new(AtomicBool::new(false));
            self.watchers.insert(
                watcher_id.clone(),
                ActiveWatcher {
                    stop: Arc::clone(&stop),
                    window_id,
                    uri: params.uri.clone(),
                },
            );
            let proxy = self.proxy.clone();
            let event_watcher_id = watcher_id.clone();
            let uri = params.uri.clone();
            thread::Builder::new()
                .name(format!("rustframe-{watcher_id}"))
                .spawn(move || {
                    let mut previous = filesystem_snapshot(&root, params.recursive).unwrap_or_default();
                    while !stop.load(Ordering::Relaxed) {
                        thread::sleep(Duration::from_millis(250));
                        let Ok(current) = filesystem_snapshot(&root, params.recursive) else { continue; };
                        let removed = previous.keys().filter(|path| !current.contains_key(*path)).cloned().collect::<Vec<_>>();
                        let created = current.keys().filter(|path| !previous.contains_key(*path)).cloned().collect::<Vec<_>>();
                        if removed.len() == 1 && created.len() == 1 {
                            let _ = proxy.send_event(UserEvent::FilesystemChange {
                                window_id,
                                event: json!({
                                    "watcherId": event_watcher_id,
                                    "operation": "rename",
                                    "oldUri": watched_uri(&uri, &root, &removed[0]),
                                    "uri": watched_uri(&uri, &root, &created[0])
                                }),
                            });
                        } else {
                            for path in &removed {
                                let _ = proxy.send_event(UserEvent::FilesystemChange {
                                    window_id,
                                    event: json!({ "watcherId": event_watcher_id, "operation": "delete", "uri": watched_uri(&uri, &root, path) }),
                                });
                            }
                            for path in &created {
                                let _ = proxy.send_event(UserEvent::FilesystemChange {
                                    window_id,
                                    event: json!({ "watcherId": event_watcher_id, "operation": "create", "uri": watched_uri(&uri, &root, path) }),
                                });
                            }
                        }
                        for (path, metadata) in &current {
                            if previous.get(path).is_some_and(|previous_metadata| previous_metadata != metadata) {
                                let _ = proxy.send_event(UserEvent::FilesystemChange {
                                    window_id,
                                    event: json!({ "watcherId": event_watcher_id, "operation": "modify", "uri": watched_uri(&uri, &root, path) }),
                                });
                            }
                        }
                        previous = current;
                    }
                })?;
            Ok(json!({ "id": watcher_id }))
        })();
        background_outcome(request.id, result)
    }

    fn unwatch(&mut self, window_id: WindowId, request: IpcRequest) -> BackgroundIpcOutcome {
        let result = (|| -> Result<Value> {
            let params: FsUnwatchParams = parse_params(&request.params)?;
            if self
                .watchers
                .get(&params.id)
                .is_some_and(|watcher| watcher.window_id != window_id)
            {
                return Err(RuntimeError::PermissionDenied(
                    "filesystem watchers can only be stopped by their owning window".into(),
                ));
            }
            let removed = self.watchers.remove(&params.id);
            if let Some(watcher) = &removed {
                watcher.stop.store(true, Ordering::Relaxed);
            }
            Ok(json!({ "unwatched": removed.is_some() }))
        })();
        background_outcome(request.id, result)
    }

    fn revoke_grant(&mut self, request: IpcRequest) -> BackgroundIpcOutcome {
        let result = (|| -> Result<Value> {
            let params: FsGrantIdParams = parse_params(&request.params)?;
            let revoked = self.fs_capability.revoke_grant(&params.id)?;
            if revoked {
                let prefix = format!("grant://{}", params.id);
                self.stop_matching(|watcher| {
                    watcher.uri == prefix || watcher.uri.starts_with(&format!("{prefix}/"))
                });
            }
            Ok(json!({ "revoked": revoked }))
        })();
        background_outcome(request.id, result)
    }

    fn stop_window(&mut self, window_id: WindowId) {
        self.stop_matching(|watcher| watcher.window_id == window_id);
    }

    fn stop_matching(&mut self, predicate: impl Fn(&ActiveWatcher) -> bool) {
        let ids = self
            .watchers
            .iter()
            .filter(|&(_, watcher)| predicate(watcher))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            if let Some(watcher) = self.watchers.remove(&id) {
                watcher.stop.store(true, Ordering::Relaxed);
            }
        }
    }
}

fn background_outcome(id: u64, result: Result<Value>) -> BackgroundIpcOutcome {
    match result {
        Ok(data) => BackgroundIpcOutcome {
            response: IpcResponse::success(id, data),
            database_change: None,
        },
        Err(error) => BackgroundIpcOutcome {
            response: IpcResponse::failure(id, &error),
            database_change: None,
        },
    }
}

type FilesystemSnapshot = BTreeMap<PathBuf, (Option<SystemTime>, u64, bool)>;

fn filesystem_snapshot(root: &Path, recursive: bool) -> std::io::Result<FilesystemSnapshot> {
    let mut snapshot = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            snapshot.insert(
                path.clone(),
                (metadata.modified().ok(), metadata.len(), metadata.is_dir()),
            );
            if recursive && metadata.is_dir() {
                directories.push(path);
            }
        }
    }
    Ok(snapshot)
}

fn watched_uri(base_uri: &str, root: &Path, path: &Path) -> String {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if relative.is_empty() {
        base_uri.to_string()
    } else {
        format!("{}/{relative}", base_uri.trim_end_matches('/'))
    }
}

#[derive(Debug, Deserialize)]
struct DbRecordParams {
    table: String,
    record: Value,
}

#[derive(Debug, Deserialize)]
struct DbUpdateParams {
    table: String,
    id: i64,
    patch: Value,
}

#[derive(Debug, Deserialize)]
struct DbBatchParams {
    operations: Vec<DatabaseBatchOperation>,
}

#[derive(Debug, Deserialize)]
struct DbTransferParams {
    path: String,
}

#[derive(Debug, Deserialize)]
struct DbGetParams {
    table: String,
    id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsWriteTextParams {
    path: String,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsWriteBinaryParams {
    path: String,
    base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsCopyParams {
    source_path: String,
    destination_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsWalkParams {
    uri: String,
    #[serde(flatten)]
    options: FsWalkOptions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FsGrantRequestParams {
    kind: String,
    #[serde(default = "default_grant_access")]
    access: FsGrantAccess,
    #[serde(default)]
    persist: bool,
    #[serde(default)]
    title: Option<String>,
}

fn default_grant_access() -> FsGrantAccess {
    FsGrantAccess::Read
}

#[derive(Debug, Deserialize)]
struct FsGrantIdParams {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DialogFileOptions {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    filters: Vec<DialogFilter>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DialogSaveOptions {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    default_name: Option<String>,
    #[serde(default)]
    filters: Vec<DialogFilter>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DialogSaveTextParams {
    #[serde(flatten)]
    options: DialogSaveOptions,
    contents: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DialogSaveBinaryParams {
    #[serde(flatten)]
    options: DialogSaveOptions,
    base64: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DialogFilter {
    name: String,
    #[serde(default)]
    extensions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalPathRecord {
    name: String,
    is_dir: bool,
    is_file: bool,
    size: u64,
    extension: Option<String>,
    modified_at: Option<String>,
}

fn dispatch_ipc_message(
    body: &str,
    window_id: WindowId,
    worker: &IpcWorker,
    security: &ResolvedFrontendSecurity,
    fs_capability: &FsCapability,
    window_manager: &mut WindowManager,
    target: &EventLoopWindowTarget<UserEvent>,
) -> Option<IpcOutcome> {
    match decode_request(body.as_bytes(), MAX_IPC_REQUEST_BYTES) {
        Ok(request) => dispatch_request(
            request,
            window_id,
            worker,
            security,
            fs_capability,
            window_manager,
            target,
        ),
        Err(error) => Some(IpcOutcome {
            response: IpcResponse::failure(0, &error),
            close_window: None,
        }),
    }
}

fn dispatch_request(
    mut request: IpcRequest,
    window_id: WindowId,
    worker: &IpcWorker,
    security: &ResolvedFrontendSecurity,
    fs_capability: &FsCapability,
    window_manager: &mut WindowManager,
    target: &EventLoopWindowTarget<UserEvent>,
) -> Option<IpcOutcome> {
    let window_label = window_manager.window_label(window_id);
    if let Err(error) = authorize_request(&request, &window_label, security).and_then(|()| {
        window_manager
            .rate_limiter
            .check(&window_label, &request.method)
    }) {
        let code = IpcErrorResponse::from(&error).code;
        audit_sensitive_request(&window_label, &request.method, "denied", Some(code));
        return Some(IpcOutcome {
            response: IpcResponse::failure(request.id, &error),
            close_window: None,
        });
    }

    if matches!(request.method.as_str(), "db.backup" | "db.restore") {
        match prepare_database_dialog(&mut request) {
            Ok(Some(data)) => {
                audit_sensitive_request(&window_label, &request.method, "completed", None);
                return Some(IpcOutcome {
                    response: IpcResponse::success(request.id, data),
                    close_window: None,
                });
            }
            Ok(None) => {}
            Err(error) => {
                let code = IpcErrorResponse::from(&error).code;
                audit_sensitive_request(&window_label, &request.method, "failed", Some(code));
                return Some(IpcOutcome {
                    response: IpcResponse::failure(request.id, &error),
                    close_window: None,
                });
            }
        }
    }

    match method_execution(&request.method) {
        MethodExecution::MainThread => {
            let method = request.method.clone();
            let outcome = handle_main_thread_request(
                request,
                window_id,
                fs_capability,
                window_manager,
                target,
            );
            audit_sensitive_request(
                &window_label,
                &method,
                if outcome.response.ok {
                    "completed"
                } else {
                    "failed"
                },
                outcome.response.error.as_ref().map(|error| error.code),
            );
            Some(outcome)
        }
        MethodExecution::Background => {
            let method = request.method.clone();
            let request_id = request.id;
            match worker.dispatch(window_id, request) {
                Ok(()) => {
                    audit_sensitive_request(&window_label, &method, "accepted", None);
                    None
                }
                Err(error) => {
                    let code = IpcErrorResponse::from(&error).code;
                    audit_sensitive_request(&window_label, &method, "failed", Some(code));
                    Some(IpcOutcome {
                        response: IpcResponse::failure(request_id, &error),
                        close_window: None,
                    })
                }
            }
        }
        MethodExecution::Unknown => Some(IpcOutcome {
            response: IpcResponse::failure(
                request.id,
                &RuntimeError::UnknownMethod(request.method),
            ),
            close_window: None,
        }),
    }
}

fn audit_sensitive_request(window_id: &str, method: &str, status: &str, error_code: Option<&str>) {
    if sensitive_rate_policy(method).is_none() {
        return;
    }
    let Some(path) = env::var_os("RUSTFRAME_AUDIT_LOG") else {
        return;
    };
    let timestamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let payload = json!({
        "timestamp": timestamp,
        "kind": "ipc",
        "windowId": window_id,
        "method": method,
        "status": status,
        "errorCode": error_code,
        "paramsRedacted": true,
    });
    if let Some(parent) = Path::new(&path).parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{payload}");
    }
}

fn prepare_database_dialog(request: &mut IpcRequest) -> Result<Option<Value>> {
    let sqlite_extensions = ["db", "sqlite", "sqlite3"];
    let path = if request.method == "db.backup" {
        let suggested_name = request
            .params
            .get("suggestedName")
            .and_then(Value::as_str)
            .unwrap_or("rustframe-backup.db");
        FileDialog::new()
            .set_title("Back up RustFrame database")
            .set_file_name(suggested_name)
            .add_filter("SQLite database", &sqlite_extensions)
            .save_file()
    } else {
        FileDialog::new()
            .set_title("Restore RustFrame database")
            .add_filter("SQLite database", &sqlite_extensions)
            .pick_file()
    };

    let Some(path) = path else {
        return Ok(Some(if request.method == "db.backup" {
            json!({ "cancelled": true })
        } else {
            json!({ "restored": false, "cancelled": true })
        }));
    };
    request.params = json!({ "path": path.to_string_lossy() });
    Ok(None)
}

fn authorize_method(method: &str, security: &ResolvedFrontendSecurity) -> Result<()> {
    match method {
        "fs.readText"
        | "fs.readBinary"
        | "fs.metadata"
        | "fs.listDir"
        | "fs.writeText"
        | "fs.writeBinary"
        | "fs.copyFrom"
        | "fs.openPath"
        | "fs.revealPath"
        | "fs.walk"
        | "fs.requestGrant"
        | "fs.listGrants"
        | "fs.revokeGrant"
        | "fs.watch"
        | "fs.unwatch"
        | "dialog.openFile"
        | "dialog.openFiles"
        | "dialog.openDirectory"
        | "dialog.saveText"
        | "dialog.saveBinary"
            if !security.filesystem =>
        {
            Err(RuntimeError::PermissionDenied(
                "filesystem bridge is disabled for this frontend".into(),
            ))
        }
        "shell.exec" if !security.shell => Err(RuntimeError::PermissionDenied(
            "shell bridge is disabled for this frontend".into(),
        )),
        "db.info" | "db.get" | "db.list" | "db.search" | "db.count" | "db.insert" | "db.update"
        | "db.delete" | "db.batch" | "db.backup" | "db.restore"
            if !security.database =>
        {
            Err(RuntimeError::PermissionDenied(
                "database bridge is disabled for this frontend".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn authorize_request(
    request: &IpcRequest,
    window_id: &str,
    security: &ResolvedFrontendSecurity,
) -> Result<()> {
    authorize_method(&request.method, security)?;
    let mut required = required_permission(request).into_iter().collect::<Vec<_>>();
    if request.method == "fs.copyFrom" {
        let source = request
            .params
            .get("sourcePath")
            .and_then(Value::as_str)
            .unwrap_or_default();
        required.push(filesystem_path_permission(source, "read").to_string());
    }
    if required.is_empty() {
        return Ok(());
    }
    for required in required {
        let allowed = security.permissions.iter().any(|(pattern, permissions)| {
            window_pattern_matches(pattern, window_id)
                && permissions.iter().any(|permission| permission == &required)
        });
        if !allowed {
            return Err(RuntimeError::PermissionDenied(format!(
                "window '{window_id}' does not have permission '{required}'"
            )));
        }
    }
    Ok(())
}

fn window_pattern_matches(pattern: &str, window_id: &str) -> bool {
    pattern == window_id
        || pattern.strip_suffix('*').is_some_and(|prefix| {
            !prefix.is_empty() && !prefix.contains('*') && window_id.starts_with(prefix)
        })
}

fn required_permission(request: &IpcRequest) -> Option<String> {
    let permission = match request.method.as_str() {
        "db.info" | "db.get" | "db.list" | "db.search" | "db.count" => "db:read",
        "db.insert" | "db.update" | "db.delete" | "db.batch" => "db:write",
        "db.backup" => "db:backup",
        "db.restore" => "db:restore",
        "fs.readText" | "fs.readBinary" | "fs.metadata" | "fs.listDir" | "fs.walk"
        | "fs.openPath" | "fs.revealPath" => {
            let path = request
                .params
                .get("path")
                .or_else(|| request.params.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            filesystem_path_permission(path, "read")
        }
        "fs.writeText" | "fs.writeBinary" | "fs.copyFrom" => {
            let path = request
                .params
                .get("path")
                .or_else(|| request.params.get("destinationPath"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            filesystem_path_permission(path, "write")
        }
        "fs.requestGrant" => {
            if request.params.get("access").and_then(Value::as_str) == Some("read-write") {
                "fs:grants:write"
            } else {
                "fs:grants:read"
            }
        }
        "fs.listGrants" => "fs:grants:read",
        "fs.revokeGrant" => "fs:grants:write",
        "fs.watch" => {
            let uri = request
                .params
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or_default();
            filesystem_path_permission(uri, "watch")
        }
        "fs.unwatch" => return None,
        "dialog.openFile" | "dialog.openFiles" | "dialog.openDirectory" => "dialog:open",
        "dialog.saveText" | "dialog.saveBinary" => "dialog:save",
        "window.open" => "window:create",
        "clipboard.readText" => "clipboard:read",
        "clipboard.writeText" => "clipboard:write",
        "shell.exec" => {
            return request
                .params
                .get("command")
                .and_then(Value::as_str)
                .map(|command| format!("shell:{command}"));
        }
        _ => return None,
    };
    Some(permission.into())
}

fn filesystem_path_permission(path: &str, operation: &str) -> &'static str {
    match (path.starts_with("grant://"), operation) {
        (true, "read") => "fs:grants:read",
        (true, "write") => "fs:grants:write",
        (true, "watch") => "fs:grants:watch",
        (false, "read") => "fs:workspace:read",
        (false, "write") => "fs:workspace:write",
        (false, "watch") => "fs:workspace:watch",
        _ => "fs:workspace:read",
    }
}

fn resolve_ipc_response(webview: &WebView, response: &IpcResponse) {
    if let Ok(serialized) = serde_json::to_string(response) {
        let script = format!(
            "if (window.RustFrame && typeof window.RustFrame.__resolveFromNative === 'function') {{ window.RustFrame.__resolveFromNative({serialized}); }}"
        );
        let _ = webview.evaluate_script(&script);
    }
}

fn method_execution(method: &str) -> MethodExecution {
    match method {
        "window.close"
        | "window.minimize"
        | "window.maximize"
        | "window.setTitle"
        | "window.current"
        | "window.list"
        | "window.open"
        | "dialog.openFile"
        | "dialog.openFiles"
        | "dialog.openDirectory"
        | "dialog.saveText"
        | "dialog.saveBinary"
        | "fs.requestGrant" => MethodExecution::MainThread,
        "fs.readText"
        | "fs.readBinary"
        | "fs.metadata"
        | "fs.listDir"
        | "fs.writeText"
        | "fs.writeBinary"
        | "fs.copyFrom"
        | "fs.openPath"
        | "fs.revealPath"
        | "fs.walk"
        | "fs.listGrants"
        | "fs.revokeGrant"
        | "fs.watch"
        | "fs.unwatch"
        | "clipboard.readText"
        | "clipboard.writeText"
        | "shell.exec"
        | "db.info"
        | "db.get"
        | "db.list"
        | "db.search"
        | "db.count"
        | "db.insert"
        | "db.update"
        | "db.delete"
        | "db.batch"
        | "db.backup"
        | "db.restore" => MethodExecution::Background,
        _ => MethodExecution::Unknown,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MethodExecution {
    MainThread,
    Background,
    Unknown,
}

fn handle_main_thread_request(
    request: IpcRequest,
    window_id: WindowId,
    fs_capability: &FsCapability,
    window_manager: &mut WindowManager,
    target: &EventLoopWindowTarget<UserEvent>,
) -> IpcOutcome {
    let mut close_window = None;
    let result: Result<Value> = match request.method.as_str() {
        "window.close" => {
            close_window = Some(window_id);
            Ok(Value::Null)
        }
        "window.minimize" => (|| {
            window_manager.minimize(window_id)?;
            Ok(Value::Null)
        })(),
        "window.maximize" => (|| {
            window_manager.maximize(window_id)?;
            Ok(Value::Null)
        })(),
        "window.setTitle" => (|| {
            let title = required_string(&request.params, "title")?;
            window_manager.set_title(window_id, title)?;
            Ok(json!(window_manager.current(window_id)?))
        })(),
        "window.current" => (|| Ok(json!(window_manager.current(window_id)?)))(),
        "window.list" => Ok(json!(window_manager.list())),
        "window.open" => (|| {
            let params: WindowOpenParams = parse_params(&request.params)?;
            Ok(json!(window_manager.open_window(target, params, false)?))
        })(),
        "fs.requestGrant" => (|| {
            let params: FsGrantRequestParams = parse_params(&request.params)?;
            let mut dialog = FileDialog::new();
            if let Some(title) = params.title {
                dialog = dialog.set_title(&title);
            }
            let selected = match params.kind.as_str() {
                "directory" => dialog.pick_folder(),
                "file" => dialog.pick_file(),
                _ => {
                    return Err(RuntimeError::InvalidParameter(
                        "fs.requestGrant kind must be 'file' or 'directory'".into(),
                    ));
                }
            };
            let Some(path) = selected else {
                return Ok(Value::Null);
            };
            Ok(json!(fs_capability.grant_path(
                path,
                params.access,
                params.persist
            )?))
        })(),
        "dialog.openFile" => (|| {
            let options: DialogFileOptions = parse_params(&request.params)?;
            let dialog = build_file_dialog(fs_capability, &options, None)?;
            let selected = dialog
                .pick_file()
                .map(|path| fs_capability.grant_path(path, FsGrantAccess::Read, false))
                .transpose()?;
            Ok(selected.map(|grant| json!(grant)).unwrap_or(Value::Null))
        })(),
        "dialog.openFiles" => (|| {
            let options: DialogFileOptions = parse_params(&request.params)?;
            let dialog = build_file_dialog(fs_capability, &options, None)?;
            let files = dialog
                .pick_files()
                .unwrap_or_default()
                .into_iter()
                .map(|path| fs_capability.grant_path(path, FsGrantAccess::Read, false))
                .collect::<Result<Vec<_>>>()?;
            Ok(json!(files))
        })(),
        "dialog.openDirectory" => (|| {
            let options: DialogFileOptions = parse_params(&request.params)?;
            let dialog = build_file_dialog(fs_capability, &options, None)?;
            let selected = dialog
                .pick_folder()
                .map(|path| fs_capability.grant_path(path, FsGrantAccess::Read, false))
                .transpose()?;
            Ok(selected.map(|grant| json!(grant)).unwrap_or(Value::Null))
        })(),
        "dialog.saveText" => (|| {
            let params: DialogSaveTextParams = parse_params(&request.params)?;
            let dialog = build_save_dialog(fs_capability, &params.options)?;
            let Some(path) = dialog.save_file() else {
                return Ok(Value::Null);
            };
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, params.contents)?;
            Ok(json!(fs_capability.grant_path(
                path,
                FsGrantAccess::ReadWrite,
                false
            )?))
        })(),
        "dialog.saveBinary" => (|| {
            let params: DialogSaveBinaryParams = parse_params(&request.params)?;
            let dialog = build_save_dialog(fs_capability, &params.options)?;
            let Some(path) = dialog.save_file() else {
                return Ok(Value::Null);
            };
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = BASE64_STANDARD.decode(&params.base64).map_err(|error| {
                RuntimeError::InvalidParameter(format!(
                    "binary payload is not valid base64: {error}"
                ))
            })?;
            fs::write(&path, bytes)?;
            Ok(json!(fs_capability.grant_path(
                path,
                FsGrantAccess::ReadWrite,
                false
            )?))
        })(),
        _ => Err(RuntimeError::UnknownMethod(request.method.clone())),
    };

    let response = match result {
        Ok(data) => IpcResponse::success(request.id, data),
        Err(error) => IpcResponse::failure(request.id, &error),
    };

    IpcOutcome {
        response,
        close_window,
    }
}

struct BackgroundIpcOutcome {
    response: IpcResponse,
    database_change: Option<Value>,
}

fn execute_background_request(
    request: IpcRequest,
    fs_capability: &FsCapability,
    shell_capability: &ShellCapability,
    database_capability: Option<&DatabaseCapability>,
) -> BackgroundIpcOutcome {
    let result: Result<Value> = match request.method.as_str() {
        "fs.readText" => (|| {
            let path = required_string(&request.params, "path")?;
            let content = fs_capability.read_text(path)?;
            Ok(Value::String(content))
        })(),
        "fs.readBinary" => (|| {
            let path = required_string(&request.params, "path")?;
            Ok(json!(fs_capability.read_binary(path)?))
        })(),
        "fs.metadata" => (|| {
            let path = required_string(&request.params, "path")?;
            Ok(json!(fs_capability.metadata(path)?))
        })(),
        "fs.listDir" => (|| {
            let path = request
                .params
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            Ok(json!(fs_capability.list_dir(path)?))
        })(),
        "fs.walk" => (|| {
            let params: FsWalkParams = parse_params(&request.params)?;
            Ok(json!(fs_capability.walk(params.uri, &params.options)?))
        })(),
        "fs.listGrants" => (|| Ok(json!(fs_capability.grants()?)))(),
        "fs.revokeGrant" => (|| {
            let params: FsGrantIdParams = parse_params(&request.params)?;
            Ok(json!({ "revoked": fs_capability.revoke_grant(&params.id)? }))
        })(),
        "fs.writeText" => (|| {
            let params: FsWriteTextParams = parse_params(&request.params)?;
            Ok(json!(
                fs_capability.write_text(params.path, &params.contents)?
            ))
        })(),
        "fs.writeBinary" => (|| {
            let params: FsWriteBinaryParams = parse_params(&request.params)?;
            Ok(json!(
                fs_capability.write_binary(params.path, &params.base64)?
            ))
        })(),
        "fs.copyFrom" => (|| {
            let params: FsCopyParams = parse_params(&request.params)?;
            Ok(json!(
                fs_capability.copy_from(params.source_path, params.destination_path)?
            ))
        })(),
        "fs.openPath" => (|| {
            let path = required_string(&request.params, "path")?;
            Ok(json!(fs_capability.open_path(path)?))
        })(),
        "fs.revealPath" => (|| {
            let path = required_string(&request.params, "path")?;
            Ok(json!(fs_capability.reveal_path(path)?))
        })(),
        "clipboard.readText" => (|| {
            let mut clipboard = Clipboard::new().map_err(clipboard_error)?;
            Ok(Value::String(
                clipboard.get_text().map_err(clipboard_error)?,
            ))
        })(),
        "clipboard.writeText" => (|| {
            let text = required_string(&request.params, "text")?;
            let mut clipboard = Clipboard::new().map_err(clipboard_error)?;
            clipboard.set_text(text).map_err(clipboard_error)?;
            Ok(Value::Null)
        })(),
        "shell.exec" => (|| {
            let command = required_string(&request.params, "command")?;
            let args = optional_string_vec(&request.params, "args")?;
            let output = shell_capability.exec(&command, &args)?;
            Ok(json!(output))
        })(),
        "db.info" => (|| Ok(json!(database(database_capability)?.info())))(),
        "db.get" => (|| {
            let params: DbGetParams = parse_params(&request.params)?;
            Ok(database(database_capability)?
                .get(&params.table, params.id)?
                .unwrap_or(Value::Null))
        })(),
        "db.list" => (|| {
            let query: DatabaseListQuery = parse_params(&request.params)?;
            Ok(Value::Array(database(database_capability)?.list(&query)?))
        })(),
        "db.search" => (|| {
            let query: DatabaseSearchQuery = parse_params(&request.params)?;
            Ok(Value::Array(database(database_capability)?.search(&query)?))
        })(),
        "db.count" => (|| {
            let query: DatabaseListQuery = parse_params(&request.params)?;
            Ok(json!(database(database_capability)?.count(&query)?))
        })(),
        "db.insert" => (|| {
            let params: DbRecordParams = parse_params(&request.params)?;
            database(database_capability)?.insert(&params.table, params.record)
        })(),
        "db.update" => (|| {
            let params: DbUpdateParams = parse_params(&request.params)?;
            database(database_capability)?.update(&params.table, params.id, params.patch)
        })(),
        "db.delete" => (|| {
            let params: DbGetParams = parse_params(&request.params)?;
            Ok(json!({
                "deleted": database(database_capability)?.delete(&params.table, params.id)?
            }))
        })(),
        "db.batch" => (|| {
            let params: DbBatchParams = parse_params(&request.params)?;
            Ok(Value::Array(
                database(database_capability)?.batch(&params.operations)?,
            ))
        })(),
        "db.backup" => (|| {
            let params: DbTransferParams = parse_params(&request.params)?;
            let destination = PathBuf::from(&params.path);
            database(database_capability)?.backup_to(&destination)?;
            Ok(json!({
                "backedUp": true,
                "cancelled": false
            }))
        })(),
        "db.restore" => (|| {
            let params: DbTransferParams = parse_params(&request.params)?;
            let capability = database(database_capability)?;
            let safety_backup = PathBuf::from(&capability.info().data_dir).join(format!(
                "restore-safety-{}.db",
                OffsetDateTime::now_utc().unix_timestamp_nanos()
            ));
            capability.restore_from(Path::new(&params.path), &safety_backup)?;
            Ok(json!({
                "restored": true
            }))
        })(),
        method => Err(RuntimeError::UnknownMethod(method.to_string())),
    };

    match result {
        Ok(data) => BackgroundIpcOutcome {
            database_change: database_change_for_request(&request, &data),
            response: IpcResponse::success(request.id, data),
        },
        Err(error) => BackgroundIpcOutcome {
            response: IpcResponse::failure(request.id, &error),
            database_change: None,
        },
    }
}

fn database_change_for_request(request: &IpcRequest, data: &Value) -> Option<Value> {
    let single = |operation: &str| {
        let table = request.params.get("table")?.as_str()?.to_string();
        let id = data
            .get("id")
            .and_then(Value::as_i64)
            .or_else(|| request.params.get("id").and_then(Value::as_i64));
        Some(json!({
            "tables": [table],
            "operations": [operation],
            "recordIds": id.into_iter().collect::<Vec<_>>()
        }))
    };
    match request.method.as_str() {
        "db.insert" => single("insert"),
        "db.update" => single("update"),
        "db.delete" => single("delete"),
        "db.batch" => {
            let params: DbBatchParams = serde_json::from_value(request.params.clone()).ok()?;
            let mut tables = params
                .operations
                .iter()
                .map(|operation| operation.table().to_string())
                .collect::<Vec<_>>();
            tables.sort();
            tables.dedup();
            let operations = params
                .operations
                .iter()
                .map(DatabaseBatchOperation::operation_name)
                .collect::<Vec<_>>();
            let record_ids = data
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.get("id").and_then(Value::as_i64))
                .collect::<Vec<_>>();
            Some(json!({ "tables": tables, "operations": operations, "recordIds": record_ids }))
        }
        "db.restore" => Some(json!({
            "tables": data.get("tables").cloned().unwrap_or_else(|| json!([])),
            "operations": ["restore"],
            "recordIds": []
        })),
        _ => None,
    }
}

fn database(database: Option<&DatabaseCapability>) -> Result<&DatabaseCapability> {
    database.ok_or(RuntimeError::DatabaseUnavailable)
}

fn clipboard_error(error: arboard::Error) -> RuntimeError {
    RuntimeError::InvalidParameter(format!("clipboard access failed: {error}"))
}

fn parse_params<T>(params: &Value) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(params.clone()).map_err(Into::into)
}

fn required_string(params: &Value, key: &str) -> Result<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| RuntimeError::InvalidParameter(format!("missing string parameter '{key}'")))
}

fn optional_string_vec(params: &Value, key: &str) -> Result<Vec<String>> {
    let Some(value) = params.get(key) else {
        return Ok(Vec::new());
    };

    let Some(values) = value.as_array() else {
        return Err(RuntimeError::InvalidParameter(format!(
            "parameter '{key}' must be an array of strings"
        )));
    };

    values
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                RuntimeError::InvalidParameter(format!(
                    "parameter '{key}' must contain only strings"
                ))
            })
        })
        .collect()
}

fn build_file_dialog(
    fs_capability: &FsCapability,
    options: &DialogFileOptions,
    default_name: Option<&str>,
) -> Result<FileDialog> {
    let mut dialog = FileDialog::new();

    if let Some(title) = options.title.as_deref() {
        dialog = dialog.set_title(title);
    }

    if let Some(directory) = options.directory.as_deref() {
        dialog = dialog.set_directory(resolve_dialog_directory(fs_capability, directory)?);
    }

    if let Some(default_name) = default_name {
        dialog = dialog.set_file_name(default_name);
    }

    for filter in &options.filters {
        let extensions = filter
            .extensions
            .iter()
            .map(|value| value.trim().trim_start_matches('.').to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if extensions.is_empty() {
            continue;
        }
        dialog = dialog.add_filter(&filter.name, &extensions);
    }

    Ok(dialog)
}

fn build_save_dialog(
    fs_capability: &FsCapability,
    options: &DialogSaveOptions,
) -> Result<FileDialog> {
    build_file_dialog(
        fs_capability,
        &DialogFileOptions {
            title: options.title.clone(),
            directory: options.directory.clone(),
            filters: options.filters.clone(),
        },
        options.default_name.as_deref(),
    )
}

fn resolve_dialog_directory(fs_capability: &FsCapability, directory: &str) -> Result<PathBuf> {
    let path = PathBuf::from(directory);
    if path.is_absolute() {
        return Err(RuntimeError::PermissionDenied(
            "dialog directory must use a root:// or grant:// URI".into(),
        ));
    }

    let resolved = fs_capability.resolve(Path::new(directory))?;
    if !resolved.is_dir() {
        return Err(RuntimeError::InvalidParameter(format!(
            "dialog directory '{directory}' is not a directory"
        )));
    }
    Ok(resolved)
}

fn external_path_record(path: &Path) -> Result<ExternalPathRecord> {
    let metadata = fs::metadata(path).map_err(|error| {
        RuntimeError::InvalidParameter(format!(
            "unable to inspect external path '{}': {error}",
            path.display()
        ))
    })?;

    Ok(ExternalPathRecord {
        name: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        is_dir: metadata.is_dir(),
        is_file: metadata.is_file(),
        size: metadata.len(),
        extension: path
            .extension()
            .map(|value| value.to_string_lossy().to_string()),
        modified_at: external_modified_at(&metadata).ok(),
    })
}

fn external_modified_at(metadata: &fs::Metadata) -> std::io::Result<String> {
    let modified_at = metadata.modified()?;
    let timestamp = OffsetDateTime::from(modified_at)
        .format(&Rfc3339)
        .map_err(std::io::Error::other)?;
    Ok(timestamp)
}

fn normalize_window_label(value: Option<String>, next_window_index: u64) -> Result<String> {
    let Some(value) = value else {
        return Ok(format!("window-{next_window_index}"));
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidParameter(
            "window id must not be empty".into(),
        ));
    }

    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(RuntimeError::InvalidParameter(
            "window id may only contain letters, digits, underscores, and hyphens".into(),
        ));
    }

    Ok(trimmed.to_string())
}

fn validate_window_title(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidParameter(
            "window title must not be empty".into(),
        ));
    }

    Ok(trimmed.to_string())
}

fn validate_window_dimension(value: f64) -> Result<f64> {
    if !value.is_finite() || value <= 0.0 {
        return Err(RuntimeError::InvalidParameter(
            "window dimensions must be positive numbers".into(),
        ));
    }

    Ok(value)
}

fn normalize_window_route(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok("/".into());
    }

    if trimmed.contains("://") || trimmed.starts_with("//") {
        return Err(RuntimeError::PermissionDenied(
            "window.open only accepts in-app routes, not absolute URLs".into(),
        ));
    }

    let mut route_end = trimmed.len();
    for delimiter in ['?', '#'] {
        if let Some(index) = trimmed.find(delimiter) {
            route_end = route_end.min(index);
        }
    }
    let path = &trimmed[..route_end];
    let suffix = &trimmed[route_end..];

    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(RuntimeError::PermissionDenied(
                "window.open routes may not escape parent directories".into(),
            ));
        }
        segments.push(segment);
    }

    let mut normalized = String::from("/");
    normalized.push_str(&segments.join("/"));
    if normalized.len() > 1 && path.ends_with('/') {
        normalized.push('/');
    }
    normalized.push_str(suffix);

    Ok(normalized)
}

fn window_url(dev_url: Option<&str>, route: &str) -> String {
    if let Some(url) = dev_url {
        let base = url.trim_end_matches('/');
        format!("{base}{route}")
    } else if route == "/" {
        APP_URL.to_string()
    } else {
        format!("{}{}", APP_URL.trim_end_matches('/'), route)
    }
}

fn load_database_capability(
    assets: EmbeddedAssetRouter,
    app_id: Option<String>,
    data_dir: Option<PathBuf>,
    config: Option<EmbeddedDatabaseConfig>,
) -> Result<Option<DatabaseCapability>> {
    let Some(config) = config else {
        return Ok(None);
    };

    let app_id = app_id.ok_or_else(|| {
        RuntimeError::InvalidConfiguration(
            "database capability requires RustFrameBuilder::app_id(...)".into(),
        )
    })?;
    let schema_text = embedded_text_asset(assets, &config.schema_path)?;
    let schema = DatabaseSchema::from_json(&schema_text)?;
    let mut migration_files = Vec::new();
    for migration_path in &config.migration_paths {
        let migration_text = embedded_text_asset(assets, migration_path)?;
        migration_files.push(DatabaseMigrationFile::from_sql(
            migration_path.clone(),
            &migration_text,
        )?);
    }
    let mut seed_files = Vec::new();
    for seed_path in &config.seed_paths {
        let seed_text = embedded_text_asset(assets, seed_path)?;
        seed_files.push(DatabaseSeedFile::from_json(seed_path.clone(), &seed_text)?);
    }

    DatabaseCapability::open(DatabaseOpenConfig {
        app_id,
        data_dir,
        schema,
        migration_files,
        seed_files,
    })
    .map(Some)
}

fn embedded_text_asset(assets: EmbeddedAssetRouter, path: &str) -> Result<String> {
    let bytes = assets.get(path).ok_or_else(|| {
        RuntimeError::InvalidConfiguration(format!("embedded asset '{path}' is missing"))
    })?;

    String::from_utf8(bytes.into_owned()).map_err(|error| {
        RuntimeError::InvalidConfiguration(format!(
            "embedded asset '{path}' is not valid UTF-8: {error}"
        ))
    })
}

fn active_dev_url(configured: Option<String>) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }

    std::env::var("RUSTFRAME_DEV_URL").ok().or(configured)
}

fn resolve_window_state_path(app_id: Option<&str>, data_dir: Option<&Path>) -> Option<PathBuf> {
    resolve_runtime_data_path(app_id, data_dir, WINDOW_STATE_FILE_NAME)
}

fn resolve_runtime_data_path(
    app_id: Option<&str>,
    data_dir: Option<&Path>,
    file_name: &str,
) -> Option<PathBuf> {
    data_dir
        .map(Path::to_path_buf)
        .or_else(|| {
            app_id.and_then(|id| {
                dirs::data_local_dir()
                    .or_else(dirs::data_dir)
                    .map(|base| base.join(id))
            })
        })
        .map(|directory| directory.join(file_name))
}

fn asset_response(
    assets: EmbeddedAssetRouter,
    request: Request<Vec<u8>>,
    content_security_policy: Option<&str>,
) -> Response<Cow<'static, [u8]>> {
    let requested = normalize_asset_path(request.uri().path());
    let resolved = assets
        .get(&requested)
        .map(|bytes| (requested.clone(), bytes))
        .or_else(|| {
            if requested.contains('.') {
                None
            } else {
                assets
                    .get("index.html")
                    .map(|bytes| ("index.html".to_string(), bytes))
            }
        });

    match resolved {
        Some((path, bytes)) => {
            let mut response = Response::builder()
                .status(200)
                .header(CONTENT_TYPE, guess_mime(&path));
            if let Some(policy) = content_security_policy {
                response = response.header("Content-Security-Policy", policy);
            }
            response.body(bytes).unwrap()
        }
        None => Response::builder()
            .status(404)
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Cow::Borrowed(&b"Not Found"[..]))
            .unwrap(),
    }
}

fn normalize_asset_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html".into()
    } else {
        trimmed.into()
    }
}

fn guess_mime(path: &str) -> String {
    MimeGuess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> Result<WebView> {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    {
        builder.build(window).map_err(Into::into)
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;

        let vbox = window.default_vbox().ok_or_else(|| {
            RuntimeError::InvalidConfiguration("unable to create GTK container for webview".into())
        })?;
        builder.build_gtk(vbox).map_err(Into::into)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
fn prepare_linux_runtime() -> Result<()> {
    use gtk::prelude::DisplayExtManual;

    gtk::init().map_err(|error| RuntimeError::InvalidConfiguration(error.to_string()))?;

    let display = gtk::gdk::Display::default()
        .ok_or_else(|| RuntimeError::InvalidConfiguration("GTK display is not available".into()))?;

    if display.backend().is_wayland() {
        return Err(RuntimeError::InvalidConfiguration(
            "Wayland is not supported by this Linux x11 runtime yet".into(),
        ));
    }

    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
)))]
fn prepare_linux_runtime() -> Result<()> {
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
fn pump_linux_events() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
)))]
fn pump_linux_events() {}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap};

    use serde_json::json;
    use wry::http::Request;

    use crate::IpcRequest;

    use super::{
        EmbeddedAssetRouter, EmbeddedDatabaseConfig, FrontendSecurity, FrontendTrust,
        MethodExecution, ResolvedFrontendSecurity, SensitiveRateLimiter, WindowRecord,
        active_dev_url, asset_response, authorize_method, authorize_request, bridge_config_script,
        load_database_capability, method_execution, normalize_asset_path, normalize_window_route,
        window_pattern_matches, window_url,
    };

    fn fixture(path: &str) -> Option<Cow<'static, [u8]>> {
        match path {
            "data/schema.json" => Some(Cow::Borrowed(
                br#"{"version":1,"tables":[{"name":"notes","columns":[{"name":"title","type":"text","required":true}]}]}"#,
            )),
            "data/seeds/001-defaults.json" => Some(Cow::Borrowed(
                br#"{"entries":[{"table":"notes","rows":[{"title":"Welcome"}]}]}"#,
            )),
            _ => None,
        }
    }

    #[test]
    fn normalizes_root_asset_path() {
        assert_eq!(normalize_asset_path("/"), "index.html");
        assert_eq!(normalize_asset_path("/styles.css"), "styles.css");
    }

    #[test]
    fn routes_window_and_native_methods_to_the_expected_execution_context() {
        assert_eq!(
            method_execution("window.setTitle"),
            MethodExecution::MainThread
        );
        assert_eq!(
            method_execution("window.current"),
            MethodExecution::MainThread
        );
        assert_eq!(method_execution("window.list"), MethodExecution::MainThread);
        assert_eq!(method_execution("window.open"), MethodExecution::MainThread);
        assert_eq!(method_execution("db.list"), MethodExecution::Background);
        assert_eq!(method_execution("db.backup"), MethodExecution::Background);
        assert_eq!(method_execution("db.restore"), MethodExecution::Background);
        assert_eq!(method_execution("fs.openPath"), MethodExecution::Background);
        assert_eq!(
            method_execution("clipboard.readText"),
            MethodExecution::Background
        );
        assert_eq!(
            method_execution("clipboard.writeText"),
            MethodExecution::Background
        );
        assert_eq!(method_execution("shell.exec"), MethodExecution::Background);
        assert_eq!(method_execution("missing.method"), MethodExecution::Unknown);
    }

    #[test]
    fn rate_limits_sensitive_operations_per_window() {
        let mut limiter = SensitiveRateLimiter::default();
        for _ in 0..5 {
            limiter.check("main", "db.restore").unwrap();
        }
        let error = limiter.check("main", "db.restore").unwrap_err();
        assert!(error.to_string().contains("rate limited"));

        // A separate window and non-sensitive reads have independent budgets.
        limiter.check("reader-1", "db.restore").unwrap();
        for _ in 0..100 {
            limiter.check("main", "db.list").unwrap();
        }
    }

    #[test]
    fn normalizes_window_routes() {
        assert_eq!(normalize_window_route("/settings").unwrap(), "/settings");
        assert_eq!(
            normalize_window_route("settings/prefs?tab=general").unwrap(),
            "/settings/prefs?tab=general"
        );
        assert_eq!(normalize_window_route("./inspector").unwrap(), "/inspector");
    }

    #[test]
    fn rejects_unsafe_window_routes() {
        let absolute = normalize_window_route("https://example.com").unwrap_err();
        assert!(absolute.to_string().contains("only accepts in-app routes"));

        let parent_escape = normalize_window_route("../settings").unwrap_err();
        assert!(
            parent_escape
                .to_string()
                .contains("may not escape parent directories")
        );
    }

    #[test]
    fn builds_window_urls_for_embedded_and_dev_modes() {
        assert_eq!(window_url(None, "/settings"), "app://localhost/settings");
        assert_eq!(
            window_url(Some("http://127.0.0.1:5173"), "/inspector?id=1"),
            "http://127.0.0.1:5173/inspector?id=1"
        );
    }

    #[test]
    fn frontend_security_defaults_to_local_first() {
        let security = FrontendSecurity::default();

        assert_eq!(security.model, FrontendTrust::LocalFirst);
        assert!(security.database);
        assert!(security.filesystem);
        assert!(security.shell);
    }

    #[test]
    fn networked_frontend_blocks_database_shell_and_filesystem_methods() {
        let security = ResolvedFrontendSecurity {
            model: FrontendTrust::Networked,
            database: false,
            filesystem: false,
            shell: false,
            permissions: BTreeMap::new(),
        };

        let db_error = authorize_method("db.list", &security).unwrap_err();
        assert!(
            db_error
                .to_string()
                .contains("database bridge is disabled for this frontend")
        );

        let fs_error = authorize_method("fs.openPath", &security).unwrap_err();
        assert!(
            fs_error
                .to_string()
                .contains("filesystem bridge is disabled for this frontend")
        );

        let shell_error = authorize_method("shell.exec", &security).unwrap_err();
        assert!(
            shell_error
                .to_string()
                .contains("shell bridge is disabled for this frontend")
        );

        assert!(authorize_method("window.setTitle", &security).is_ok());
    }

    #[test]
    fn window_scoped_permissions_match_exact_and_declared_prefixes() {
        assert!(window_pattern_matches("main", "main"));
        assert!(window_pattern_matches("reader-*", "reader-42"));
        assert!(!window_pattern_matches("*", "reader-42"));
        assert!(!window_pattern_matches("reader-*", "settings"));
        assert!(!window_pattern_matches("reader-*-draft", "reader-1-draft"));

        let security = ResolvedFrontendSecurity {
            model: FrontendTrust::LocalFirst,
            database: true,
            filesystem: true,
            shell: false,
            permissions: BTreeMap::from([
                ("main".into(), vec!["db:write".into()]),
                ("reader-*".into(), vec!["db:read".into()]),
            ]),
        };
        let read = IpcRequest {
            id: 1,
            method: "db.list".into(),
            params: json!({ "table": "notes" }),
        };
        let write = IpcRequest {
            id: 2,
            method: "db.insert".into(),
            params: json!({ "table": "notes", "record": {} }),
        };

        assert!(authorize_request(&read, "reader-42", &security).is_ok());
        assert!(authorize_request(&write, "main", &security).is_ok());
        assert!(authorize_request(&write, "reader-42", &security).is_err());
        assert!(authorize_request(&read, "undeclared", &security).is_err());

        let no_permissions = ResolvedFrontendSecurity {
            permissions: BTreeMap::new(),
            ..security
        };
        assert!(authorize_request(&read, "main", &no_permissions).is_err());
    }

    #[test]
    fn filesystem_copy_and_watch_require_uri_scoped_permissions() {
        let copy = IpcRequest {
            id: 1,
            method: "fs.copyFrom".into(),
            params: json!({
                "sourcePath": "grant://source/report.md",
                "destinationPath": "root://workspace/imports/report.md"
            }),
        };
        let root_watch = IpcRequest {
            id: 2,
            method: "fs.watch".into(),
            params: json!({ "uri": "root://workspace" }),
        };
        let grant_watch = IpcRequest {
            id: 3,
            method: "fs.watch".into(),
            params: json!({ "uri": "grant://selected" }),
        };
        let security = |permissions: Vec<&str>| ResolvedFrontendSecurity {
            model: FrontendTrust::LocalFirst,
            database: false,
            filesystem: true,
            shell: false,
            permissions: BTreeMap::from([(
                "main".into(),
                permissions.into_iter().map(str::to_string).collect(),
            )]),
        };

        assert!(authorize_request(&copy, "main", &security(vec!["fs:workspace:write"])).is_err());
        assert!(
            authorize_request(
                &copy,
                "main",
                &security(vec!["fs:workspace:write", "fs:grants:read"]),
            )
            .is_ok()
        );
        assert!(
            authorize_request(&root_watch, "main", &security(vec!["fs:workspace:watch"]),).is_ok()
        );
        assert!(
            authorize_request(&grant_watch, "main", &security(vec!["fs:workspace:watch"]),)
                .is_err()
        );
        assert!(
            authorize_request(&grant_watch, "main", &security(vec!["fs:grants:watch"]),).is_ok()
        );
    }

    #[test]
    fn bridge_config_script_serializes_frontend_security() {
        let script = bridge_config_script(
            &ResolvedFrontendSecurity {
                model: FrontendTrust::Networked,
                database: true,
                filesystem: false,
                shell: false,
                permissions: BTreeMap::new(),
            },
            &WindowRecord {
                id: "settings".into(),
                title: "Settings".into(),
                route: "/settings".into(),
                width: 720.0,
                height: 540.0,
                is_primary: false,
            },
        )
        .unwrap();

        assert!(script.contains("\"model\":\"networked\""));
        assert!(script.contains("\"database\":true"));
        assert!(script.contains("\"filesystem\":false"));
        assert!(script.contains("\"shell\":false"));
        assert!(script.contains("\"currentWindow\":{"));
        assert!(script.contains("\"id\":\"settings\""));
    }

    #[test]
    fn prefers_environment_dev_url() {
        let previous = std::env::var("RUSTFRAME_DEV_URL").ok();
        unsafe {
            std::env::set_var("RUSTFRAME_DEV_URL", "http://127.0.0.1:4321");
        }
        assert_eq!(
            active_dev_url(Some("http://127.0.0.1:5173".into())).as_deref(),
            Some("http://127.0.0.1:4321")
        );
        if let Some(value) = previous {
            unsafe {
                std::env::set_var("RUSTFRAME_DEV_URL", value);
            }
        } else {
            unsafe {
                std::env::remove_var("RUSTFRAME_DEV_URL");
            }
        }
    }

    #[test]
    fn loads_embedded_database_from_assets() {
        let temp = tempfile::tempdir().unwrap();
        let database = load_database_capability(
            EmbeddedAssetRouter { fetch: fixture },
            Some("orbit_desk".into()),
            Some(temp.path().join("data")),
            Some(EmbeddedDatabaseConfig {
                schema_path: "data/schema.json".into(),
                seed_paths: vec!["data/seeds/001-defaults.json".into()],
                migration_paths: Vec::new(),
            }),
        )
        .unwrap()
        .unwrap();

        let rows = database
            .list(&crate::DatabaseListQuery {
                table: "notes".into(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["title"], "Welcome");
    }

    #[test]
    fn asset_response_falls_back_to_index_for_extensionless_routes() {
        fn fixture(path: &str) -> Option<Cow<'static, [u8]>> {
            match path {
                "index.html" => Some(Cow::Borrowed(b"<html>ok</html>")),
                _ => None,
            }
        }

        let response = asset_response(
            EmbeddedAssetRouter { fetch: fixture },
            Request::builder()
                .uri("app://localhost/dashboard")
                .body(Vec::new())
                .unwrap(),
            Some("default-src 'self'; object-src 'none'"),
        );

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            "default-src 'self'; object-src 'none'"
        );
    }

    #[test]
    fn asset_response_returns_not_found_for_missing_extension_asset() {
        let response = asset_response(
            EmbeddedAssetRouter { fetch: |_| None },
            Request::builder()
                .uri("app://localhost/missing.js")
                .body(Vec::new())
                .unwrap(),
            None,
        );

        assert_eq!(response.status(), 404);
    }

    #[test]
    fn database_loader_returns_none_when_database_is_not_configured() {
        let result =
            load_database_capability(EmbeddedAssetRouter { fetch: fixture }, None, None, None)
                .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn database_loader_requires_app_id_when_database_is_enabled() {
        let error = load_database_capability(
            EmbeddedAssetRouter { fetch: fixture },
            None,
            None,
            Some(EmbeddedDatabaseConfig {
                schema_path: "data/schema.json".into(),
                seed_paths: Vec::new(),
                migration_paths: Vec::new(),
            }),
        )
        .unwrap_err();

        assert!(error.to_string().contains("app_id"));
    }
}
