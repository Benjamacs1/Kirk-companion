Unicode True
!include "MUI2.nsh"
!define APP "Kirk Companion"
Name "${APP} ${VERSION}"
OutFile "${OUTDIR}\Kirk-Companion-Setup-${VERSION}.exe"
InstallDir "$LOCALAPPDATA\Programs\Kirk Companion Native"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "0.2.0.0"
VIAddVersionKey "ProductName" "${APP}"
VIAddVersionKey "FileDescription" "Kirk Companion native Rust installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 Benja (Benjamacs1). MIT License."
!define MUI_ICON "${ROOT}\src\ui\icon.ico"
!define MUI_UNICON "${ROOT}\src\ui\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\kirk-companion.exe"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${ROOT}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "${ROOT}\target\release\kirk-companion.exe"
  File "${ROOT}\src\ui\icon.ico"
  File "${ROOT}\LICENSE"
  File "${ROOT}\COPYRIGHT.md"
  File "${ROOT}\assets\fonts\OFL-Squares.txt"
  File "${OUTDIR}\THIRD-PARTY-LICENSES.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Kirk Companion"
  CreateShortcut "$SMPROGRAMS\Kirk Companion\Kirk Companion.lnk" "$INSTDIR\kirk-companion.exe"
  WriteRegStr HKCU "Software\Classes\kirk" "" "URL:Kirk Companion"
  WriteRegStr HKCU "Software\Classes\kirk" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\kirk\DefaultIcon" "" "$INSTDIR\icon.ico"
  WriteRegStr HKCU "Software\Classes\kirk\shell\open\command" "" '$\"$INSTDIR\kirk-companion.exe$\" $\"%1$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "DisplayName" "${APP}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "Publisher" "Benjamacs1"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "DisplayIcon" "$INSTDIR\icon.ico"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  ReadRegStr $0 HKCU "Software\Classes\kirk\shell\open\command" ""
  StrCmp $0 '$\"$INSTDIR\kirk-companion.exe$\" $\"%1$\"' 0 +2
    DeleteRegKey HKCU "Software\Classes\kirk"
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kirk Companion"
  StrCmp $0 '$\"$INSTDIR\kirk-companion.exe$\" --background' 0 +2
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kirk Companion"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KirkCompanionNative"
  Delete "$SMPROGRAMS\Kirk Companion\Kirk Companion.lnk"
  RMDir "$SMPROGRAMS\Kirk Companion"
  Delete "$INSTDIR\kirk-companion.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\COPYRIGHT.md"
  Delete "$INSTDIR\OFL-Squares.txt"
  Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ; Saved profiles and the OS credential are kept. Unpair in the app before uninstalling.
SectionEnd
