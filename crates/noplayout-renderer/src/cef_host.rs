//! Everything that touches CEF: library loading, subprocess dispatch, `initialize`, the one
//! windowless browser and shutdown. Kept apart from the control-socket logic in `main.rs`.

use cef::rc::Rc as _;
use cef::*;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// CEF version string of the headers this binary was built against, e.g.
/// `156.0.31+ge603d97+chromium-156.0.8078.100`. `api_hash` at start-up refuses a libcef from a
/// different API version, so this is also the version that runs.
pub const CEF_VERSION_FULL: &str = {
    let bytes = cef::sys::CEF_VERSION;
    // Strip the trailing NUL of the C string constant.
    match std::str::from_utf8(bytes.split_last().expect("CEF_VERSION is not empty").1) {
        Ok(s) => s,
        Err(_) => panic!("CEF_VERSION is not UTF-8"),
    }
};

/// External message pump interval. M0 pumps on a fixed cadence; M1 replaces this with
/// `on_schedule_message_pump_work` scheduling and the output clock's drive modes (PLAN §5.2).
// TODO(M1): scheduled message pump. (STATUS.md, "M0 limits")
pub const PUMP_INTERVAL: Duration = Duration::from_millis(5);

/// Program page size. M0 fixes 1920x1080; `Configure{format}` sets it from M1.
const VIEW_W: i32 = 1920;
const VIEW_H: i32 = 1080;

/// `true` when Chromium launched this executable as one of its helper subprocesses.
pub fn is_subprocess(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a.starts_with("--type="))
}

#[cfg(target_os = "macos")]
fn load_framework(helper: bool) -> Result<library_loader::LibraryLoader, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find own executable: {e}"))?;
    // LibraryLoader panics if the framework path does not exist; check first so an unbundled
    // run gets a plain message instead.
    let dir = exe.parent().ok_or("executable has no parent directory")?;
    let framework = if helper {
        dir.join("../../..")
    } else {
        dir.join("../Frameworks")
    }
    .join("Chromium Embedded Framework.framework");
    if !framework.exists() {
        return Err(format!(
            "the CEF framework was not found next to {} (expected {}); on macOS the renderer \
             must run from its app bundle: build it with `cargo xtask bundle`",
            exe.display(),
            framework.display()
        ));
    }
    let loader = library_loader::LibraryLoader::new(&exe, helper);
    if !loader.load() {
        return Err(format!(
            "could not load the CEF framework at {}",
            framework.display()
        ));
    }
    Ok(loader)
}

/// Entry point for CEF helper subprocesses (renderer, GPU, utility...).
pub fn run_subprocess() -> ExitCode {
    let args = args::Args::new();

    #[cfg(target_os = "macos")]
    let _sandbox = {
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    };
    #[cfg(target_os = "macos")]
    let _loader = match load_framework(true) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("noplayout-renderer helper: {e}");
            return ExitCode::FAILURE;
        }
    };

    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let mut app = RendererAppBuilder::build(RendererApp);
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1))
}

/// The browser process's CEF: initialised, with one windowless browser on `about:blank`.
pub struct Cef {
    browser: Browser,
    paints: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    _loader: library_loader::LibraryLoader,
}

impl Cef {
    pub fn start(cache_dir: &Path) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        let loader = load_framework(false)?;

        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
        let args = args::Args::new();
        let mut app = RendererAppBuilder::build(RendererApp);
        let ret = execute_process(
            Some(args.as_main_args()),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if ret != -1 {
            return Err(format!(
                "CEF treated the browser process as a subprocess (code {ret})"
            ));
        }

        std::fs::create_dir_all(cache_dir)
            .map_err(|e| format!("cannot create {}: {e}", cache_dir.display()))?;
        let cache = cache_dir
            .canonicalize()
            .map_err(|e| format!("cannot resolve {}: {e}", cache_dir.display()))?;
        let settings = Settings {
            windowless_rendering_enabled: 1,
            external_message_pump: 1,
            // The Windows sandbox needs the bootstrap executable and Linux needs a SUID helper;
            // both are packaging work (M6). macOS helpers run sandboxed (see run_subprocess).
            // TODO(M6): sandbox on Windows and Linux. (STATUS.md, "M0 limits")
            no_sandbox: i32::from(!cfg!(target_os = "macos")),
            root_cache_path: CefString::from(cache.to_string_lossy().as_ref()),
            log_severity: LogSeverity::WARNING,
            ..Default::default()
        };
        if initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        ) != 1
        {
            return Err(format!(
                "CEF failed to initialise (exit code {})",
                get_exit_code()
            ));
        }

        let paints = Arc::new(AtomicU64::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let window_info = WindowInfo {
            windowless_rendering_enabled: 1,
            ..Default::default()
        };
        let browser_settings = BrowserSettings {
            windowless_frame_rate: 30,
            // Fully transparent background: graphics are keyed (IDEA §4).
            background_color: 0,
            ..Default::default()
        };
        let mut client = RendererClientBuilder::build(
            OsrHandler {
                paints: paints.clone(),
            },
            Lifespan {
                closed: closed.clone(),
            },
        );
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from("about:blank")),
            Some(&browser_settings),
            None,
            None,
        )
        .ok_or("CEF could not create the offscreen browser")?;

        Ok(Self {
            browser,
            paints,
            closed,
            #[cfg(target_os = "macos")]
            _loader: loader,
        })
    }

    /// Run one slice of CEF's message loop.
    pub fn pump(&self) {
        do_message_loop_work();
    }

    /// Paints received from CEF so far.
    pub fn paints(&self) -> u64 {
        self.paints.load(Ordering::Relaxed)
    }

    /// Close the browser, let CEF finish, and shut CEF down.
    pub fn shutdown(self) {
        if let Some(host) = self.browser.host() {
            host.close_browser(1);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while !self.closed.load(Ordering::Acquire) && Instant::now() < deadline {
            do_message_loop_work();
            std::thread::sleep(PUMP_INTERVAL);
        }
        if !self.closed.load(Ordering::Acquire) {
            eprintln!("noplayout-renderer: browser did not close within 3 s; shutting CEF down");
        }
        // A few more turns let CEF release the browser before shutdown.
        for _ in 0..10 {
            do_message_loop_work();
            std::thread::sleep(PUMP_INTERVAL);
        }
        drop(self.browser);
        cef::shutdown();
    }
}

#[derive(Clone)]
struct RendererApp;

wrap_app! {
    struct RendererAppBuilder {
        app: RendererApp,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefStringUtf16>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };
            for switch in ["no-startup-window", "noerrdialogs", "hide-crash-restore-bubble"] {
                command_line.append_switch(Some(&switch.into()));
            }
            // No keychain prompts from a headless renderer on macOS.
            if cfg!(target_os = "macos") {
                command_line.append_switch(Some(&"use-mock-keychain".into()));
            }
        }
    }
}

impl RendererAppBuilder {
    fn build(app: RendererApp) -> App {
        Self::new(app)
    }
}

#[derive(Clone)]
struct OsrHandler {
    paints: Arc<AtomicU64>,
}

wrap_render_handler! {
    struct OsrHandlerBuilder {
        handler: OsrHandler,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                rect.x = 0;
                rect.y = 0;
                rect.width = VIEW_W;
                rect.height = VIEW_H;
            }
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            // M0 only counts paints; M1 copies them into the paint ring (E-042).
            if !buffer.is_null() && width > 0 && height > 0 {
                self.handler.paints.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[derive(Clone)]
struct Lifespan {
    closed: Arc<AtomicBool>,
}

wrap_life_span_handler! {
    struct LifespanBuilder {
        handler: Lifespan,
    }

    impl LifeSpanHandler {
        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.handler.closed.store(true, Ordering::Release);
        }
    }
}

wrap_client! {
    struct RendererClientBuilder {
        render_handler: RenderHandler,
        life_span_handler: LifeSpanHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }
    }
}

impl RendererClientBuilder {
    fn build(osr: OsrHandler, lifespan: Lifespan) -> Client {
        Self::new(OsrHandlerBuilder::new(osr), LifespanBuilder::new(lifespan))
    }
}
