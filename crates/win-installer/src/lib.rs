#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod windows_impl {
    use std::env;
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::path::{Path, PathBuf};

    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const PREVIEW_HANDLER_CLSID: &str = "{4F831CA2-0DB6-4F14-A4F2-8AB7DE6F6601}";
    const PREVIEW_HANDLER_PROGID: &str = "mdview.PreviewHandler";
    const PREVIEW_HANDLER_NAME: &str = "mdview Markdown Preview Handler";
    const PREVIEW_HANDLER_SHELLEX_KEY: &str = "{8895B1C6-B41F-4C1C-A562-0D564250836F}";
    const PREVHOST_APPID: &str = "{6D2B5079-2F0B-48DD-AB7F-97CEC514D30B}";
    const CONTEXT_MENU_VERB: &str = "mdview";
    const CONTEXT_MENU_LABEL: &str = "Open with mdview";
    const MARKDOWN_PROGID: &str = "mdview.MarkdownFile";
    const MARKDOWN_PROGID_NAME: &str = "Markdown Document (mdview)";
    const APP_EXE_NAME: &str = "viewer-shell.exe";
    const APP_REGISTERED_NAME: &str = "mdview";
    const APP_CAPABILITIES_REL_PATH: &str =
        "Software\\Classes\\Applications\\viewer-shell.exe\\Capabilities";
    const CURRENT_VERSION_PREVIEW_HANDLERS: &str =
        "Software\\Microsoft\\Windows\\CurrentVersion\\PreviewHandlers";

    #[derive(Debug)]
    pub enum InstallerError {
        Io(std::io::Error),
        MissingCurrentExe,
        MissingPreviewDll(PathBuf),
    }

    impl Display for InstallerError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Io(err) => write!(f, "{err}"),
                Self::MissingCurrentExe => write!(f, "unable to resolve current executable path"),
                Self::MissingPreviewDll(path) => {
                    write!(f, "preview handler DLL not found: {}", path.display())
                }
            }
        }
    }

    impl Error for InstallerError {}

    impl From<std::io::Error> for InstallerError {
        fn from(value: std::io::Error) -> Self {
            Self::Io(value)
        }
    }

    pub fn register_preview_handler() -> Result<(), InstallerError> {
        let dll_path = locate_preview_dll()?;
        let classes = classes_root()?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        let clsid_root = format!("CLSID\\{PREVIEW_HANDLER_CLSID}");
        let (clsid_key, _) = classes.create_subkey(&clsid_root)?;
        clsid_key.set_value("", &PREVIEW_HANDLER_NAME)?;
        clsid_key.set_value("ProgID", &PREVIEW_HANDLER_PROGID)?;
        clsid_key.set_value("AppID", &PREVHOST_APPID)?;
        clsid_key.set_value("DisableLowILProcessIsolation", &1u32)?;

        let (inproc_key, _) = classes.create_subkey(format!("{clsid_root}\\InprocServer32"))?;
        inproc_key.set_value("", &dll_path.to_string_lossy().to_string())?;
        inproc_key.set_value("ThreadingModel", &"Apartment")?;

        let (progid_key, _) = classes.create_subkey(PREVIEW_HANDLER_PROGID)?;
        progid_key.set_value("", &PREVIEW_HANDLER_NAME)?;
        let (progid_clsid_key, _) =
            classes.create_subkey(format!("{PREVIEW_HANDLER_PROGID}\\CLSID"))?;
        progid_clsid_key.set_value("", &PREVIEW_HANDLER_CLSID)?;

        let (preview_handlers_key, _) = hkcu.create_subkey(CURRENT_VERSION_PREVIEW_HANDLERS)?;
        preview_handlers_key.set_value(PREVIEW_HANDLER_CLSID, &PREVIEW_HANDLER_NAME)?;

        for ext in [".md", ".markdown"] {
            let shellex_path = format!("{ext}\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}");
            let (shellex_key, _) = classes.create_subkey(shellex_path)?;
            shellex_key.set_value("", &PREVIEW_HANDLER_CLSID)?;

            if let Ok(ext_key) = classes.open_subkey(ext) {
                let progid: Result<String, _> = ext_key.get_value("");
                if let Ok(progid) = progid {
                    let progid = progid.trim();
                    if !progid.is_empty() {
                        let progid_shellex_path =
                            format!("{progid}\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}");
                        let (progid_shellex_key, _) = classes.create_subkey(progid_shellex_path)?;
                        progid_shellex_key.set_value("", &PREVIEW_HANDLER_CLSID)?;
                    }
                }
            }
        }

        let (markdown_shellex_key, _) = classes.create_subkey(format!(
            "{MARKDOWN_PROGID}\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}"
        ))?;
        markdown_shellex_key.set_value("", &PREVIEW_HANDLER_CLSID)?;

        Ok(())
    }

    pub fn register_context_menu() -> Result<(), InstallerError> {
        let exe_path = current_exe_path()?;
        let command_value = format!("\"{}\" \"%1\"", exe_path.display());
        let icon_value = exe_path.to_string_lossy().to_string();
        let classes = classes_root()?;

        for ext in [".md", ".markdown"] {
            let root = format!("SystemFileAssociations\\{ext}\\shell\\{CONTEXT_MENU_VERB}");
            let (menu_key, _) = classes.create_subkey(&root)?;
            menu_key.set_value("FriendlyAppName", &CONTEXT_MENU_LABEL)?;
            menu_key.set_value("Icon", &icon_value)?;
            menu_key.set_value("", &CONTEXT_MENU_LABEL)?;

            let (command_key, _) = classes.create_subkey(format!("{root}\\command"))?;
            command_key.set_value("", &command_value)?;
        }

        Ok(())
    }

    pub fn register_open_with_and_capabilities() -> Result<(), InstallerError> {
        let exe_path = current_exe_path()?;
        let command_value = format!("\"{}\" \"%1\"", exe_path.display());
        let icon_value = format!("{},0", exe_path.display());
        let classes = classes_root()?;

        // ProgID used by Open With and Default Apps mapping.
        let (progid_key, _) = classes.create_subkey(MARKDOWN_PROGID)?;
        progid_key.set_value("", &MARKDOWN_PROGID_NAME)?;
        let (progid_icon_key, _) = classes.create_subkey(format!("{MARKDOWN_PROGID}\\DefaultIcon"))?;
        progid_icon_key.set_value("", &icon_value)?;
        let (progid_cmd_key, _) =
            classes.create_subkey(format!("{MARKDOWN_PROGID}\\shell\\open\\command"))?;
        progid_cmd_key.set_value("", &command_value)?;

        // Register app executable for Open With.
        let app_root = format!("Applications\\{APP_EXE_NAME}");
        let (app_key, _) = classes.create_subkey(&app_root)?;
        app_key.set_value("FriendlyAppName", &"mdview")?;
        let (app_cmd_key, _) = classes.create_subkey(format!("{app_root}\\shell\\open\\command"))?;
        app_cmd_key.set_value("", &command_value)?;
        let (supported_types_key, _) = classes.create_subkey(format!("{app_root}\\SupportedTypes"))?;
        supported_types_key.set_value(".md", &"")?;
        supported_types_key.set_value(".markdown", &"")?;

        // Associate extensions with our ProgID as an available handler (without forcing default).
        for ext in [".md", ".markdown"] {
            let (open_with_progids_key, _) = classes.create_subkey(format!("{ext}\\OpenWithProgids"))?;
            open_with_progids_key.set_value(MARKDOWN_PROGID, &"")?;
            let (open_with_list_key, _) =
                classes.create_subkey(format!("{ext}\\OpenWithList\\{APP_EXE_NAME}"))?;
            open_with_list_key.set_value("", &"")?;
        }

        // Register for Windows Default Apps UI.
        let (cap_key, _) = classes.create_subkey(format!("{app_root}\\Capabilities"))?;
        cap_key.set_value("ApplicationName", &"mdview")?;
        cap_key.set_value(
            "ApplicationDescription",
            &"Windows-first markdown viewer with Explorer preview support.",
        )?;
        let (file_assoc_key, _) = classes.create_subkey(format!("{app_root}\\Capabilities\\FileAssociations"))?;
        file_assoc_key.set_value(".md", &MARKDOWN_PROGID)?;
        file_assoc_key.set_value(".markdown", &MARKDOWN_PROGID)?;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (registered_apps_key, _) = hkcu.create_subkey("Software\\RegisteredApplications")?;
        registered_apps_key.set_value(APP_REGISTERED_NAME, &APP_CAPABILITIES_REL_PATH)?;

        Ok(())
    }

    pub fn unregister_all() -> Result<(), InstallerError> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let classes = classes_root()?;

        let _ = classes.delete_subkey_all(format!("CLSID\\{PREVIEW_HANDLER_CLSID}\\InprocServer32"));
        let _ = classes.delete_subkey_all(format!("CLSID\\{PREVIEW_HANDLER_CLSID}"));
        let _ = classes.delete_subkey_all(format!("{PREVIEW_HANDLER_PROGID}\\CLSID"));
        let _ = classes.delete_subkey_all(PREVIEW_HANDLER_PROGID);
        let _ = classes.delete_subkey_all(MARKDOWN_PROGID);
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}\\shell\\open\\command"));
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}\\shell\\open"));
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}\\shell"));
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}\\SupportedTypes"));
        let _ = classes.delete_subkey_all(format!(
            "Applications\\{APP_EXE_NAME}\\Capabilities\\FileAssociations"
        ));
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}\\Capabilities"));
        let _ = classes.delete_subkey_all(format!("Applications\\{APP_EXE_NAME}"));
        let _ = classes.delete_subkey_all(format!(".md\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}"));
        let _ =
            classes.delete_subkey_all(format!(".markdown\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}"));
        let _ = classes.delete_subkey_all(format!(
            "{MARKDOWN_PROGID}\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}"
        ));
        for progid in ["md_auto_file", "markdown_auto_file"] {
            let _ = classes.delete_subkey_all(format!(
                "{progid}\\shellex\\{PREVIEW_HANDLER_SHELLEX_KEY}"
            ));
        }
        let _ = classes.delete_subkey_all("SystemFileAssociations\\.md\\shell\\mdview\\command");
        let _ = classes.delete_subkey_all("SystemFileAssociations\\.md\\shell\\mdview");
        let _ =
            classes.delete_subkey_all("SystemFileAssociations\\.markdown\\shell\\mdview\\command");
        let _ = classes.delete_subkey_all("SystemFileAssociations\\.markdown\\shell\\mdview");
        let _ = classes.delete_subkey_all(format!(".md\\OpenWithList\\{APP_EXE_NAME}"));
        let _ = classes.delete_subkey_all(format!(".markdown\\OpenWithList\\{APP_EXE_NAME}"));

        if let Ok(preview_handlers_key) = hkcu.open_subkey_with_flags(
            CURRENT_VERSION_PREVIEW_HANDLERS,
            winreg::enums::KEY_SET_VALUE,
        ) {
            let _ = preview_handlers_key.delete_value(PREVIEW_HANDLER_CLSID);
        }

        if let Ok(open_with_progids_md) =
            classes.open_subkey_with_flags(".md\\OpenWithProgids", winreg::enums::KEY_SET_VALUE)
        {
            let _ = open_with_progids_md.delete_value(MARKDOWN_PROGID);
        }
        if let Ok(open_with_progids_markdown) = classes.open_subkey_with_flags(
            ".markdown\\OpenWithProgids",
            winreg::enums::KEY_SET_VALUE,
        ) {
            let _ = open_with_progids_markdown.delete_value(MARKDOWN_PROGID);
        }

        if let Ok(registered_apps_key) =
            hkcu.open_subkey_with_flags("Software\\RegisteredApplications", winreg::enums::KEY_SET_VALUE)
        {
            let _ = registered_apps_key.delete_value(APP_REGISTERED_NAME);
        }

        Ok(())
    }

    pub fn register_all() -> Result<(), InstallerError> {
        register_preview_handler()?;
        register_context_menu()?;
        register_open_with_and_capabilities()?;
        Ok(())
    }

    fn classes_root() -> Result<RegKey, InstallerError> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Classes", winreg::enums::KEY_ALL_ACCESS)
            .map_err(InstallerError::Io)
    }

    fn current_exe_path() -> Result<PathBuf, InstallerError> {
        env::current_exe().map_err(|_| InstallerError::MissingCurrentExe)
    }

    fn locate_preview_dll() -> Result<PathBuf, InstallerError> {
        let exe = current_exe_path()?;
        let exe_dir = exe
            .parent()
            .map(Path::to_path_buf)
            .ok_or(InstallerError::MissingCurrentExe)?;

        let candidate = exe_dir.join("win_preview_handler.dll");
        if candidate.exists() {
            return Ok(candidate);
        }

        Err(InstallerError::MissingPreviewDll(candidate))
    }
}

#[cfg(windows)]
pub use windows_impl::{
    register_all, register_context_menu, register_open_with_and_capabilities,
    register_preview_handler, unregister_all, InstallerError,
};

#[cfg(not(windows))]
#[derive(Debug)]
pub enum InstallerError {
    UnsupportedPlatform,
}

#[cfg(not(windows))]
impl std::fmt::Display for InstallerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported platform")
    }
}

#[cfg(not(windows))]
pub fn register_preview_handler() -> Result<(), InstallerError> {
    Err(InstallerError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn register_context_menu() -> Result<(), InstallerError> {
    Err(InstallerError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn register_open_with_and_capabilities() -> Result<(), InstallerError> {
    Err(InstallerError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn unregister_all() -> Result<(), InstallerError> {
    Err(InstallerError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub fn register_all() -> Result<(), InstallerError> {
    Err(InstallerError::UnsupportedPlatform)
}
