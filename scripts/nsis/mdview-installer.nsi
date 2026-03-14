Unicode true
ManifestDPIAware true
RequestExecutionLevel user

!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef MDVIEW_SOURCE_DIR
  !error "MDVIEW_SOURCE_DIR is required"
!endif

!ifndef MDVIEW_OUTFILE
  !error "MDVIEW_OUTFILE is required"
!endif

!ifndef MDVIEW_VERSION
  !define MDVIEW_VERSION "dev"
!endif

!define APP_NAME "mdview"
!define APP_EXE "viewer-shell.exe"
!define PREVIEW_DLL "win_preview_handler.dll"
!define INSTALL_DIR "$LOCALAPPDATA\Programs\mdview"

Name "${APP_NAME}"
OutFile "${MDVIEW_OUTFILE}"
InstallDir "${INSTALL_DIR}"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "InstallLocation"
ShowInstDetails show
ShowUninstDetails show

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File /oname=${APP_EXE} "${MDVIEW_SOURCE_DIR}\viewer-shell.exe"
  File /oname=${PREVIEW_DLL} "${MDVIEW_SOURCE_DIR}\win_preview_handler.dll"

  CreateDirectory "$SMPROGRAMS\mdview"
  CreateShortcut "$SMPROGRAMS\mdview\mdview.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortcut "$DESKTOP\mdview.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${MDVIEW_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "mdview"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoRepair" 1

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ExecWait '"$INSTDIR\${APP_EXE}" "--register"' $0
  ${If} $0 <> 0
    DetailPrint "Registration command exited with code $0"
  ${EndIf}
SectionEnd

Section "Uninstall"
  ExecWait '"$INSTDIR\${APP_EXE}" "--unregister"' $0
  ${If} $0 <> 0
    DetailPrint "Unregister command exited with code $0"
  ${EndIf}

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\${PREVIEW_DLL}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\mdview\mdview.lnk"
  RMDir "$SMPROGRAMS\mdview"
  Delete "$DESKTOP\mdview.lnk"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd
