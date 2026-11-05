@echo off
setlocal EnableDelayedExpansion

REM Resolve MinGW toolchain path dynamically
for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\*MSVCRT*") do (
    set "MINGW_BIN=%%D\mingw64\bin"
)

set "PATH=A:\ZenCode\target\debug\deps;!MINGW_BIN!;%PATH%"

echo ========================================================
echo   Compiling Zencode Workspace Test Suite
echo ========================================================
cargo test --no-run --workspace
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%

REM Ensure ComCtl6 SxS manifest is present for test executables
for %%F in ("A:\ZenCode\target\debug\deps\zencode_desktop_lib-*.exe") do (
    if not exist "%%F.manifest" (
        copy /y "A:\ZenCode\target\debug\deps\zencode_desktop_lib-5fb25bf9ac3659e0.exe.manifest" "%%F.manifest" >nul 2>&1
        powershell -Command "(Get-Item '%%F').LastWriteTime = (Get-Date)" >nul 2>&1
    )
)

echo ========================================================
echo   Running Zencode Workspace Test Suite
echo ========================================================
cargo test --workspace
