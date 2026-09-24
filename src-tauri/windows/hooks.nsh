; Tauri registers and unregisters the association. Only replace its icon.
; SHCTX follows Tauri's current-user / all-users installation context.
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\MTR Pack Studio Project\DefaultIcon" "" '$\"$INSTDIR\icons\mtrpack.ico$\",0'
  !insertmacro UPDATEFILEASSOC
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro UPDATEFILEASSOC
!macroend
