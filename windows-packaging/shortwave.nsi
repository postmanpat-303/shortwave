; Shortwave 5.1.0 - Windows installer (NSIS, 64-bit)
; Packages the self-contained bundle (bin/ lib/ share/) built from
; gitlab.gnome.org/World/Shortwave @ e468e17 with MSYS2/UCRT64.

Unicode true
SetCompressor /SOLID lzma
SetCompressorDictSize 64

!define APPNAME "Shortwave"
!define APPVERSION "5.1.0"
!define APPPUBLISHER "Felix Häcker (unofficial Windows build)"
!define APPURL "https://apps.gnome.org/Shortwave/"
!define APPID "de.haeckerfelix.Shortwave"

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"

Name "${APPNAME}"
OutFile "shortwave-${APPVERSION}-win64-setup.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel admin

; --- Interface settings ---
!define MUI_ABORTWARNING
!define MUI_ICON "shortwave.ico"
!define MUI_UNICON "shortwave.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\bin\shortwave.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Shortwave"

; --- Pages ---
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "COPYING.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

; --- Sections ---
Section "!Shortwave (required)" SecApp
    SectionIn RO
    SetOutPath "$INSTDIR"
    File /r "shortwave\bundle\*.*"

    ; Start menu shortcut
    CreateDirectory "$SMPROGRAMS\${APPNAME}"
    CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" \
        "$INSTDIR\bin\shortwave.exe" "" "$INSTDIR\bin\shortwave.exe" 0
    CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" \
        "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0

    ; Uninstaller + registry
    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\${APPNAME}" "InstallDir" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "DisplayName" "${APPNAME}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "DisplayIcon" "$INSTDIR\bin\shortwave.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "Publisher" "${APPPUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "URLInfoAbout" "${APPURL}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "DisplayVersion" "${APPVERSION}"

    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}" \
        "EstimatedSize" "$0"
SectionEnd

Section "Desktop shortcut" SecDesktop
    CreateShortcut "$DESKTOP\${APPNAME}.lnk" \
        "$INSTDIR\bin\shortwave.exe" "" "$INSTDIR\bin\shortwave.exe" 0
SectionEnd

; --- Uninstaller ---
Section "Uninstall"
    Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
    Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
    RMDir "$SMPROGRAMS\${APPNAME}"
    Delete "$DESKTOP\${APPNAME}.lnk"

    RMDir /r "$INSTDIR\bin"
    RMDir /r "$INSTDIR\lib"
    RMDir /r "$INSTDIR\share"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPID}"
    DeleteRegKey HKLM "Software\${APPNAME}"
SectionEnd
