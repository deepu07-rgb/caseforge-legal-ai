@echo off
REM CaseForge — wipe local vault so you can create a new password.
REM Closes CaseForge if running, then deletes encrypted vault data on this PC.

echo.
echo ============================================
echo  CaseForge local vault reset
echo ============================================
echo.
echo This deletes:
echo   %APPDATA%\app.caseforge.desktop\vaults
echo.
echo Case data stored there will be gone. No cloud recovery.
echo.

set /p CONFIRM=Type YES to continue: 
if /I not "%CONFIRM%"=="YES" (
  echo Cancelled.
  pause
  exit /b 1
)

taskkill /IM caseforge.exe /F >nul 2>&1
timeout /t 1 /nobreak >nul

if exist "%APPDATA%\app.caseforge.desktop\vaults" (
  rmdir /s /q "%APPDATA%\app.caseforge.desktop\vaults"
)

if exist "%APPDATA%\app.caseforge.desktop\vaults" (
  echo FAILED: vault folder still exists. Close CaseForge and try again.
  pause
  exit /b 1
)

echo.
echo OK — vault wiped. Restart CaseForge and create a new password.
echo.
pause
