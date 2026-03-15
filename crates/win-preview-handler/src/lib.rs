#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod preview_handler {
    use std::env;
    use std::ffi::c_void;
    use std::fs::{self, OpenOptions};
    use std::io::Write as IoWrite;
    use std::iter::once;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;

    use windows::core::{implement, GUID, HRESULT, IUnknown, PCWSTR, Result};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG,
        E_POINTER, HWND, RECT, S_FALSE, S_OK,
    };
    use windows_core::BOOL;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, IClassFactory, IClassFactory_Impl,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Ole::{
        IObjectWithSite, IObjectWithSite_Impl, IOleWindow, IOleWindow_Impl,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IInitializeWithFile, IInitializeWithFile_Impl,
    };
    use windows::Win32::UI::Shell::{
        IPreviewHandler, IPreviewHandler_Impl, IPreviewHandlerFrame,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, MoveWindow, MSG,
        PeekMessageW, TranslateMessage,
        WM_QUIT, WS_CHILD, WS_EX_NOPARENTNOTIFY, WS_VISIBLE, PM_REMOVE,
    };
    use windows_core::{Interface, Ref};

    use webview2_com::{
        CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            CreateCoreWebView2EnvironmentWithOptions,
            ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
        },
    };

    use base_styles::tokens_from_snapshot;
    use md_engine::MarkdownEngine;
    use win_theme_watcher::current_snapshot;

    static ACTIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);

    // -----------------------------------------------------------------------
    // RAII COM STA init guard for the preview background thread.
    //
    // CoInitializeEx returns S_OK (first init on thread) or S_FALSE (already
    // STA) — both require a matching CoUninitialize on drop.
    // RPC_E_CHANGED_MODE means the thread is already MTA; in that case we
    // must NOT call CoUninitialize.
    // -----------------------------------------------------------------------
    struct ComInit {
        should_uninit: bool,
    }

    impl ComInit {
        fn apartment_threaded() -> Self {
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let should_uninit = hr.ok().is_ok(); // true for S_OK and S_FALSE
            if !should_uninit {
                log(&format!("thread: CoInitializeEx failed: {hr:?}"));
            }
            ComInit { should_uninit }
        }
    }

    impl Drop for ComInit {
        fn drop(&mut self) {
            if self.should_uninit {
                unsafe { CoUninitialize(); }
            }
        }
    }

    pub const PREVIEW_HANDLER_CLSID: GUID =
        GUID::from_u128(0x4f831ca2_0db6_4f14_a4f2_8ab7de6f6601);

    // -----------------------------------------------------------------------
    // HWND wrapper safe to send across threads.
    // -----------------------------------------------------------------------
    struct SendHwnd(pub isize);
    unsafe impl Send for SendHwnd {}
    impl SendHwnd {
        fn hwnd(&self) -> HWND {
            HWND(self.0 as *mut c_void)
        }
    }
    impl From<HWND> for SendHwnd {
        fn from(h: HWND) -> Self {
            SendHwnd(h.0 as isize)
        }
    }

    // -----------------------------------------------------------------------
    // Commands sent from the COM STA thread to the preview window thread.
    // -----------------------------------------------------------------------
    enum PreviewCmd {
        /// Create (or recreate) the preview window.
        Show {
            parent: SendHwnd,
            bounds: RECT,
            /// Fully rendered HTML page ready for NavigateToString.
            html: String,
        },
        /// Resize the existing preview window.
        Resize(RECT),
        /// Destroy the window and exit the thread.
        Destroy,
    }

    // -----------------------------------------------------------------------
    // WebView2 async-creation state.
    //
    // WebView2 creation is two-phase (env → controller), both async.  The COM
    // callbacks fire on this thread during DispatchMessageW.  We store their
    // results in Mutex<Option<T>> arcs that are shared between the callbacks
    // and the poll loop below, all on the same thread.
    // -----------------------------------------------------------------------
    type EnvSlot  = Arc<Mutex<Option<Result<ICoreWebView2Environment>>>>;
    type CtrlSlot = Arc<Mutex<Option<Result<ICoreWebView2Controller>>>>;

    #[derive(PartialEq)]
    enum WvStage {
        WaitingEnv,
        WaitingCtrl,
        Ready,
        Failed,
    }

    struct WvCreation {
        env_slot:  EnvSlot,
        ctrl_slot: CtrlSlot,
        stage:     WvStage,
        html:      String,
        bounds:    RECT,
    }

    struct ActivePreview {
        container:  HWND,
        creation:   Option<WvCreation>,
        controller: Option<ICoreWebView2Controller>,
    }

    // -----------------------------------------------------------------------
    // Background thread that owns the child window and WebView2 controller.
    //
    // All window and WebView2 operations live here.  The COM STA thread sends
    // commands via the bounded channel and never touches HWNDs or COM objects
    // directly — this prevents the WM_SYNCPAINT cross-process deadlock.
    // -----------------------------------------------------------------------
    struct PreviewThread {
        tx:         mpsc::SyncSender<PreviewCmd>,
        child_hwnd: Arc<Mutex<isize>>,
        handle:     Option<thread::JoinHandle<()>>,
    }

    impl PreviewThread {
        fn new() -> Self {
            let (tx, rx) = mpsc::sync_channel::<PreviewCmd>(8);
            let child_hwnd = Arc::new(Mutex::new(0isize));
            let child_clone = child_hwnd.clone();
            let handle = thread::spawn(move || {
                preview_window_thread(rx, child_clone);
            });
            PreviewThread { tx, child_hwnd, handle: Some(handle) }
        }

        fn send(&self, cmd: PreviewCmd) {
            let _ = self.tx.try_send(cmd);
        }

        fn child(&self) -> HWND {
            let v = *self.child_hwnd.lock().unwrap();
            HWND(v as *mut c_void)
        }
    }

    impl Drop for PreviewThread {
        fn drop(&mut self) {
            let _ = self.tx.try_send(PreviewCmd::Destroy);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    fn preview_window_thread(
        rx: mpsc::Receiver<PreviewCmd>,
        child_hwnd: Arc<Mutex<isize>>,
    ) {
        let _com = ComInit::apartment_threaded();
        let mut active: Option<ActivePreview> = None;
        let mut msg = MSG::default();

        loop {
            // ---- 1. Process all pending commands (non-blocking). ----
            loop {
                match rx.try_recv() {
                    Ok(PreviewCmd::Show { parent, bounds, html }) => {
                        // Tear down whatever is currently showing.
                        if let Some(prev) = active.take() {
                            teardown_active(prev);
                        }
                        // Create container window and kick off async WebView2 creation.
                        active = start_preview(
                            parent.hwnd(), bounds, html, &child_hwnd,
                        );
                        if let Some(ref a) = active {
                            log(&format!("thread: child={:?}", a.container));
                        }
                    }
                    Ok(PreviewCmd::Resize(bounds)) => {
                        if let Some(ref mut a) = active {
                            resize_active(a, bounds);
                        }
                    }
                    Ok(PreviewCmd::Destroy) => {
                        if let Some(prev) = active.take() {
                            teardown_active(prev);
                        }
                        *child_hwnd.lock().unwrap() = 0;
                        return;
                    }
                    Err(_) => break,
                }
            }

            // ---- 2. Advance WebView2 creation if still in progress. ----
            if let Some(ref mut a) = active {
                poll_creation(a);
            }

            // ---- 3. Drain Win32 messages so WebView2 callbacks fire. ----
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        if let Some(prev) = active.take() {
                            teardown_active(prev);
                        }
                        return;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // ---- 4. Brief yield to avoid busy-loop. ----
            thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    // -----------------------------------------------------------------------
    // Create the container HWND and kick off async WebView2 env creation.
    // Returns None only if CreateWindowExW itself fails.
    // -----------------------------------------------------------------------
    fn start_preview(
        parent: HWND,
        bounds: RECT,
        html: String,
        child_hwnd: &Arc<Mutex<isize>>,
    ) -> Option<ActivePreview> {
        if parent.is_invalid() {
            return None;
        }
        let w = (bounds.right  - bounds.left).max(1);
        let h = (bounds.bottom - bounds.top ).max(1);

        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let empty: Vec<u16> = "\0".encode_utf16().collect();

        let container = unsafe {
            match CreateWindowExW(
                WS_EX_NOPARENTNOTIFY,
                PCWSTR(class.as_ptr()),
                PCWSTR(empty.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                0, 0, w, h,
                Some(parent),
                None, None, None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    log(&format!("CreateWindowExW failed: {e}"));
                    return None;
                }
            }
        };

        *child_hwnd.lock().unwrap() = container.0 as isize;

        // Kick off async WebView2 environment creation.
        let env_slot: EnvSlot = Arc::new(Mutex::new(None));
        let env_slot_cb = Arc::clone(&env_slot);

        let udf = webview2_user_data_folder();
        let udf_w: Vec<u16> = udf.encode_utf16().chain(once(0)).collect();

        let env_result = unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                PCWSTR(udf_w.as_ptr()),
                None,
                &CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
                    move |hr: Result<()>, env: Option<ICoreWebView2Environment>| {
                        *env_slot_cb.lock().unwrap() = Some(
                            hr.and_then(|_| env.ok_or_else(|| windows_core::Error::from(E_FAIL)))
                        );
                        Ok(())
                    },
                )),
            )
        };
        if let Err(e) = env_result {
            log(&format!("CreateCoreWebView2EnvironmentWithOptions failed: {e}"));
            unsafe { let _ = DestroyWindow(container); }
            *child_hwnd.lock().unwrap() = 0;
            return None;
        }

        Some(ActivePreview {
            container,
            creation: Some(WvCreation {
                env_slot,
                ctrl_slot: Arc::new(Mutex::new(None)),
                stage: WvStage::WaitingEnv,
                html,
                bounds,
            }),
            controller: None,
        })
    }

    // -----------------------------------------------------------------------
    // Advance WebView2 creation state machine one step.
    // Called each loop iteration after message pumping so callbacks have fired.
    // -----------------------------------------------------------------------
    fn poll_creation(active: &mut ActivePreview) {
        let creation = match active.creation.as_mut() {
            Some(c) if c.stage != WvStage::Ready && c.stage != WvStage::Failed => c,
            _ => return,
        };

        match creation.stage {
            WvStage::WaitingEnv => {
                let env_res = creation.env_slot.lock().unwrap().take();
                match env_res {
                    Some(Ok(env)) => {
                        // Got environment — start controller creation.
                        let ctrl_slot_cb = Arc::clone(&creation.ctrl_slot);
                        let container = active.container;
                        let result = unsafe {
                            env.CreateCoreWebView2Controller(
                                container,
                                &CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                                    move |hr: Result<()>, ctrl: Option<ICoreWebView2Controller>| {
                                        *ctrl_slot_cb.lock().unwrap() = Some(
                                            hr.and_then(|_| ctrl.ok_or_else(|| windows_core::Error::from(E_FAIL)))
                                        );
                                        Ok(())
                                    },
                                )),
                            )
                        };
                        if let Err(e) = result {
                            log(&format!("CreateCoreWebView2Controller failed: {e}"));
                            creation.stage = WvStage::Failed;
                        } else {
                            creation.stage = WvStage::WaitingCtrl;
                        }
                    }
                    Some(Err(e)) => {
                        log(&format!("WebView2 environment creation failed: {e}"));
                        creation.stage = WvStage::Failed;
                    }
                    None => {}
                }
            }
            WvStage::WaitingCtrl => {
                let ctrl_res = creation.ctrl_slot.lock().unwrap().take();
                match ctrl_res {
                    Some(Ok(ctrl)) => {
                        // Got controller — set bounds, show, navigate.
                        let bounds = creation.bounds;
                        let w = (bounds.right  - bounds.left).max(1);
                        let h = (bounds.bottom - bounds.top ).max(1);
                        let wv_rect = RECT { left: 0, top: 0, right: w, bottom: h };

                        let ok = unsafe {
                            ctrl.SetBounds(wv_rect)
                                .and_then(|_| ctrl.SetIsVisible(true))
                                .and_then(|_| {
                                    let wv: ICoreWebView2 = ctrl.CoreWebView2()?;
                                    let html_w: Vec<u16> = creation.html
                                        .encode_utf16()
                                        .chain(once(0))
                                        .collect();
                                    wv.NavigateToString(PCWSTR(html_w.as_ptr()))
                                })
                        };
                        if let Err(e) = ok {
                            log(&format!("WebView2 setup failed: {e}"));
                        }
                        active.controller = Some(ctrl);
                        creation.stage = WvStage::Ready;
                        // Drop the creation arcs now that we're done.
                        active.creation = None;
                    }
                    Some(Err(e)) => {
                        log(&format!("WebView2 controller creation failed: {e}"));
                        creation.stage = WvStage::Failed;
                    }
                    None => {}
                }
            }
            WvStage::Ready | WvStage::Failed => {}
        }
    }

    // -----------------------------------------------------------------------
    // Resize both the container window and the WebView2 controller.
    // -----------------------------------------------------------------------
    fn resize_active(active: &mut ActivePreview, bounds: RECT) {
        if active.container.is_invalid() {
            return;
        }
        let w = (bounds.right  - bounds.left).max(1);
        let h = (bounds.bottom - bounds.top ).max(1);
        unsafe {
            let _ = MoveWindow(active.container, 0, 0, w, h, true);
        }
        // Update pending bounds so the controller gets the right size when it arrives.
        if let Some(ref mut c) = active.creation {
            c.bounds = bounds;
        }
        // Resize an already-live controller.
        if let Some(ref ctrl) = active.controller {
            let wv_rect = RECT { left: 0, top: 0, right: w, bottom: h };
            unsafe { let _ = ctrl.SetBounds(wv_rect); }
        }
    }

    // -----------------------------------------------------------------------
    // Close WebView2 controller and destroy the container window.
    // -----------------------------------------------------------------------
    fn teardown_active(active: ActivePreview) {
        if let Some(ctrl) = active.controller {
            unsafe { let _ = ctrl.Close(); }
        }
        if !active.container.is_invalid() {
            unsafe { let _ = DestroyWindow(active.container); }
        }
    }

    // -----------------------------------------------------------------------
    // User data folder accessible from Low-Integrity (prevhost.exe sandbox).
    // Mirrors the log-file path: %USERPROFILE%\AppData\Local\Temp\Low\
    // -----------------------------------------------------------------------
    fn webview2_user_data_folder() -> String {
        if let Ok(up) = env::var("USERPROFILE") {
            let p = std::path::PathBuf::from(up)
                .join("AppData")
                .join("Local")
                .join("Temp")
                .join("Low")
                .join("mdview-webview2");
            let _ = fs::create_dir_all(&p);
            if let Some(s) = p.to_str() {
                return s.to_owned();
            }
        }
        let p = env::temp_dir().join("mdview-webview2");
        let _ = fs::create_dir_all(&p);
        p.to_str().unwrap_or("mdview-webview2").to_owned()
    }

    // -----------------------------------------------------------------------
    // Content rendering
    // -----------------------------------------------------------------------
    const PREVIEW_BYTE_LIMIT: usize = 512 * 1024;

    fn render_preview_html(path: &str) -> String {
        if path.is_empty() {
            return error_page("No file path provided.");
        }
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => return error_page(&format!("Cannot read file: {}", e)),
        };
        // Truncate large files at a char boundary and append a notice in
        // Markdown so the rendered output looks intentional, not broken.
        let source = if source.len() > PREVIEW_BYTE_LIMIT {
            let mut end = PREVIEW_BYTE_LIMIT;
            while !source.is_char_boundary(end) {
                end -= 1;
            }
            let mut s = source[..end].to_string();
            s.push_str(
                "\n\n---\n\n*\\[mdview: file exceeds 512 KB — \
                 preview truncated. Open in editor for full content.\\]*\n",
            );
            s
        } else {
            source
        };

        let doc = MarkdownEngine::default().render(&source);
        let snap = current_snapshot();
        let css_vars = tokens_from_snapshot(&snap).to_css_vars();
        build_html_page(&doc.html, &css_vars)
    }

    fn error_page(msg: &str) -> String {
        let escaped = msg
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        build_html_page(
            &format!("<p style=\"color:var(--mdv-muted-text,#b6b6b6);font-style:italic\">{escaped}</p>"),
            "",
        )
    }

    fn build_html_page(body_html: &str, css_vars: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="color-scheme" content="dark light">
<style>
{css_vars}
{PREVIEW_CSS}
</style>
</head>
<body><article class="md-body">
{body_html}
</article></body>
</html>"#,
            css_vars = css_vars,
            body_html = body_html,
        )
    }

    // Minimal but complete CSS for the preview pane.
    // Uses --mdv-* variables emitted by base_styles::ThemeTokens::to_css_vars().
    const PREVIEW_CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html {
  background: var(--mdv-bg, #1e1e1e);
  color:      var(--mdv-text, #f3f3f3);
  font-family: "Segoe UI", system-ui, sans-serif;
  font-size: 14px;
  line-height: 1.65;
}

body {
  margin: 0;
  padding: 16px 20px 32px;
}

.md-body {
  max-width: 860px;
}

h1, h2, h3, h4, h5, h6 {
  margin-top: 1.4em;
  margin-bottom: 0.5em;
  line-height: 1.3;
}
h1 { font-size: 1.9em; border-bottom: 1px solid var(--mdv-border, #3c3c3c); padding-bottom: 0.25em; }
h2 { font-size: 1.45em; border-bottom: 1px solid var(--mdv-border, #3c3c3c); padding-bottom: 0.2em; }
h3 { font-size: 1.2em; }
h4, h5, h6 { font-size: 1em; }

p { margin: 0.75em 0; }

a { color: var(--mdv-accent, #0a84ff); text-decoration: none; }
a:hover { text-decoration: underline; }

code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 0.88em;
  background: var(--mdv-code-bg, #2d2d2d);
  padding: 0.15em 0.4em;
  border-radius: 4px;
}

pre {
  background: var(--mdv-code-bg, #2d2d2d);
  border: 1px solid var(--mdv-border, #3c3c3c);
  border-radius: 6px;
  padding: 12px 16px;
  overflow-x: auto;
  margin: 1em 0;
}
pre code {
  background: none;
  padding: 0;
  font-size: 0.87em;
}

blockquote {
  margin: 0.75em 0;
  padding: 0.5em 0 0.5em 1em;
  border-left: 3px solid var(--mdv-accent, #0a84ff);
  color: var(--mdv-muted-text, #b6b6b6);
}

table {
  border-collapse: collapse;
  width: 100%;
  margin: 1em 0;
  font-size: 0.93em;
}
th, td {
  border: 1px solid var(--mdv-border, #3c3c3c);
  padding: 6px 12px;
  text-align: left;
}
th { background: var(--mdv-surface, #252526); }
tr:nth-child(even) { background: var(--mdv-surface, #252526); }

img { max-width: 100%; height: auto; }

hr {
  border: none;
  border-top: 1px solid var(--mdv-border, #3c3c3c);
  margin: 1.5em 0;
}

ul, ol { padding-left: 1.5em; margin: 0.5em 0; }
li { margin: 0.25em 0; }

/* Task-list checkboxes (comrak) */
input[type="checkbox"] {
  margin-right: 0.4em;
  accent-color: var(--mdv-accent, #0a84ff);
}

del { color: var(--mdv-muted-text, #b6b6b6); }
"#;

    // -----------------------------------------------------------------------
    // Handler state (on the COM STA thread)
    // -----------------------------------------------------------------------
    #[derive(Default)]
    struct State {
        file_path:       Option<String>,
        parent_hwnd:     HWND,
        bounds:          RECT,
        site:            Option<IUnknown>,
        frame:           Option<IPreviewHandlerFrame>,
        /// DoPreview was called; waiting for SetRect to supply real bounds.
        preview_pending: bool,
    }

    #[implement(IInitializeWithFile, IObjectWithSite, IOleWindow, IPreviewHandler)]
    pub struct MarkdownPreviewHandler {
        state:   Mutex<State>,
        /// Background window thread; None until first DoPreview+SetRect.
        preview: Mutex<Option<PreviewThread>>,
    }

    impl MarkdownPreviewHandler {
        pub fn new() -> Self {
            ACTIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
            log("new");
            Self {
                state:   Mutex::new(State::default()),
                preview: Mutex::new(None),
            }
        }
    }

    impl Drop for MarkdownPreviewHandler {
        fn drop(&mut self) {
            ACTIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
            log("drop");
        }
    }

    #[allow(non_snake_case)]
    impl IInitializeWithFile_Impl for MarkdownPreviewHandler_Impl {
        fn Initialize(&self, pszfilepath: &PCWSTR, _grfmode: u32) -> Result<()> {
            let path = unsafe { pszfilepath.to_string()? };
            log(&format!("Initialize path={path}"));
            self.state.lock().map_err(|_| E_FAIL)?.file_path = Some(path);
            Ok(())
        }
    }

    #[allow(non_snake_case)]
    impl IObjectWithSite_Impl for MarkdownPreviewHandler_Impl {
        fn SetSite(&self, punksite: Ref<'_, IUnknown>) -> Result<()> {
            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            let site = punksite.cloned();
            log(&format!("SetSite has_site={}", site.is_some()));
            state.frame = site
                .as_ref()
                .and_then(|s| s.cast::<IPreviewHandlerFrame>().ok());
            state.site = site;
            Ok(())
        }

        fn GetSite(&self, riid: *const GUID, ppvsite: *mut *mut c_void) -> Result<()> {
            if riid.is_null() || ppvsite.is_null() {
                return Err(E_POINTER.into());
            }
            let state = self.state.lock().map_err(|_| E_FAIL)?;
            let site = state.site.as_ref().ok_or(E_FAIL)?;
            unsafe { site.query(riid, ppvsite).ok() }
        }
    }

    #[allow(non_snake_case)]
    impl IOleWindow_Impl for MarkdownPreviewHandler_Impl {
        fn GetWindow(&self) -> Result<HWND> {
            if let Ok(p) = self.preview.lock() {
                if let Some(pt) = p.as_ref() {
                    let c = pt.child();
                    if !c.is_invalid() {
                        return Ok(c);
                    }
                }
            }
            let state = self.state.lock().map_err(|_| E_FAIL)?;
            if state.parent_hwnd.is_invalid() {
                return Err(E_FAIL.into());
            }
            Ok(state.parent_hwnd)
        }

        fn ContextSensitiveHelp(&self, _fentermode: BOOL) -> Result<()> {
            Ok(())
        }
    }

    #[allow(non_snake_case)]
    impl IPreviewHandler_Impl for MarkdownPreviewHandler_Impl {
        fn SetWindow(&self, hwnd: HWND, prect: *const RECT) -> Result<()> {
            if prect.is_null() {
                return Err(E_POINTER.into());
            }
            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            state.parent_hwnd = hwnd;
            state.bounds = unsafe { *prect };
            log(&format!(
                "SetWindow hwnd={:?} rect=({},{},{},{})",
                hwnd,
                state.bounds.left, state.bounds.top,
                state.bounds.right, state.bounds.bottom
            ));
            Ok(())
        }

        fn SetRect(&self, prect: *const RECT) -> Result<()> {
            if prect.is_null() {
                return Err(E_POINTER.into());
            }
            let rect = unsafe { *prect };

            let (do_show, parent, path) = {
                let mut state = self.state.lock().map_err(|_| E_FAIL)?;
                state.bounds = rect;
                let has_area = rect.right > rect.left && rect.bottom > rect.top;
                let do_show = state.preview_pending && has_area;
                if do_show {
                    state.preview_pending = false;
                }
                (do_show, state.parent_hwnd, state.file_path.clone())
            };

            log(&format!(
                "SetRect ({},{},{},{}) do_show={do_show}",
                rect.left, rect.top, rect.right, rect.bottom
            ));

            if do_show {
                let html = render_preview_html(&path.unwrap_or_default());
                let mut preview = self.preview.lock().map_err(|_| E_FAIL)?;
                if preview.is_none() {
                    *preview = Some(PreviewThread::new());
                }
                if let Some(pt) = preview.as_ref() {
                    pt.send(PreviewCmd::Show {
                        parent: parent.into(),
                        bounds: rect,
                        html,
                    });
                }
            } else if let Ok(preview) = self.preview.lock() {
                if let Some(pt) = preview.as_ref() {
                    pt.send(PreviewCmd::Resize(rect));
                }
            }

            Ok(())
        }

        fn DoPreview(&self) -> Result<()> {
            let (do_show, parent, path, bounds) = {
                let mut state = self.state.lock().map_err(|_| E_FAIL)?;
                let has_area = state.bounds.right > state.bounds.left
                    && state.bounds.bottom > state.bounds.top;
                if has_area {
                    state.preview_pending = false;
                    (true, state.parent_hwnd, state.file_path.clone(), state.bounds)
                } else {
                    state.preview_pending = true;
                    (false, state.parent_hwnd, state.file_path.clone(), state.bounds)
                }
            };

            log(&format!("DoPreview — do_show={do_show}"));

            if do_show {
                let html = render_preview_html(&path.unwrap_or_default());
                let mut preview = self.preview.lock().map_err(|_| E_FAIL)?;
                if preview.is_none() {
                    *preview = Some(PreviewThread::new());
                }
                if let Some(pt) = preview.as_ref() {
                    pt.send(PreviewCmd::Show {
                        parent: parent.into(),
                        bounds,
                        html,
                    });
                }
            }

            Ok(())
        }

        fn Unload(&self) -> Result<()> {
            log("Unload");
            {
                let mut state = self.state.lock().map_err(|_| E_FAIL)?;
                state.file_path       = None;
                state.frame           = None;
                state.site            = None;
                state.preview_pending = false;
            }
            if let Ok(mut preview) = self.preview.lock() {
                preview.take(); // Drop triggers PreviewThread::drop → Destroy + join
            }
            Ok(())
        }

        fn SetFocus(&self) -> Result<()> {
            Ok(())
        }

        fn QueryFocus(&self) -> Result<HWND> {
            Ok(self.state.lock().map_err(|_| E_FAIL)?.parent_hwnd)
        }

        fn TranslateAccelerator(&self, _pmsg: *const MSG) -> Result<()> {
            let state = self.state.lock().map_err(|_| E_FAIL)?;
            if let Some(frame) = state.frame.as_ref() {
                unsafe {
                    return frame.TranslateAccelerator(_pmsg);
                }
            }
            Err(windows::core::Error::from(S_FALSE))
        }
    }

    // -----------------------------------------------------------------------
    // Logging — tries Low-IL path first, then normal temp.
    // -----------------------------------------------------------------------
    fn log(msg: &str) {
        let candidates: Vec<std::path::PathBuf> = {
            let mut v = Vec::new();
            if let Ok(up) = env::var("USERPROFILE") {
                let base = std::path::PathBuf::from(&up)
                    .join("AppData").join("Local").join("Temp");
                v.push(base.join("Low").join("mdview-preview.log"));
                v.push(base.join("mdview-preview.log"));
            }
            v.push(env::temp_dir().join("mdview-preview.log"));
            v
        };
        for path in &candidates {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{msg}");
                return;
            }
        }
    }

    // -----------------------------------------------------------------------
    // COM boilerplate
    // -----------------------------------------------------------------------
    #[implement(IClassFactory)]
    struct PreviewHandlerFactory;

    #[allow(non_snake_case)]
    impl IClassFactory_Impl for PreviewHandlerFactory_Impl {
        fn CreateInstance(
            &self,
            punkouter: Ref<'_, IUnknown>,
            riid: *const GUID,
            ppvobject: *mut *mut c_void,
        ) -> Result<()> {
            if riid.is_null() || ppvobject.is_null() {
                return Err(E_POINTER.into());
            }
            if !punkouter.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let unknown: IUnknown = MarkdownPreviewHandler::new().into();
            unsafe { unknown.query(riid, ppvobject).ok() }
        }

        fn LockServer(&self, flock: BOOL) -> Result<()> {
            if flock.as_bool() {
                ACTIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
            } else {
                ACTIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    #[no_mangle]
    pub extern "system" fn DllCanUnloadNow() -> HRESULT {
        log("DllCanUnloadNow");
        if ACTIVE_OBJECTS.load(Ordering::SeqCst) == 0 { S_OK } else { S_FALSE }
    }

    #[no_mangle]
    pub extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        log("DllGetClassObject");
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_INVALIDARG;
        }
        unsafe {
            if *rclsid != PREVIEW_HANDLER_CLSID {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
        }
        let factory: IClassFactory = PreviewHandlerFactory.into();
        unsafe { factory.query(riid, ppv) }
    }
}

#[cfg(not(windows))]
mod preview_handler {
    pub const PREVIEW_HANDLER_PROGID: &str = "mdview.PreviewHandler";
}
