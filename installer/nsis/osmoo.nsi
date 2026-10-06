; OSMOO NSIS Installer Script
; Product: OSMOO (Operating System Machine Optimization Operator)
; Parent: Osmiora Computational Systems
; Build: makensis osmoo.nsi

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; ── Version Information ──────────────────────────────────────────────
!define PRODUCT_NAME "OSMOO"
!define PRODUCT_PUBLISHER "Osmiora"
!define PRODUCT_WEB_SITE "https://osmoo.in"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"
!define PRODUCT_VERSION "1.0.0"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "OSMOO-${PRODUCT_VERSION}-Setup-x64.exe"
InstallDir "$PROGRAMFILES\OSMOO"
InstallDirRegKey HKLM "Software\OSMOO" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

; ── MUI Settings ─────────────────────────────────────────────────────
!define MUI_ABORTWARNING
!define MUI_ICON "..\..\assets\icons\voxy.ico"
!define MUI_UNICON "..\..\assets\icons\voxy.ico"

; ── Pages ─────────────────────────────────────────────────────────────
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

; ── Installer Sections ────────────────────────────────────────────────
Section "OSMOO Core Application (required)" SecMain
  SectionIn RO

  SetOutPath "$INSTDIR"
  SetOverwrite on

  ; Create runtime directories
  CreateDirectory "$INSTDIR\config"
  CreateDirectory "$INSTDIR\logs"

  ; Install primary production binaries and components
  File "..\OSMOO\OSMOO.exe"
  File "..\OSMOO\voxy-daemon.exe"
  File "..\OSMOO\voxy-overlay.exe"
  File "..\..\LICENSE"
  File "..\..\README.md"

  ; Store installation folder
  WriteRegStr HKLM "Software\OSMOO" "InstallDir" "$INSTDIR"

  ; Create Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\OSMOO.exe"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME} Companion Overlay.lnk" "$INSTDIR\voxy-overlay.exe"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall ${PRODUCT_NAME}.lnk" "$INSTDIR\uninstall.exe"

  ; Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Write uninstall registry keys
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\OSMOO.exe"
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
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\OSMOO.exe"
SectionEnd

; ── Uninstaller Section ───────────────────────────────────────────────
Section "Uninstall"
  ; Remove application files (Preserving User Data in %APPDATA%\OSMOO)
  Delete "$INSTDIR\OSMOO.exe"
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

  ; Remove registry keys
  DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM "Software\OSMOO"
SectionEnd

; ── Callbacks ─────────────────────────────────────────────────────────
Function .onInit
  ; Check if already installed
  ReadRegStr $0 HKLM "Software\OSMOO" "InstallDir"
  ${If} $0 != ""
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "${PRODUCT_NAME} is already installed. Overwrite with new version?" \
      IDYES continueInstall
    Abort
  ${EndIf}

  continueInstall:
FunctionEnd

Function un.onInit
  MessageBox MB_YESNO|MB_ICONQUESTION \
    "Are you sure you want to completely uninstall ${PRODUCT_NAME}? Your user workspace configuration will be preserved." \
    IDYES proceedUninstall
  Abort

  proceedUninstall:
FunctionEnd
