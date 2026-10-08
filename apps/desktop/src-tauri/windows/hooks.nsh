; Fuselane offers to open .torrent files and magnet links without taking them over
; (P5 5.3, "Alternate handler only"). Tauri's own file associations overwrite the
; default handler, so they aren't used on Windows. Instead:
;  - "Open with" lists Fuselane for .torrent files (OpenWithProgids), and
;  - Settings > Default apps lists it for .torrent and magnet (RegisteredApplications).
; Nothing here changes which app is the default.
; Uninstalling also removes the browser extension's host registration.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\Fuselane.torrent" "" "BitTorrent file"
  WriteRegStr SHCTX "Software\Classes\Fuselane.torrent\DefaultIcon" "" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr SHCTX "Software\Classes\Fuselane.torrent\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
  WriteRegStr SHCTX "Software\Classes\.torrent\OpenWithProgids" "Fuselane.torrent" ""

  WriteRegStr SHCTX "Software\Classes\Fuselane.magnet" "" "Magnet link"
  WriteRegStr SHCTX "Software\Classes\Fuselane.magnet" "URL Protocol" ""
  WriteRegStr SHCTX "Software\Classes\Fuselane.magnet\DefaultIcon" "" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr SHCTX "Software\Classes\Fuselane.magnet\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'

  WriteRegStr SHCTX "Software\Fuselane\Capabilities" "ApplicationName" "Fuselane"
  WriteRegStr SHCTX "Software\Fuselane\Capabilities" "ApplicationDescription" "Downloads over every network at once"
  WriteRegStr SHCTX "Software\Fuselane\Capabilities\FileAssociations" ".torrent" "Fuselane.torrent"
  WriteRegStr SHCTX "Software\Fuselane\Capabilities\URLAssociations" "magnet" "Fuselane.magnet"
  WriteRegStr SHCTX "Software\RegisteredApplications" "Fuselane" "Software\Fuselane\Capabilities"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue SHCTX "Software\Classes\.torrent\OpenWithProgids" "Fuselane.torrent"
  DeleteRegKey SHCTX "Software\Classes\Fuselane.torrent"
  DeleteRegKey SHCTX "Software\Classes\Fuselane.magnet"
  DeleteRegValue SHCTX "Software\RegisteredApplications" "Fuselane"
  DeleteRegKey SHCTX "Software\Fuselane\Capabilities"

  ; The browser extension's helper (the app writes these on every launch, STEPS 7.4).
  DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\app.fuselane.host"
  DeleteRegKey HKCU "Software\Chromium\NativeMessagingHosts\app.fuselane.host"
  DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\app.fuselane.host"
  DeleteRegKey HKCU "Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\app.fuselane.host"
  DeleteRegKey HKCU "Software\Mozilla\NativeMessagingHosts\app.fuselane.host"
  RMDir /r "$APPDATA\Fuselane\NativeMessagingHosts"
!macroend
