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
set "PATH=%~dp0target\debug;!PATH!"

echo ========================================================
echo   Launching Zencode Desktop (Windows 11)
echo ========================================================
if not exist "%~dp0target\debug\zencode-desktop.exe" (
    echo Building Zencode Desktop...
    cargo build -p zencode-desktop
)
"%~dp0target\debug\zencode-desktop.exe"
