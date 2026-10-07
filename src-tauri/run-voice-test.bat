@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set PATH=%USERPROFILE%\.cargo\bin;C:\Strawberry\perl\bin;%PATH%
cd /d "c:\Users\puroh\Downloads\Case Forge- Legal AI app\src-tauri"
echo START > cargo-test-voice-bat.txt
cargo test voice_court --lib --color never >> cargo-test-voice-bat.txt 2>&1
echo EXIT=%ERRORLEVEL%>> cargo-test-voice-bat.txt