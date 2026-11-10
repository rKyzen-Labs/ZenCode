@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically for x86_64-pc-windows-gnu
set "MINGW_BIN="
for /f "delims=" %%D in ('dir /b /ad "%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*" 2^>nul') do (
    if exist "%LOCALAPPDATA%\Microsoft\WinGet\Packages\%%D\mingw64\bin" (
        set "MINGW_BIN=%LOCALAPPDATA%\Microsoft\WinGet\Packages\%%D\mingw64\bin"
    )
)

if defined MINGW_BIN (
    set "PATH=!MINGW_BIN!;%PATH%"
)

echo ========================================================
echo   Launching Zencode Desktop in Development Mode
echo ========================================================
cd /d "%~dp0apps\desktop"
pnpm tauri dev
