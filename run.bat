@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically
for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*") do (
    set "MINGW_BIN=%%D\mingw64\bin"
)

set "PATH=A:\ZenCode\target\debug;!MINGW_BIN!;%PATH%"

echo ========================================================
echo   Launching Zencode Desktop (Windows 11)
echo ========================================================
"A:\ZenCode\target\debug\zencode-desktop.exe"
