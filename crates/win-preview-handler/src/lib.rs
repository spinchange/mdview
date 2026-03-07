#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod preview_handler {
    use std::fs;
    use std::ffi::c_void;
    use std::path::Path;
    use std::sync::mpsc;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;

    use base_styles::tokens_from_snapshot;
    use md_engine::MarkdownEngine;
    use webview2_com::{
        CoTaskMemPWSTR, CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    };
    use win_theme_watcher::current_snapshot;
    use windows::core::{implement, GUID, IUnknown, PCWSTR, Result, HRESULT};
    use windows_core::{BOOL, Error, Interface, Ref};
    use windows::Win32::Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, COLORREF, E_FAIL, E_INVALIDARG,
        E_POINTER, HWND, RECT, S_FALSE, S_OK,
    };
    use windows::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, FillRect, GetDC, HBRUSH, InvalidateRect, ReleaseDC,
    };
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IInitializeWithFile, IInitializeWithFile_Impl,
    };
    use windows::Win32::UI::Shell::{IPreviewHandler, IPreviewHandler_Impl};
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, MSG};

    static ACTIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);

    pub const PREVIEW_HANDLER_CLSID: GUID =
        GUID::from_u128(0x4f831ca2_0db6_4f14_a4f2_8ab7de6f6601);
    pub const PREVIEW_HANDLER_PROGID: &str = "mdview.PreviewHandler";

    #[derive(Default)]
    struct State {
        file_path: Option<String>,
        parent_hwnd: HWND,
        bounds: RECT,
        environment: Option<ICoreWebView2Environment>,
        controller: Option<ICoreWebView2Controller>,
        webview: Option<ICoreWebView2>,
    }

    #[implement(IInitializeWithFile, IPreviewHandler)]
    pub struct MarkdownPreviewHandler {
        state: Mutex<State>,
    }

    impl MarkdownPreviewHandler {
        pub fn new() -> Self {
            ACTIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
            Self {
                state: Mutex::new(State::default()),
            }
        }
    }

    impl Drop for MarkdownPreviewHandler {
        fn drop(&mut self) {
            ACTIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[allow(non_snake_case)]
    impl IInitializeWithFile_Impl for MarkdownPreviewHandler_Impl {
        fn Initialize(&self, pszfilepath: &PCWSTR, _grfmode: u32) -> Result<()> {
            let path = unsafe { pszfilepath.to_string()? };
            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            state.file_path = Some(path);
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
            apply_bounds(&state.controller, state.bounds)?;
            Ok(())
        }

        fn SetRect(&self, prect: *const RECT) -> Result<()> {
            if prect.is_null() {
                return Err(E_POINTER.into());
            }

            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            state.bounds = unsafe { *prect };
            apply_bounds(&state.controller, state.bounds)?;
            Ok(())
        }

        fn DoPreview(&self) -> Result<()> {
            let (path, parent_hwnd, bounds, maybe_webview) = {
                let state = self.state.lock().map_err(|_| E_FAIL)?;
                let path = state.file_path.clone().ok_or(E_FAIL)?;
                (path, state.parent_hwnd, state.bounds, state.webview.clone())
            };

            if parent_hwnd.is_invalid() {
                return Err(E_FAIL.into());
            }

            paint_parent_background(parent_hwnd);
            let rendered = render_preview_page(&path);

            if let Some(webview) = maybe_webview {
                if let Err(err) = apply_virtual_host_mapping(&webview, &path) {
                    eprintln!("[mdview-preview] virtual host mapping failed: {err}");
                }
                navigate_to_markup(&webview, &rendered)?;
                return Ok(());
            }

            let environment = create_webview_environment()?;
            let controller = create_webview_controller(&environment, parent_hwnd)?;
            apply_bounds(&Some(controller.clone()), bounds)?;
            let webview = unsafe { controller.CoreWebView2()? };
            if let Err(err) = apply_virtual_host_mapping(&webview, &path) {
                eprintln!("[mdview-preview] virtual host mapping failed: {err}");
            }
            navigate_to_markup(&webview, &rendered)?;

            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            state.environment = Some(environment);
            state.controller = Some(controller);
            state.webview = Some(webview);
            Ok(())
        }

        fn Unload(&self) -> Result<()> {
            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            if let Some(webview) = state.webview.as_ref() {
                if let Ok(webview3) = webview.cast::<ICoreWebView2_3>() {
                    let host = CoTaskMemPWSTR::from("mdview.local");
                    unsafe {
                        let _ = webview3.ClearVirtualHostNameToFolderMapping(*host.as_ref().as_pcwstr());
                    }
                }
            }
            if let Some(controller) = state.controller.take() {
                unsafe {
                    let _ = controller.SetIsVisible(false);
                    let _ = controller.Close();
                }
            }
            state.environment = None;
            state.webview = None;
            state.file_path = None;
            Ok(())
        }

        fn SetFocus(&self) -> Result<()> {
            Ok(())
        }

        fn QueryFocus(&self) -> Result<HWND> {
            let state = self.state.lock().map_err(|_| E_FAIL)?;
            Ok(state.parent_hwnd)
        }

        fn TranslateAccelerator(&self, _pmsg: *const MSG) -> Result<()> {
            Ok(())
        }
    }

    fn create_webview_environment() -> Result<ICoreWebView2Environment> {
        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(|handler| unsafe {
                CreateCoreWebView2Environment(&handler).map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, environment| {
                error_code?;
                tx.send(environment.ok_or_else(|| Error::from(E_POINTER)))
                    .map_err(|_| Error::from(E_FAIL))?;
                Ok(())
            }),
        )
        .map_err(|_| E_FAIL)?;

        let environment = webview2_com::wait_with_pump(rx).map_err(|_| E_FAIL)?;
        environment.map_err(|_| E_FAIL.into())
    }

    fn create_webview_controller(
        environment: &ICoreWebView2Environment,
        parent_hwnd: HWND,
    ) -> Result<ICoreWebView2Controller> {
        let environment = environment.clone();
        let (tx, rx) = mpsc::channel();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                environment
                    .CreateCoreWebView2Controller(parent_hwnd, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                tx.send(controller.ok_or_else(|| Error::from(E_POINTER)))
                    .map_err(|_| Error::from(E_FAIL))?;
                Ok(())
            }),
        )
        .map_err(|_| E_FAIL)?;

        let controller = webview2_com::wait_with_pump(rx).map_err(|_| E_FAIL)?;
        controller.map_err(|_| E_FAIL.into())
    }

    fn apply_bounds(controller: &Option<ICoreWebView2Controller>, bounds: RECT) -> Result<()> {
        if let Some(controller) = controller {
            unsafe {
                controller.SetBounds(bounds)?;
                controller.SetIsVisible(true)?;
            }
        }
        Ok(())
    }

    fn navigate_to_markup(webview: &ICoreWebView2, html: &str) -> Result<()> {
        let html_utf16 = CoTaskMemPWSTR::from(html);
        unsafe { webview.NavigateToString(*html_utf16.as_ref().as_pcwstr()) }
    }

    fn apply_virtual_host_mapping(webview: &ICoreWebView2, file_path: &str) -> Result<()> {
        let mapped_dir = parent_dir_for_mapping(file_path)?;
        let webview3: ICoreWebView2_3 = webview.cast()?;
        let host = CoTaskMemPWSTR::from("mdview.local");
        let folder = CoTaskMemPWSTR::from(mapped_dir.as_str());
        unsafe {
            webview3.SetVirtualHostNameToFolderMapping(
                *host.as_ref().as_pcwstr(),
                *folder.as_ref().as_pcwstr(),
                COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
            )?;
        }
        Ok(())
    }

    fn parent_dir_for_mapping(file_path: &str) -> Result<String> {
        let parent = Path::new(file_path).parent().ok_or(E_FAIL)?;
        if parent.as_os_str().is_empty() {
            return Err(E_FAIL.into());
        }
        Ok(parent.to_string_lossy().into_owned())
    }

    fn render_preview_page(path: &str) -> String {
        let css_vars = tokens_from_snapshot(&current_snapshot()).to_css_vars();
        let shell_css = "html,body{margin:0;padding:0;background:var(--mdv-bg,#1E1E1E);color:var(--mdv-text,#F3F3F3);font-family:\"Segoe UI\",sans-serif;}.mdv-content{padding:16px;line-height:1.6;}a{color:var(--mdv-accent,#0A84FF);}code,pre{background:var(--mdv-code-bg,#2D2D2D);border-radius:6px;}pre{padding:10px;overflow:auto;}table{border-collapse:collapse;}th,td{border:1px solid var(--mdv-border,#3C3C3C);padding:6px 8px;}.mdv-error{margin:16px;padding:14px 16px;border:1px solid var(--mdv-border,#3C3C3C);border-radius:10px;background:var(--mdv-surface,#252526);}.mdv-error h1{margin:0 0 8px;font-size:18px;}.mdv-error p{margin:0 0 8px;}.mdv-error pre{margin:0;white-space:pre-wrap;word-break:break-word;}";

        match fs::read_to_string(path) {
            Ok(markdown) => {
                let rendered = MarkdownEngine::default().render(&markdown);
                format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"color-scheme\" content=\"light dark\"><base href=\"http://mdview.local/\" /><style>{css_vars}</style><style>{shell_css}</style></head><body><article class=\"mdv-content\">{}</article></body></html>",
                    rendered.html
                )
            }
            Err(err) => {
                let escaped_path = escape_html(path);
                let escaped_error = escape_html(&err.to_string());
                format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"color-scheme\" content=\"light dark\"><base href=\"http://mdview.local/\" /><style>{css_vars}</style><style>{shell_css}</style></head><body><section class=\"mdv-error\"><h1>Unable to preview Markdown file</h1><p>The file could not be read. Text preview is unavailable for this item.</p><pre>Path: {escaped_path}\nError: {escaped_error}</pre></section></body></html>"
                )
            }
        }
    }

    fn escape_html(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }

    fn paint_parent_background(parent_hwnd: HWND) {
        let tokens = tokens_from_snapshot(&current_snapshot());
        let Some(color) = parse_hex_rgb_colorref(&tokens.bg) else {
            return;
        };

        unsafe {
            let dc = GetDC(Some(parent_hwnd));
            if dc.is_invalid() {
                return;
            }

            let mut rect = RECT::default();
            if GetClientRect(parent_hwnd, &mut rect).is_err() {
                let _ = ReleaseDC(Some(parent_hwnd), dc);
                return;
            }

            let brush: HBRUSH = CreateSolidBrush(color);
            if !brush.is_invalid() {
                let _ = FillRect(dc, &rect, brush);
                let _ = DeleteObject(brush.into());
                let _ = InvalidateRect(Some(parent_hwnd), Some(&rect as *const RECT), false);
            }

            let _ = ReleaseDC(Some(parent_hwnd), dc);
        }
    }

    fn parse_hex_rgb_colorref(value: &str) -> Option<COLORREF> {
        let hex = value.strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }

        let rgb = u32::from_str_radix(hex, 16).ok()?;
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        Some(COLORREF((b << 16) | (g << 8) | r))
    }

    #[implement(IClassFactory)]
    struct MarkdownPreviewHandlerFactory;

    #[allow(non_snake_case)]
    impl IClassFactory_Impl for MarkdownPreviewHandlerFactory_Impl {
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

            // Start from IUnknown and let COM query to the requested interface.
            let unknown: IUnknown = MarkdownPreviewHandler::new().into();
            let hr = unsafe { unknown.query(riid, ppvobject) };
            hr.ok()
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
        if ACTIVE_OBJECTS.load(Ordering::SeqCst) == 0 {
            S_OK
        } else {
            S_FALSE
        }
    }

    #[no_mangle]
    pub extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        let _ = PREVIEW_HANDLER_PROGID;
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_INVALIDARG;
        }

        unsafe {
            if *rclsid != PREVIEW_HANDLER_CLSID {
                return CLASS_E_CLASSNOTAVAILABLE;
            }
        }

        let factory: IClassFactory = MarkdownPreviewHandlerFactory.into();
        unsafe { factory.query(riid, ppv) }
    }

    #[allow(dead_code)]
    fn _assert_types(_: Option<IClassFactory>, _: Option<IInitializeWithFile>, _: Option<IPreviewHandler>) {}
}

#[cfg(not(windows))]
mod preview_handler {
    pub const PREVIEW_HANDLER_PROGID: &str = "mdview.PreviewHandler";
}
