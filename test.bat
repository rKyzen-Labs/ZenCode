@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically
set "MINGW_BIN="
for /f "delims=" %%D in ('dir /b /ad "%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*" 2^>nul') do (
    if exist "%LOCALAPPDATA%\Microsoft\WinGet\Packages\%%D\mingw64\bin" (
        set "MINGW_BIN=%LOCALAPPDATA%\Microsoft\WinGet\Packages\%%D\mingw64\bin"
    )
)

if defined MINGW_BIN (
    set "PATH=!MINGW_BIN!;%PATH%"
)
set "PATH=%~dp0target\debug;%~dp0target\debug\deps;!PATH!"

echo ========================================================
echo   Compiling Zencode Workspace Test Suite
echo ========================================================
cargo test --no-run --workspace
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%

REM Ensure ComCtl6 SxS manifest is present for test executables
powershell -NoProfile -Command "$m = '%~dp0apps\desktop\src-tauri\comctl6.manifest'; if (Test-Path $m) { Get-ChildItem -Path '%~dp0target\debug\deps\zencode_desktop_lib-*.exe' -ErrorAction SilentlyContinue | ForEach-Object { Copy-Item $m -Destination ($_.FullName + '.manifest') -Force; $_.LastWriteTime = Get-Date } }" >nul 2>&1

echo ========================================================
echo   Running Zencode Workspace Test Suite
echo ========================================================
cargo test --workspace
