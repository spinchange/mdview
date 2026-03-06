#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod preview_handler {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;

    use windows::core::{implement, GUID, IUnknown, PCWSTR, Result, HRESULT};
    use windows_core::Interface;
    use windows::Win32::Foundation::{
        BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER,
        HWND, RECT, S_FALSE, S_OK,
    };
    use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IInitializeWithFile, IInitializeWithFile_Impl,
    };
    use windows::Win32::UI::Shell::{IPreviewHandler, IPreviewHandler_Impl};
    use windows::Win32::UI::WindowsAndMessaging::MSG;

    static ACTIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);

    pub const PREVIEW_HANDLER_CLSID: GUID =
        GUID::from_u128(0x4f831ca2_0db6_4f14_a4f2_8ab7de6f6601);
    pub const PREVIEW_HANDLER_PROGID: &str = "mdview.PreviewHandler";

    #[derive(Default)]
    struct State {
        file_path: Option<String>,
        parent_hwnd: HWND,
        bounds: RECT,
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
            Ok(())
        }

        fn SetRect(&self, prect: *const RECT) -> Result<()> {
            if prect.is_null() {
                return Err(E_POINTER.into());
            }

            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
            state.bounds = unsafe { *prect };
            Ok(())
        }

        fn DoPreview(&self) -> Result<()> {
            let state = self.state.lock().map_err(|_| E_FAIL)?;
            if state.file_path.is_none() {
                return Err(E_FAIL.into());
            }

            // Phase 1.5 scaffold: rendering host wiring will be added in the next pass.
            Ok(())
        }

        fn Unload(&self) -> Result<()> {
            let mut state = self.state.lock().map_err(|_| E_FAIL)?;
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

    #[implement(IClassFactory)]
    struct MarkdownPreviewHandlerFactory;

    #[allow(non_snake_case)]
    impl IClassFactory_Impl for MarkdownPreviewHandlerFactory_Impl {
        fn CreateInstance(
            &self,
            punkouter: Option<&IUnknown>,
            riid: *const GUID,
            ppvobject: *mut *mut c_void,
        ) -> Result<()> {
            if riid.is_null() || ppvobject.is_null() {
                return Err(E_POINTER.into());
            }

            if punkouter.is_some() {
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
