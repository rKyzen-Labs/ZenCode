@echo off
setlocal

echo ========================================================
echo   Launching ZenCode CLI in Development Mode
echo ========================================================
cargo run -p zencode-cli -- %*
