@echo off
chcp 65001 >nul
rem Sad Launcher: local preview of the site at http://localhost:8080
cd /d "%~dp0site"
set PORT=8080

where python >nul 2>nul
if %errorlevel%==0 (
  echo Сайт: http://localhost:%PORT%  ^(закрой это окно, чтобы остановить^)
  start "" "http://localhost:%PORT%"
  python -m http.server %PORT% --bind 127.0.0.1
  goto :eof
)

where py >nul 2>nul
if %errorlevel%==0 (
  echo Сайт: http://localhost:%PORT%  ^(закрой это окно, чтобы остановить^)
  start "" "http://localhost:%PORT%"
  py -m http.server %PORT% --bind 127.0.0.1
  goto :eof
)

where npx >nul 2>nul
if %errorlevel%==0 (
  echo Сайт: http://localhost:%PORT%  ^(закрой это окно, чтобы остановить^)
  start "" "http://localhost:%PORT%"
  npx --yes serve -l %PORT% .
  goto :eof
)

echo Не нашёл Python или Node.js. Установи Python с python.org и запусти снова.
pause
