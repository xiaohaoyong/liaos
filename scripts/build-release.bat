@echo off
set "PATH=C:\Users\Administrator\.cargo\bin;D:\App\nodejs;C:\Windows\System32;C:\Windows"
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
cd /d d:\Workspace\project\Liaos
call npm run tauri build
exit /b %ERRORLEVEL%
