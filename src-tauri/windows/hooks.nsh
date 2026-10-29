; Installer hooks (bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_POSTINSTALL
  ; Explorer caches icons per executable path, so after an update the desktop and Start menu
  ; shortcuts and the taskbar keep showing the old icon. Tell the shell to reload them.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
  nsExec::Exec '"$SYSDIR\ie4uinit.exe" -show'
!macroend
