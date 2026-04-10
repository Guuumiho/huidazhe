$vsDevCmd = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"

if (-not (Test-Path $vsDevCmd)) {
  Write-Error "VsDevCmd.bat not found. Please install Visual Studio Build Tools with C++ workload first."
  exit 1
}

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

if (-not (Test-Path $cargo)) {
  Write-Error "cargo.exe not found. Please install Rust first."
  exit 1
}

cmd /c "call `"$vsDevCmd`" -arch=x64 -host_arch=x64 && `"$cargo`" run --manifest-path `"$PSScriptRoot\src-tauri\Cargo.toml`""
