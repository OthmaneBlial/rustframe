use std::{borrow::Cow, collections::BTreeMap, path::PathBuf, sync::mpsc, thread};

use mime_guess::MimeGuess;
use serde::Deserialize;
use serde_json::{Value, json};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder},
};
use wry::{
    NewWindowResponse, WebView, WebViewBuilder,
    http::{Request, Response, header::CONTENT_TYPE},
};

use crate::{
    DatabaseCapability, DatabaseListQuery, DatabaseMigrationFile, DatabaseOpenConfig,
    DatabaseSchema, DatabaseSeedFile, FsCapability, IpcRequest, IpcResponse, Result, RuntimeError,
    ShellCapability, ShellCommand,
};

const APP_URL: &str = "app://localhost/";

pub trait EmbeddedAssets {
    fn get(path: &str) -> Option<Cow<'static, [u8]>>;
}

#[derive(Clone, Debug)]
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

#[derive(Default)]
pub struct RustFrameBuilder {
    window: WindowOptions,
    app_id: Option<String>,
    data_dir: Option<PathBuf>,
    dev_url: Option<String>,
    assets: Option<EmbeddedAssetRouter>,
    database: Option<EmbeddedDatabaseConfig>,
    fs_roots: Vec<PathBuf>,
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

    pub fn allow_shell_command<I, S>(
        mut self,
        name: impl Into<String>,
        program: impl Into<String>,
        args: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.shell_commands
            .insert(name.into(), ShellCommand::new(program, args));
        self
    }

    pub fn run(self) -> Result<()> {
        let assets = self.assets.ok_or(RuntimeError::MissingAssets)?;
        let fs_capability = FsCapability::new(self.fs_roots)?;
        let shell_capability = ShellCapability::new(self.shell_commands);
        let database_capability =
            load_database_capability(assets, self.app_id, self.data_dir, self.database)?;

        prepare_linux_runtime()?;

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let ipc_proxy = event_loop.create_proxy();
        let worker = IpcWorker::spawn(
            ipc_proxy.clone(),
            fs_capability,
            shell_capability,
            database_capability,
        )?;
        let window = WindowBuilder::new()
            .with_title(&self.window.title)
            .with_inner_size(tao::dpi::LogicalSize::new(
                self.window.width,
                self.window.height,
            ))
            .build(&event_loop)?;

        let builder = WebViewBuilder::new()
            .with_background_color((6, 9, 18, 255))
            .with_custom_protocol("app".into(), move |_id, request| {
                asset_response(assets, request)
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_ipc_handler(move |request| {
                let _ = ipc_proxy.send_event(UserEvent::Ipc(request.body().clone()));
            });

        let dev_url = active_dev_url(self.dev_url);
        let builder = match dev_url {
            Some(url) => builder.with_url(url),
            None => builder.with_url(APP_URL),
        };

        let webview = build_webview(builder, &window)?;
        let mut pending_exit = false;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::UserEvent(UserEvent::Ipc(body)) => {
                    if let Some(outcome) = dispatch_ipc_message(&body, &window, &worker) {
                        resolve_ipc_response(&webview, &outcome.response);
                        if outcome.should_exit {
                            pending_exit = true;
                        }
                    }
                }
                Event::UserEvent(UserEvent::IpcResponse(response)) => {
                    resolve_ipc_response(&webview, &response);
                }
                Event::MainEventsCleared => {
                    if pending_exit {
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

enum UserEvent {
    Ipc(String),
    IpcResponse(IpcResponse),
}

struct IpcOutcome {
    response: IpcResponse,
    should_exit: bool,
}

struct IpcWorker {
    sender: mpsc::Sender<IpcRequest>,
}

impl IpcWorker {
    fn spawn(
        proxy: EventLoopProxy<UserEvent>,
        fs_capability: FsCapability,
        shell_capability: ShellCapability,
        database_capability: Option<DatabaseCapability>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<IpcRequest>();
        thread::Builder::new()
            .name("rustframe-ipc-worker".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let response = execute_background_request(
                        request,
                        &fs_capability,
                        &shell_capability,
                        database_capability.as_ref(),
                    );

                    if proxy.send_event(UserEvent::IpcResponse(response)).is_err() {
                        break;
                    }
                }
            })?;

        Ok(Self { sender })
    }

    fn dispatch(&self, request: IpcRequest) -> Result<()> {
        self.sender.send(request).map_err(|_| {
            RuntimeError::InvalidConfiguration("background IPC worker is unavailable".into())
        })
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
struct DbGetParams {
    table: String,
    id: i64,
}

fn dispatch_ipc_message(body: &str, window: &Window, worker: &IpcWorker) -> Option<IpcOutcome> {
    match serde_json::from_str::<IpcRequest>(body) {
        Ok(request) => dispatch_request(request, window, worker),
        Err(error) => Some(IpcOutcome {
            response: IpcResponse::failure(0, &RuntimeError::Json(error)),
            should_exit: false,
        }),
    }
}

fn dispatch_request(
    request: IpcRequest,
    window: &Window,
    worker: &IpcWorker,
) -> Option<IpcOutcome> {
    match method_execution(&request.method) {
        MethodExecution::MainThread => Some(handle_main_thread_request(request, window)),
        MethodExecution::Background => {
            let request_id = request.id;
            match worker.dispatch(request) {
                Ok(()) => None,
                Err(error) => Some(IpcOutcome {
                    response: IpcResponse::failure(request_id, &error),
                    should_exit: false,
                }),
            }
        }
        MethodExecution::Unknown => Some(IpcOutcome {
            response: IpcResponse::failure(
                request.id,
                &RuntimeError::UnknownMethod(request.method),
            ),
            should_exit: false,
        }),
    }
}

fn resolve_ipc_response(webview: &WebView, response: &IpcResponse) {
    if let Ok(serialized) = serde_json::to_string(response) {
        let script = format!("window.RustFrame.__resolveFromNative({serialized});");
        let _ = webview.evaluate_script(&script);
    }
}

fn method_execution(method: &str) -> MethodExecution {
    match method {
        "window.close" | "window.minimize" | "window.maximize" | "window.setTitle" => {
            MethodExecution::MainThread
        }
        "fs.readText" | "shell.exec" | "db.info" | "db.get" | "db.list" | "db.count"
        | "db.insert" | "db.update" | "db.delete" => MethodExecution::Background,
        _ => MethodExecution::Unknown,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MethodExecution {
    MainThread,
    Background,
    Unknown,
}

fn handle_main_thread_request(request: IpcRequest, window: &Window) -> IpcOutcome {
    let mut should_exit = false;
    let result: Result<Value> = match request.method.as_str() {
        "window.close" => {
            should_exit = true;
            Ok(Value::Null)
        }
        "window.minimize" => {
            window.set_minimized(true);
            Ok(Value::Null)
        }
        "window.maximize" => {
            window.set_maximized(true);
            Ok(Value::Null)
        }
        "window.setTitle" => (|| {
            let title = required_string(&request.params, "title")?;
            window.set_title(&title);
            Ok(Value::Null)
        })(),
        _ => Err(RuntimeError::UnknownMethod(request.method.clone())),
    };

    let response = match result {
        Ok(data) => IpcResponse::success(request.id, data),
        Err(error) => IpcResponse::failure(request.id, &error),
    };

    IpcOutcome {
        response,
        should_exit,
    }
}

fn execute_background_request(
    request: IpcRequest,
    fs_capability: &FsCapability,
    shell_capability: &ShellCapability,
    database_capability: Option<&DatabaseCapability>,
) -> IpcResponse {
    let result: Result<Value> = match request.method.as_str() {
        "fs.readText" => (|| {
            let path = required_string(&request.params, "path")?;
            let content = fs_capability.read_text(path)?;
            Ok(Value::String(content))
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
        method => Err(RuntimeError::UnknownMethod(method.to_string())),
    };

    match result {
        Ok(data) => IpcResponse::success(request.id, data),
        Err(error) => IpcResponse::failure(request.id, &error),
    }
}

fn database(database: Option<&DatabaseCapability>) -> Result<&DatabaseCapability> {
    database.ok_or(RuntimeError::DatabaseUnavailable)
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
        RuntimeError::InvalidConfiguration(format!("embedded asset '{}' is missing", path))
    })?;

    String::from_utf8(bytes.into_owned()).map_err(|error| {
        RuntimeError::InvalidConfiguration(format!(
            "embedded asset '{}' is not valid UTF-8: {}",
            path, error
        ))
    })
}

fn active_dev_url(configured: Option<String>) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }

    std::env::var("RUSTFRAME_DEV_URL").ok().or(configured)
}

fn asset_response(
    assets: EmbeddedAssetRouter,
    request: Request<Vec<u8>>,
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
        Some((path, bytes)) => Response::builder()
            .status(200)
            .header(CONTENT_TYPE, guess_mime(&path))
            .body(bytes)
            .unwrap(),
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
        return builder.build(window).map_err(Into::into);
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
            "Wayland is not supported by this Linux-first x11 runtime yet".into(),
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
    use std::borrow::Cow;

    use wry::http::Request;

    use super::{
        EmbeddedAssetRouter, EmbeddedDatabaseConfig, MethodExecution, active_dev_url,
        asset_response, load_database_capability, method_execution, normalize_asset_path,
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
        assert_eq!(method_execution("db.list"), MethodExecution::Background);
        assert_eq!(method_execution("shell.exec"), MethodExecution::Background);
        assert_eq!(method_execution("missing.method"), MethodExecution::Unknown);
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
        );

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    }

    #[test]
    fn asset_response_returns_not_found_for_missing_extension_asset() {
        let response = asset_response(
            EmbeddedAssetRouter { fetch: |_| None },
            Request::builder()
                .uri("app://localhost/missing.js")
                .body(Vec::new())
                .unwrap(),
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
