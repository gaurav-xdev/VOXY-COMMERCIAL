; VOXY NSIS Installer Script
; Build: makensis voxy.nsi
; Requires: NSIS 3.x (https://nsis.sourceforge.io)

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; ── Version Information ──────────────────────────────────────────────
!define PRODUCT_NAME "VOXY"
!define PRODUCT_PUBLISHER "VOXY Engineering Team"
!define PRODUCT_WEB_SITE "https://github.com/voxy-ai/voxy"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"
!define PRODUCT_VERSION "0.1.0"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "voxy-${PRODUCT_VERSION}-setup.exe"
InstallDir "$PROGRAMFILES\VOXY"
InstallDirRegKey HKLM "Software\VOXY" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

; ── MUI Settings ─────────────────────────────────────────────────────
!define MUI_ABORTWARNING
!define MUI_ICON "assets\icons\voxy.ico"
!define MUI_UNICON "assets\icons\voxy.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "installer\nsis\header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "installer\nsis\welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "installer\nsis\welcome.bmp"

; ── Pages ─────────────────────────────────────────────────────────────
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

; ── Installer Sections ────────────────────────────────────────────────
Section "VOXY (required)" SecMain
  SectionIn RO

  SetOutPath "$INSTDIR"
  SetOverwrite on

  ; Create directories
  CreateDirectory "$INSTDIR\config"
  CreateDirectory "$INSTDIR\logs"

  ; Install files
  File "target\release\voxy-daemon.exe"
  File "target\release\voxy-overlay.exe"
  File "LICENSE"
  File "README.md"

  ; Store installation folder
  WriteRegStr HKLM "Software\VOXY" "InstallDir" "$INSTDIR"

  ; Add to PATH
  ${EnvVarUpdate} "0" "PATH" "append" "HKLM" "$INSTDIR"

  ; Create Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\voxy-overlay.exe"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME} Daemon.lnk" "$INSTDIR\voxy-daemon.exe"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk" "$INSTDIR\uninstall.exe"

  ; Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Write uninstall registry keys
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\voxy-overlay.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoRepair" 1

  ; Get installed size
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Desktop Shortcut" SecDesktop
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\voxy-overlay.exe"
SectionEnd

; ── Uninstaller Section ───────────────────────────────────────────────
Section "Uninstall"
  ; Remove files
  Delete "$INSTDIR\voxy-daemon.exe"
  Delete "$INSTDIR\voxy-overlay.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\uninstall.exe"

  ; Remove directories
  RMDir /r "$INSTDIR\config"
  RMDir /r "$INSTDIR\logs"
  RMDir "$INSTDIR"

  ; Remove Start Menu shortcuts
  RMDir /r "$SMPROGRAMS\${PRODUCT_NAME}"

  ; Remove desktop shortcut
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  ; Remove from PATH
  ${EnvVarUpdate} "1" "PATH" "remove" "HKLM" "$INSTDIR"

  ; Remove registry keys
  DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM "Software\VOXY"
SectionEnd

; ── Callbacks ─────────────────────────────────────────────────────────
Function .onInit
  ; Check if already installed
  ReadRegStr $0 HKLM "Software\VOXY" "InstallDir"
  ${If} $0 != ""
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "${PRODUCT_NAME} is already installed. Overwrite?" \
      IDYES continueInstall
    Abort
  ${EndIf}

  continueInstall:
FunctionEnd

Function un.onInit
  MessageBox MB_YESNO|MB_ICONQUESTION \
    "Are you sure you want to completely remove ${PRODUCT_NAME}?" \
    IDYES proceedUninstall
  Abort

  proceedUninstall:
FunctionEnd
