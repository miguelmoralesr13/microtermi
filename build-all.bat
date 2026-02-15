@echo off
echo === Microtermi: compilando app + visor de coverage ===
cd /d "%~dp0"

echo.
echo [1/3] Compilando microtermi (release)...
cargo build --release -p microtermi-gui
if %ERRORLEVEL% neq 0 (
    echo Error al compilar la app.
    exit /b 1
)

echo.
echo [2/3] Compilando visor de coverage...
cd tools\coverage-viewer
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo Error al compilar el visor.
    cd /d "%~dp0"
    exit /b 1
)
cd /d "%~dp0"

echo.
echo [3/3] Copiando visor junto a microtermi.exe...
set "VIEWER_SRC=tools\coverage-viewer\target\release"
set "DEST=target\release"
if exist "%VIEWER_SRC%\microtermi-coverage-viewer.exe" (
    copy /Y "%VIEWER_SRC%\microtermi-coverage-viewer.exe" "%DEST%\"
    echo Copiado microtermi-coverage-viewer.exe
)
if exist "%VIEWER_SRC%\microtermi_coverage_viewer.exe" (
    copy /Y "%VIEWER_SRC%\microtermi_coverage_viewer.exe" "%DEST%\"
    echo Copiado microtermi_coverage_viewer.exe
)
if not exist "%VIEWER_SRC%\microtermi-coverage-viewer.exe" if not exist "%VIEWER_SRC%\microtermi_coverage_viewer.exe" (
    echo No se encontro el .exe del visor en %VIEWER_SRC%
    dir "%VIEWER_SRC%\*.exe"
)

echo.
echo === Listo ===
echo App: %DEST%\microtermi.exe
echo Visor: %DEST%\microtermi-coverage-viewer.exe (o microtermi_coverage_viewer.exe)
echo.
echo Ejecuta %DEST%\microtermi.exe y en Coverage usa "Ver aqui".
explorer /select,"%~dp0%DEST%\microtermi.exe" 2>nul || echo Abre la carpeta target\release
