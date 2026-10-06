@echo off
echo.
echo ========================================
echo  V2Ray Sybau - Windows Build
echo ========================================
echo.

setlocal enabledelayedexpansion

REM Check if dotnet is installed
dotnet --version >nul 2>&1
if errorlevel 1 (
    echo ERROR: .NET SDK not found. Please install .NET 8 SDK from https://dotnet.microsoft.com/download/dotnet/8.0
    pause
    exit /b 1
)

REM Clean previous builds
echo [1/4] Cleaning previous builds...
if exist bin rmdir /s /q bin
if exist obj rmdir /s /q obj

REM Restore dependencies
echo [2/4] Restoring NuGet packages...
dotnet restore V2RaySybau.sln
if errorlevel 1 (
    echo ERROR: Failed to restore packages
    pause
    exit /b 1
)

REM Build Release
echo [3/4] Building Release configuration...
dotnet build V2RaySybau.sln -c Release --no-restore
if errorlevel 1 (
    echo ERROR: Build failed
    pause
    exit /b 1
)

REM Publish
echo [4/4] Publishing Release build...
dotnet publish src/V2RaySybau/V2RaySybau.csproj -c Release -r win-x64 --self-contained false -o bin/Release/publish
if errorlevel 1 (
    echo ERROR: Publish failed
    pause
    exit /b 1
)

echo.
echo ========================================
echo  BUILD SUCCESSFUL!
echo ========================================
echo.
echo Executable location:
echo   bin/Release/publish/V2RaySybau.exe
echo.
echo Next steps:
echo   1. Download xray.exe from:
echo      https://github.com/XTLS/Xray-core/releases
echo   2. Place xray.exe in:
echo      bin/Release/publish/xray.exe
echo   3. Run: bin/Release/publish/V2RaySybau.exe
echo.
pause
