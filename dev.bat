@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically for x86_64-pc-windows-gnu
for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*") do (
    set "MINGW_BIN=%%D\mingw64\bin"
)

set "PATH=!MINGW_BIN!;%PATH%"

echo ========================================================
echo   Launching Zencode Desktop in Development Mode
echo ========================================================
cd /d "%~dp0apps\desktop"
pnpm tauri dev
