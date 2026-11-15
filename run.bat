@echo off
setlocal

cargo run --release -p zencode-cli -- %*
