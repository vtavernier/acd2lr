; Required defines:
; ROOT
; INSTALLSIZE
; OUTFILE
; VERSIONMAJOR
; VERSIONMINOR
; VERSIONBUILD

;--------------------------------
;Include Modern UI

!include "MUI2.nsh"

;--------------------------------
;General
!define APPNAME "ACDSee to Lightroom metadata converter"
!define SHORTNAME "acd2lr"
!define COMPANYNAME "Vincent Tavernier"
!define DESCRIPTION "ACDSee to Lightroom metadata converter"
!define ABOUTURL "https://vtavernier.github.io/"

;Name and file
Name "${APPNAME}"
Icon "acd2lr/app.ico"
OutFile "${OUTFILE}"
Unicode True

;Default installation folder
InstallDir "$PROGRAMFILES64\${SHORTNAME}"

;Get installation folder from registry if available
InstallDirRegKey HKLM "Software\${SHORTNAME}" ""

;Request application privileges for Windows Vista
RequestExecutionLevel admin

;--------------------------------
;Interface Settings

!define MUI_ABORTWARNING

;--------------------------------
;Pages

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

;--------------------------------
;Languages

!insertmacro MUI_LANGUAGE "French"

;--------------------------------
;Installer Sections

LangString TITLE_SecMainProgram ${LANG_FRENCH} "Programme principal"
LangString DESC_SecMainProgram ${LANG_FRENCH} "Programme principal."

Section $(TITLE_SecMainProgram) SecMainProgram
  SectionIn RO

  SetOutPath "$INSTDIR"

  File /r ${ROOT}/bin
  File /r ${ROOT}/etc
  File /r ${ROOT}/lib
  File /r ${ROOT}/share

  ;Store installation folder
  WriteRegStr HKCU "Software\${SHORTNAME}" "" $INSTDIR

  ;Create uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  # Registry information for add/remove programs
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "DisplayIcon" "$INSTDIR\bin\${SHORTNAME}.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "Publisher" "${COMPANYNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "URLInfoAbout" "${ABOUTURL}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "DisplayVersion" "${VERSIONMAJOR}.${VERSIONMINOR}.${VERSIONBUILD}"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "VersionMajor" ${VERSIONMAJOR}
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "VersionMinor" ${VERSIONMINOR}
  # There is no option for modifying or repairing the install
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "NoRepair" 1
  # Set the INSTALLSIZE constant (!defined at the top of this script) so Add/Remove Programs can accurately report the size
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}" "EstimatedSize" ${INSTALLSIZE}
SectionEnd

LangString TITLE_SecStartMenuShortcut ${LANG_FRENCH} "Menu démarrer"
LangString DESC_SecStartMenuShortcut ${LANG_FRENCH} "Ajouter un raccourci dans le menu démarrer."

Section $(TITLE_SecStartMenuShortcut) SecStartMenuShortcut
  CreateShortCut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\bin\${SHORTNAME}.exe" "" "$INSTDIR\bin\${SHORTNAME}.exe"
SectionEnd

LangString TITLE_SecContextMenu ${LANG_FRENCH} "Menu contextuel"
LangString DESC_SecContextMenu ${LANG_FRENCH} "Ajouter un raccourci dans le menu contextuel de l'Explorateur Windows."

LangString ContextMenuEntry ${LANG_FRENCH} "${APPNAME}"

!macro CONTEXT_MENU TYPE
  WriteRegStr HKCR "${TYPE}\shell\$(ContextMenuEntry)\command" "" "$\"$INSTDIR\bin\${SHORTNAME}.exe$\" $\"%1$\""
  WriteRegStr HKCR "${TYPE}\shell\$(ContextMenuEntry)" "Icon" "$INSTDIR\bin\${SHORTNAME}.exe,0"
!macroend

!macro UNINSTALL_CONTEXT_MENU TYPE
  DeleteRegKey HKCR "${TYPE}\shell\$(ContextMenuEntry)"
!macroend

Section $(TITLE_SecContextMenu) SecContextMenu
  !insertmacro CONTEXT_MENU "Directory"
  !insertmacro CONTEXT_MENU ".jpg"
  !insertmacro CONTEXT_MENU ".jpeg"
  !insertmacro CONTEXT_MENU ".tiff"
  !insertmacro CONTEXT_MENU ".xmp"
SectionEnd

;--------------------------------
;Descriptions

;Assign language strings to sections
!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
!insertmacro MUI_DESCRIPTION_TEXT ${SecMainProgram} $(DESC_SecMainProgram)
!insertmacro MUI_DESCRIPTION_TEXT ${SecStartMenuShortcut} $(DESC_SecStartMenuShortcut)
!insertmacro MUI_DESCRIPTION_TEXT ${SecContextMenu} $(DESC_SecContextMenu)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

;--------------------------------
;Uninstaller Section

Section "Uninstall"
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$INSTDIR\uninstall.exe"

  RMDir /r "$INSTDIR"

  !insertmacro UNINSTALL_CONTEXT_MENU "Directory"
  !insertmacro UNINSTALL_CONTEXT_MENU ".jpg"
  !insertmacro UNINSTALL_CONTEXT_MENU ".jpeg"
  !insertmacro UNINSTALL_CONTEXT_MENU ".tiff"
  !insertmacro UNINSTALL_CONTEXT_MENU ".xmp"

  DeleteRegKey /ifempty HKCU "Software\${SHORTNAME}"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${COMPANYNAME} ${APPNAME}"
SectionEnd
