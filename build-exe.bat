@echo off
echo Construyendo Microtermi (release)...
cargo build --release -p microtermi-gui
if %ERRORLEVEL% neq 0 exit /b %ERRORLEVEL%
echo.
echo Listo. Ejecutable: target\release\microtermi.exe
explorer /select,"%~dp0target\release\microtermi.exe" 2>nul || echo Abre target\release\ para ver microtermi.exe
