@echo off
setlocal DisableDelayedExpansion
cd /d "%~dp0"
if not exist "%~dp0friends-voice-server.exe" (
  echo Server executable not found. Extract the entire ZIP release first.
  pause
  exit /b 1
)
echo Friends Voice Server - Windows
set /p "ROOM_PASSWORD=Set a room password (at least 8 characters): "
if not defined ROOM_PASSWORD (
  echo Password is required.
  pause
  exit /b 1
)
echo.
echo Server listens on TCP port 45888.
echo Keep this window open while friends are connected.
echo.
"%~dp0friends-voice-server.exe" "%ROOM_PASSWORD%"
echo.
echo Server stopped.
pause
