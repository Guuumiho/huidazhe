@echo off
set "VSDEVCMD=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"

if not exist "%VSDEVCMD%" (
  echo VsDevCmd.bat not found. Please install Visual Studio Build Tools with C++ workload first.
  exit /b 1
)

if not exist "%CARGO%" (
  echo cargo.exe not found. Please install Rust first.
  exit /b 1
)

call "%VSDEVCMD%" -arch=x64 -host_arch=x64
"%CARGO%" run --manifest-path "%~dp0src-tauri\Cargo.toml"
