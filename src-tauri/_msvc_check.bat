@echo off
setlocal
set "LOG=c:\Users\puroh\Downloads\Case Forge- Legal AI app\src-tauri\cargo-check-msvc.log"
echo === START %DATE% %TIME% === > "%LOG%"

call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >> "%LOG%" 2>&1
if errorlevel 1 (
  echo VsDevCmd FAILED >> "%LOG%"
  echo CARGO_EXIT=1 >> "%LOG%"
  exit /b 1
)

set "PATH=C:\Strawberry\perl\bin;C:\Strawberry\perl\site\bin;C:\Strawberry\c\bin;%PATH%"
echo PERL: >> "%LOG%"
where perl >> "%LOG%" 2>&1
echo LINK: >> "%LOG%"
where link >> "%LOG%" 2>&1
echo CL: >> "%LOG%"
where cl >> "%LOG%" 2>&1

cd /d "c:\Users\puroh\Downloads\Case Forge- Legal AI app\src-tauri"
echo === cargo check begin %TIME% === >> "%LOG%"
cargo check >> "%LOG%" 2>&1
set "EC=%ERRORLEVEL%"
echo === cargo check end %TIME% === >> "%LOG%"
echo CARGO_EXIT=%EC% >> "%LOG%"
exit /b %EC%
