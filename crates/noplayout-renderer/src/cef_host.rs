//! Everything that touches CEF: library loading, subprocess dispatch, `initialize`, the program
//! browser (transparent OSR, PLAN §2.4) and shutdown. Kept apart from the control logic in
//! `main.rs`; CEF calls back into the main loop only through [`Wake`] messages.

use crate::Wake;
use cef::rc::Rc as _;
use cef::*;
use noplayout_frames::{Writer, clock};
use noplayout_proto::{DriveMode, Event, RendererConfig};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
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

/// Upper bound between message-loop turns when CEF schedules nothing (CEF's own guidance for
/// `external_message_pump` is a 1/30 s fallback).
pub const PUMP_FALLBACK: Duration = Duration::from_millis(33);

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
    let mut app = RendererAppBuilder::build(RendererApp { wake: None });
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    ExitCode::from(u8::try_from(code.clamp(0, 255)).unwrap_or(1))
}

/// State shared between CEF's callbacks (UI thread) and the main loop.
pub struct Shared {
    pub paints: AtomicU64,
    pub last_paint_ns: AtomicU64,
    /// Paints that did not fit the ring (wrong size) or found no slot.
    pub paints_dropped: AtomicU64,
    pub width: AtomicU32,
    pub height: AtomicU32,
    pub ring: Mutex<Option<Writer>>,
    /// Single-use page identity, injected after the main frame loads (E-046).
    pub launch: Mutex<Option<(u64, String)>>,
    pub closed: AtomicBool,
    pub wake: Sender<Wake>,
}

/// The browser process's CEF with at most one program browser.
pub struct Cef {
    pub shared: Arc<Shared>,
    browser: Option<Browser>,
    #[cfg(target_os = "macos")]
    _loader: library_loader::LibraryLoader,
}

impl Cef {
    pub fn start(cache_dir: &Path, wake: Sender<Wake>) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        let loader = load_framework(false)?;

        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
        let args = args::Args::new();
        let mut app = RendererAppBuilder::build(RendererApp {
            wake: Some(wake.clone()),
        });
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
        // TODO(M2): PLAN §2.4 runs CEF's own loop on Windows/Linux; M1 uses the scheduled
        // external pump on every OS (one code path while the drive modes are measured).
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
        Ok(Self {
            shared: Arc::new(Shared {
                paints: AtomicU64::new(0),
                last_paint_ns: AtomicU64::new(0),
                paints_dropped: AtomicU64::new(0),
                width: AtomicU32::new(1920),
                height: AtomicU32::new(1080),
                ring: Mutex::new(None),
                launch: Mutex::new(None),
                closed: AtomicBool::new(false),
                wake,
            }),
            browser: None,
            #[cfg(target_os = "macos")]
            _loader: loader,
        })
    }

    /// Create the ring and open the program browser (PLAN §2.4). Replaces any earlier browser.
    pub fn open_program(&mut self, cfg: &RendererConfig) -> Result<(), String> {
        if !cfg
            .page_token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("page token has characters outside [A-Za-z0-9_-]".into());
        }
        let (w, h) = (cfg.format.width, cfg.format.height);
        let writer = Writer::create(&cfg.ring, cfg.page_inc, w, h)
            .map_err(|e| format!("cannot create paint ring {}: {e}", cfg.ring))?;
        *self.shared.ring.lock().unwrap_or_else(|p| p.into_inner()) = Some(writer);
        self.shared.width.store(w, Ordering::Relaxed);
        self.shared.height.store(h, Ordering::Relaxed);
        *self.shared.launch.lock().unwrap_or_else(|p| p.into_inner()) =
            Some((cfg.page_inc, cfg.page_token.clone()));

        let rate = clock::Rate {
            num: cfg.format.rate_num,
            den: cfg.format.rate_den,
        };
        let window_info = WindowInfo {
            windowless_rendering_enabled: 1,
            shared_texture_enabled: 0,
            external_begin_frame_enabled: i32::from(cfg.mode == DriveMode::A),
            ..Default::default()
        };
        let browser_settings = BrowserSettings {
            windowless_frame_rate: frame_rate(cfg.mode, cfg.active, rate),
            // Fully transparent background: graphics are keyed (IDEA §4). CEF allows only
            // 0x00 or 0xFF alpha.
            background_color: 0,
            ..Default::default()
        };
        let mut client = RendererClientBuilder::build(self.shared.clone());
        if let Some(old) = self.browser.take()
            && let Some(host) = old.host()
        {
            host.close_browser(1);
        }
        self.shared.closed.store(false, Ordering::Release);
        let browser = browser_host_create_browser_sync(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(cfg.url.as_str())),
            Some(&browser_settings),
            None,
            None,
        )
        .ok_or("CEF could not create the offscreen program browser")?;
        if let Some(host) = browser.host() {
            // Audio is a v1 non-goal.
            host.set_audio_muted(1);
        }
        self.browser = Some(browser);
        Ok(())
    }

    fn host(&self) -> Option<BrowserHost> {
        self.browser.as_ref().and_then(|b| b.host())
    }

    /// Mode A: one external begin-frame (called on the UI thread).
    pub fn begin_frame(&self) {
        if let Some(h) = self.host() {
            h.send_external_begin_frame();
        }
    }

    /// Paint probe (E-057): ask for a full-view repaint whatever the damage.
    pub fn invalidate(&self) {
        if let Some(h) = self.host() {
            h.invalidate(PaintElementType::VIEW);
        }
    }

    /// Mode B promotion/demotion: `ceil(fps)` when active, 10 when standby.
    pub fn set_frame_rate(&self, mode: DriveMode, active: bool, rate: clock::Rate) {
        if let Some(h) = self.host() {
            h.set_windowless_frame_rate(frame_rate(mode, active, rate));
        }
    }

    /// Run script in the program page's main frame.
    pub fn run_script(&self, code: &str) {
        if let Some(f) = self.browser.as_ref().and_then(|b| b.main_frame()) {
            f.execute_java_script(Some(&CefString::from(code)), None, 0);
        }
    }

    /// Run one slice of CEF's message loop.
    pub fn pump(&self) {
        do_message_loop_work();
    }

    pub fn has_program(&self) -> bool {
        self.browser.is_some()
    }

    /// Close the browser, let CEF finish, and shut CEF down.
    pub fn shutdown(mut self) {
        if let Some(host) = self.host() {
            host.close_browser(1);
            let deadline = Instant::now() + Duration::from_secs(3);
            while !self.shared.closed.load(Ordering::Acquire) && Instant::now() < deadline {
                do_message_loop_work();
                std::thread::sleep(Duration::from_millis(5));
            }
            if !self.shared.closed.load(Ordering::Acquire) {
                eprintln!(
                    "noplayout-renderer: browser did not close within 3 s; shutting CEF down"
                );
            }
        }
        // A few more turns let CEF release the browser before shutdown.
        for _ in 0..10 {
            do_message_loop_work();
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(self.browser.take());
        cef::shutdown();
    }
}

/// `windowless_frame_rate` per PLAN §2.4. In mode A CEF paints on begin-frames and ignores it.
fn frame_rate(mode: DriveMode, active: bool, rate: clock::Rate) -> i32 {
    match (mode, active) {
        (DriveMode::B, false) => 10,
        _ => i32::try_from(rate.ceil_fps()).unwrap_or(60),
    }
}

#[derive(Clone)]
struct RendererApp {
    wake: Option<Sender<Wake>>,
}

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

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            self.app
                .wake
                .clone()
                .map(PumpScheduler::new)
        }
    }
}

impl RendererAppBuilder {
    fn build(app: RendererApp) -> App {
        Self::new(app)
    }
}

wrap_browser_process_handler! {
    struct PumpScheduler {
        wake: Sender<Wake>,
    }

    impl BrowserProcessHandler {
        // May be called from any thread; the main loop turns it into a deadline.
        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            let _ = self.wake.send(Wake::Pump(delay_ms));
        }
    }
}

wrap_render_handler! {
    struct OsrHandler {
        shared: Arc<Shared>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                rect.x = 0;
                rect.y = 0;
                rect.width = self.shared.width.load(Ordering::Relaxed) as i32;
                rect.height = self.shared.height.load(Ordering::Relaxed) as i32;
            }
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            let painted_at = clock::mono_ns();
            if type_ != PaintElementType::VIEW || buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let s = &self.shared;
            s.paints.fetch_add(1, Ordering::Relaxed);
            s.last_paint_ns.store(painted_at, Ordering::Relaxed);
            let (w, h) = (width as u32, height as u32);
            let len = w as usize * h as usize * 4;
            let mut ring = s.ring.lock().unwrap_or_else(|p| p.into_inner());
            let Some(writer) = ring.as_mut() else {
                return; // no Configure yet (self-test): counting is all
            };
            // CEF's buffer is BGRA, premultiplied, upper-left origin, tightly packed, and only
            // valid inside this callback: the one mandatory copy goes straight into the ring.
            if writer.slot_len() < len {
                s.paints_dropped.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let Some(mut slot) = writer.begin() else {
                s.paints_dropped.fetch_add(1, Ordering::Relaxed);
                return;
            };
            // SAFETY: CEF guarantees `width * height * 4` readable bytes at `buffer`.
            let src = unsafe { std::slice::from_raw_parts(buffer, len) };
            slot.pixels()[..len].copy_from_slice(src);
            slot.publish(painted_at, w, h, w * 4);
        }
    }
}

wrap_life_span_handler! {
    struct Lifespan {
        shared: Arc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.shared.closed.store(true, Ordering::Release);
        }
    }
}

wrap_load_handler! {
    struct Loads {
        shared: Arc<Shared>,
    }

    impl LoadHandler {
        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            let Some(frame) = frame else { return };
            if frame.is_main() != 1 {
                return;
            }
            // Hand the page its identity once, by script, never in the URL (E-046). The
            // token's alphabet was checked in `open_program`.
            let launch = self.shared.launch.lock().unwrap_or_else(|p| p.into_inner()).take();
            if let Some((inc, token)) = launch {
                let code = format!(
                    "window.noplayoutLaunch && window.noplayoutLaunch({{inc:{inc},token:\"{token}\"}})"
                );
                frame.execute_java_script(Some(&CefString::from(code.as_str())), None, 0);
            }
            let _ = self.shared.wake.send(Wake::Event(Event::PageLoaded {
                http_status: http_status_code,
            }));
        }
    }
}

wrap_display_handler! {
    struct Console {
        shared: Arc<Shared>,
    }

    impl DisplayHandler {
        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let text = message.map(|m| m.to_string()).unwrap_or_default();
            let error = level.get_raw() >= LogSeverity::ERROR.get_raw();
            let _ = self.shared.wake.send(Wake::Event(Event::Console { error, text }));
            0
        }
    }
}

wrap_request_handler! {
    struct Requests {
        shared: Arc<Shared>,
    }

    impl RequestHandler {
        fn on_render_process_terminated(
            &self,
            _browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: ::std::os::raw::c_int,
            _error_string: Option<&CefString>,
        ) {
            let _ = self.shared.wake.send(Wake::Event(Event::PageTerminated {
                status: format!("{status:?} (code {error_code})"),
            }));
        }
    }
}

wrap_client! {
    struct RendererClientBuilder {
        render_handler: RenderHandler,
        life_span_handler: LifeSpanHandler,
        load_handler: LoadHandler,
        display_handler: DisplayHandler,
        request_handler: RequestHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load_handler.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display_handler.clone())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(self.request_handler.clone())
        }
    }
}

impl RendererClientBuilder {
    fn build(shared: Arc<Shared>) -> Client {
        Self::new(
            OsrHandler::new(shared.clone()),
            Lifespan::new(shared.clone()),
            Loads::new(shared.clone()),
            Console::new(shared.clone()),
            Requests::new(shared),
        )
    }
}
