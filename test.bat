@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically
for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*") do (
    set "MINGW_BIN=%%D\mingw64\bin"
)

set "PATH=A:\ZenCode\target\debug\deps;!MINGW_BIN!;%PATH%"

echo ========================================================
echo   Running Zencode Workspace Test Suite
echo ========================================================
cargo test --workspace
