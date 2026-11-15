@echo off
setlocal
call "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 exit /b %errorlevel%
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
pushd "%~dp0.."
call npm.cmd run tauri -- %*
set "TAURI_EXIT=%ERRORLEVEL%"
popd
exit /b %TAURI_EXIT%
