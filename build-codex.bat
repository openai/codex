@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d C:\Users\balin\Documents\Github\craft-agents-codex\codex-rs
cargo build --release -p codex-cli
