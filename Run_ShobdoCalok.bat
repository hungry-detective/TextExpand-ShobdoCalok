@echo off
setlocal
cd /d "%~dp0"

echo [Shobdo Calok] Checking for Python...
python --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Python not found. Please install Python 3.10+ to run this app.
    pause
    exit /b 1
)

echo [Shobdo Calok] Checking dependencies...
pip install -r requirements.txt --quiet --no-warn-script-location

echo [Shobdo Calok] Starting the application...
start pythonw main.py

exit /b 0
