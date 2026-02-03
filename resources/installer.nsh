!include "LogicLib.nsh"

!macro customInstall
  IfFileExists "$INSTDIR\\resources\\runtime\\assets\\bin\\vc_redist.x64.exe" 0 done

  SetRegView 64
  ClearErrors
  ReadRegDWORD $0 HKLM "SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64" "Installed"
  ${If} ${Errors}
    StrCpy $0 0
  ${EndIf}

  ${If} $0 != 1
    DetailPrint "Installing Microsoft Visual C++ Redistributable..."
    ExecWait '"$INSTDIR\\resources\\runtime\\assets\\bin\\vc_redist.x64.exe" /install /quiet /norestart'
  ${Else}
    DetailPrint "Microsoft Visual C++ Redistributable already installed."
  ${EndIf}

  done:
!macroend
