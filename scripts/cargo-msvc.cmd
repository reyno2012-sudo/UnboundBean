@echo off
setlocal
call "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul
if errorlevel 1 exit /b %errorlevel%
pushd "%~dp0..\src-tauri"
"%USERPROFILE%\.cargo\bin\cargo.exe" %*
set "CARGO_EXIT=%ERRORLEVEL%"
popd
exit /b %CARGO_EXIT%
