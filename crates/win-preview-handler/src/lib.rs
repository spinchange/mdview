#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod preview_handler {
    use std::env;
    use std::ffi::c_void;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{mpsc, Arc, Mutex, OnceLock};
    use std::thread;

    use windows::core::{implement, GUID, IUnknown, PCWSTR, Result, HRESULT};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG,
        E_POINTER, HWND, LPARAM, RECT, S_FALSE, S_OK, WPARAM,
    };
    
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
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
        PeekMessageW, SendMessageW, SetWindowTextW, ShowWindow, SW_SHOW,
        TranslateMessage, WM_QUIT, WM_SETFONT, WS_CHILD, WS_EX_NOPARENTNOTIFY,
        WS_VSCROLL, ES_MULTILINE, ES_READONLY, PM_REMOVE,
    };
    use windows::Win32::UI::Controls::{EM_SETLIMITTEXT, EM_SETMARGINS};
    use windows_core::{BOOL, Interface, Ref};

    static ACTIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);

    pub const PREVIEW_HANDLER_CLSID: GUID =
        GUID::from_u128(0x4f831ca2_0db6_4f14_a4f2_8ab7de6f6601);
    // -----------------------------------------------------------------------
    // HWND wrapper that is safe to send across threads.
    // We enforce correct thread usage by design: the preview thread owns the
    // child window; the COM thread only sends commands via the channel.
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
    // Commands sent from the COM (STA) thread to the preview window thread.
    // -----------------------------------------------------------------------
    enum PreviewCmd {
        /// Create (or recreate) the preview window.
        Show {
            parent: SendHwnd,
            bounds: RECT,
            content: String,
        },
        /// Resize the existing preview window.
        Resize(RECT),
        /// Destroy the window and exit the thread.
        Destroy,
    }

    // -----------------------------------------------------------------------
    // Background thread that owns the child window.
    //
    // The COM STA thread must never create a visible child window of a
    // cross-process parent while Explorer's UI thread is blocked on a COM
    // call — that causes a WM_SYNCPAINT deadlock.  By moving all window
    // operations to this thread, we ensure they happen only after all COM
    // calls have returned and Explorer's thread is free.
    // -----------------------------------------------------------------------
    struct PreviewThread {
        tx: mpsc::SyncSender<PreviewCmd>,
        /// Shared handle so IOleWindow::GetWindow can return it.
        child_hwnd: Arc<Mutex<isize>>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl PreviewThread {
        fn new() -> Self {
            let (tx, rx) = mpsc::sync_channel::<PreviewCmd>(8);
            let child_hwnd = Arc::new(Mutex::new(0isize));
            let child_clone = child_hwnd.clone();
            let handle = thread::spawn(move || {
                preview_window_thread(rx, child_clone);
            });
            PreviewThread {
                tx,
                child_hwnd,
                handle: Some(handle),
            }
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
        let mut local_child = HWND::default();
        let mut msg = MSG::default();

        loop {
            // Process all pending commands (non-blocking).
            loop {
                match rx.try_recv() {
                    Ok(PreviewCmd::Show { parent, bounds, content }) => {
                        if !local_child.is_invalid() {
                            unsafe { let _ = DestroyWindow(local_child); }
                        }
                        local_child = create_child(parent.hwnd(), bounds, &content);
                        *child_hwnd.lock().unwrap() = local_child.0 as isize;
                        log(&format!("thread: child={:?}", local_child));
                    }
                    Ok(PreviewCmd::Resize(bounds)) => {
                        if !local_child.is_invalid() {
                            resize(local_child, bounds);
                        }
                    }
                    Ok(PreviewCmd::Destroy) => {
                        if !local_child.is_invalid() {
                            unsafe { let _ = DestroyWindow(local_child); }
                        }
                        *child_hwnd.lock().unwrap() = 0;
                        return;
                    }
                    Err(_) => break,
                }
            }

            // Drain Win32 messages so the child window paints correctly.
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        return;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Sleep briefly to avoid a busy-loop.
            thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    // -----------------------------------------------------------------------
    // Font — created once per DLL load, never deleted (stock-like lifetime).
    // Consolas 10pt ClearType: readable monospace that shows MD syntax well.
    // -----------------------------------------------------------------------
    static PREVIEW_FONT: OnceLock<isize> = OnceLock::new();

    fn preview_font() -> isize {
        *PREVIEW_FONT.get_or_init(|| unsafe {
            use windows::Win32::Graphics::Gdi::{
                CreateFontW, FONT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY,
            };
            let face: Vec<u16> = "Consolas\0".encode_utf16().collect();
            let hf = CreateFontW(
                -13,   // height ≈ 10pt at 96 dpi (negative = char height)
                0,     // width — let GDI choose aspect ratio
                0, 0,  // escapement, orientation
                400,   // FW_NORMAL
                0, 0, 0, // italic, underline, strikeout
                FONT_CHARSET(0),             // ANSI_CHARSET
                FONT_OUTPUT_PRECISION(0),    // OUT_DEFAULT_PRECIS
                FONT_CLIP_PRECISION(0),      // CLIP_DEFAULT_PRECIS
                FONT_QUALITY(5),             // CLEARTYPE_QUALITY
                0x31,  // FIXED_PITCH | FF_MODERN
                PCWSTR(face.as_ptr()),
            );
            hf.0 as isize
        })
    }

    // Padding inset (pixels) applied on every edge so text never touches the
    // pane border. EM_SETMARGINS adds an extra left/right gutter inside the
    // control on top of this.
    const EDGE_PAD: i32 = 8;
    // Inner left/right margin sent via EM_SETMARGINS (pixels).
    const INNER_MARGIN: u16 = 8;
    // Maximum UTF-8 bytes we pass to the EDIT control.
    const PREVIEW_BYTE_LIMIT: usize = 512 * 1024;

    fn create_child(parent: HWND, bounds: RECT, text: &str) -> HWND {
        if parent.is_invalid() {
            return HWND::default();
        }

        // Inset the child rect so content has breathing room on all sides.
        let x = EDGE_PAD;
        let y = EDGE_PAD;
        let w = ((bounds.right - bounds.left) - EDGE_PAD * 2).max(1);
        let h = ((bounds.bottom - bounds.top) - EDGE_PAD * 2).max(1);

        let class: Vec<u16> = "EDIT\0".encode_utf16().collect();
        let empty: Vec<u16> = "\0".encode_utf16().collect();
        unsafe {
            match CreateWindowExW(
                WS_EX_NOPARENTNOTIFY,
                PCWSTR(class.as_ptr()),
                PCWSTR(empty.as_ptr()),
                // ES_MULTILINE without ES_AUTOHSCROLL = word-wrap at window width.
                // ES_AUTOVSCROLL omitted: it auto-scrolls to end on text insert
                // which is wrong for a top-of-file preview.
                WS_CHILD | WS_VSCROLL
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        (ES_MULTILINE | ES_READONLY) as u32,
                    ),
                x, y, w, h,
                Some(parent),
                None, None, None,
            ) {
                Ok(hwnd) => {
                    // 1. Raise text limit before setting content (default ~32 KB).
                    //    PREVIEW_BYTE_LIMIT bytes UTF-8 ≤ PREVIEW_BYTE_LIMIT UTF-16
                    //    code units, so using the byte count as the TCHAR limit is
                    //    safe (it may be slightly generous, never too tight).
                    let _ = SendMessageW(
                        hwnd,
                        EM_SETLIMITTEXT,
                        Some(WPARAM(PREVIEW_BYTE_LIMIT)),
                        Some(LPARAM(0)),
                    );

                    // 2. Apply font before first paint.
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETFONT,
                        Some(WPARAM(preview_font() as usize)),
                        Some(LPARAM(1)), // redraw = TRUE
                    );

                    // 3. Left/right inner margin (EM_SETMARGINS).
                    //    WPARAM: EC_LEFTMARGIN(1) | EC_RIGHTMARGIN(2) = 3
                    //    LPARAM: low-word = left, high-word = right (pixels)
                    let margin_param = (INNER_MARGIN as u32)
                        | ((INNER_MARGIN as u32) << 16);
                    let _ = SendMessageW(
                        hwnd,
                        EM_SETMARGINS,
                        Some(WPARAM(3)), // EC_LEFTMARGIN | EC_RIGHTMARGIN
                        Some(LPARAM(margin_param as isize)),
                    );

                    // 4. Set content.
                    let txt: Vec<u16> =
                        text.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = SetWindowTextW(hwnd, PCWSTR(txt.as_ptr()));

                    let _ = ShowWindow(hwnd, SW_SHOW);
                    hwnd
                }
                Err(e) => {
                    log(&format!("CreateWindowExW failed: {e}"));
                    HWND::default()
                }
            }
        }
    }

    fn resize(hwnd: HWND, bounds: RECT) {
        if hwnd.is_invalid() {
            return;
        }
        // Keep the same EDGE_PAD inset as create_child so the margin is stable
        // as the user resizes the preview pane.
        let w = ((bounds.right - bounds.left) - EDGE_PAD * 2).max(1);
        let h = ((bounds.bottom - bounds.top) - EDGE_PAD * 2).max(1);
        unsafe {
            let _ = MoveWindow(hwnd, EDGE_PAD, EDGE_PAD, w, h, true);
        }
    }

    // -----------------------------------------------------------------------
    // Handler state (on the COM STA thread)
    // -----------------------------------------------------------------------
    #[derive(Default)]
    struct State {
        file_path: Option<String>,
        parent_hwnd: HWND,
        bounds: RECT,
        site: Option<IUnknown>,
        frame: Option<IPreviewHandlerFrame>,
        /// DoPreview was called; waiting for SetRect to supply real bounds.
        preview_pending: bool,
    }

    #[implement(IInitializeWithFile, IObjectWithSite, IOleWindow, IPreviewHandler)]
    pub struct MarkdownPreviewHandler {
        state: Mutex<State>,
        /// Lives on the background thread; None until first DoPreview+SetRect.
        preview: Mutex<Option<PreviewThread>>,
    }

    impl MarkdownPreviewHandler {
        pub fn new() -> Self {
            ACTIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
            log("new");
            Self {
                state: Mutex::new(State::default()),
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
            // Return our child window if created, otherwise the host window.
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
                hwnd, state.bounds.left, state.bounds.top,
                state.bounds.right, state.bounds.bottom
            ));
            Ok(())
        }

        fn SetRect(&self, prect: *const RECT) -> Result<()> {
            if prect.is_null() {
                return Err(E_POINTER.into());
            }
            let rect = unsafe { *prect };

            // Collect everything we need under the state lock, then release it
            // before touching the preview lock (consistent lock ordering).
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
                // File read happens outside both locks.
                let content = read_file(&path.unwrap_or_default());
                let mut preview = self.preview.lock().map_err(|_| E_FAIL)?;
                if preview.is_none() {
                    *preview = Some(PreviewThread::new());
                }
                if let Some(pt) = preview.as_ref() {
                    pt.send(PreviewCmd::Show {
                        parent: parent.into(),
                        bounds: rect,
                        content,
                    });
                }
            } else {
                if let Ok(preview) = self.preview.lock() {
                    if let Some(pt) = preview.as_ref() {
                        pt.send(PreviewCmd::Resize(rect));
                    }
                }
            }

            Ok(())
        }

        fn DoPreview(&self) -> Result<()> {
            // If we already have bounds (e.g. switching files while pane is
            // open), trigger the show now — SetRect won't be called again.
            // Otherwise defer to SetRect, which arrives with the real bounds.
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
                let content = read_file(&path.unwrap_or_default());
                let mut preview = self.preview.lock().map_err(|_| E_FAIL)?;
                if preview.is_none() {
                    *preview = Some(PreviewThread::new());
                }
                if let Some(pt) = preview.as_ref() {
                    pt.send(PreviewCmd::Show {
                        parent: parent.into(),
                        bounds,
                        content,
                    });
                }
            }

            Ok(())
        }

        fn Unload(&self) -> Result<()> {
            log("Unload");
            {
                let mut state = self.state.lock().map_err(|_| E_FAIL)?;
                state.file_path = None;
                state.frame = None;
                state.site = None;
                state.preview_pending = false;
            }
            // Drop the preview thread (sends Destroy + joins).
            if let Ok(mut preview) = self.preview.lock() {
                preview.take(); // Drop triggers PreviewThread::drop
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

    fn read_file(path: &str) -> String {
        if path.is_empty() {
            return String::from("[mdview] No file.");
        }
        match fs::read_to_string(path) {
            Ok(text) => {
                if text.len() > PREVIEW_BYTE_LIMIT {
                    // Truncate at a char boundary so UTF-8 slicing is safe.
                    let mut end = PREVIEW_BYTE_LIMIT;
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!(
                        "{}\n\n\
                        ── [mdview: file exceeds {} KB; preview truncated. \
                        Open in editor for full content.] ──",
                        &text[..end],
                        PREVIEW_BYTE_LIMIT / 1024,
                    )
                } else {
                    text
                }
            }
            Err(e) => format!("[mdview] Cannot read {path}: {e}"),
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
