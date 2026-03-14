use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use mime_guess::MimeGuess;
use serde_json::{Value, json};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
use wry::{
    NewWindowResponse, WebView, WebViewBuilder,
    http::{Request, Response, header::CONTENT_TYPE},
};

use crate::{
    FsCapability, IpcRequest, IpcResponse, Result, RuntimeError, ShellCapability, ShellCommand,
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

#[derive(Default)]
pub struct RustFrameBuilder {
    window: WindowOptions,
    dev_url: Option<String>,
    assets: Option<EmbeddedAssetRouter>,
    fs_roots: Vec<PathBuf>,
    shell_commands: BTreeMap<String, ShellCommand>,
}

impl RustFrameBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.window.title = title.into();
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

        prepare_linux_runtime()?;

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        let window = WindowBuilder::new()
            .with_title(&self.window.title)
            .with_inner_size(tao::dpi::LogicalSize::new(
                self.window.width,
                self.window.height,
            ))
            .build(&event_loop)?;

        let builder = WebViewBuilder::new()
            .with_custom_protocol("app".into(), move |_id, request| {
                asset_response(assets, request)
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_ipc_handler(move |request| {
                let _ = proxy.send_event(UserEvent::Ipc(request.body().clone()));
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
                    let outcome =
                        handle_ipc_message(&body, &window, &fs_capability, &shell_capability);

                    if let Ok(serialized) = serde_json::to_string(&outcome.response) {
                        let script = format!("window.RustFrame.__resolveFromNative({serialized});");
                        let _ = webview.evaluate_script(&script);
                    }

                    if outcome.should_exit {
                        pending_exit = true;
                    }
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
}

struct IpcOutcome {
    response: IpcResponse,
    should_exit: bool,
}

fn handle_ipc_message(
    body: &str,
    window: &Window,
    fs_capability: &FsCapability,
    shell_capability: &ShellCapability,
) -> IpcOutcome {
    match serde_json::from_str::<IpcRequest>(body) {
        Ok(request) => handle_request(request, window, fs_capability, shell_capability),
        Err(error) => IpcOutcome {
            response: IpcResponse::failure(0, &RuntimeError::Json(error)),
            should_exit: false,
        },
    }
}

fn handle_request(
    request: IpcRequest,
    window: &Window,
    fs_capability: &FsCapability,
    shell_capability: &ShellCapability,
) -> IpcOutcome {
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
        method => Err(RuntimeError::UnknownMethod(method.to_string())),
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
