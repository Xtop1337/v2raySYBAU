#!/bin/bash
set -e

echo "========================================"
echo " V2Ray Sybau - Release Preparation"
echo "========================================"
echo ""

# Check version
VERSION="1.0.0"
RELEASE_DIR="dist/v2raySybau-${VERSION}"

echo "[1/5] Cleaning previous builds..."
rm -rf bin/ obj/ dist/

echo "[2/5] Restoring packages..."
dotnet restore V2RaySybau.sln

echo "[3/5] Building Release..."
dotnet build V2RaySybau.sln -c Release --no-restore

echo "[4/5] Publishing..."
dotnet publish src/V2RaySybau/V2RaySybau.csproj -c Release -r win-x64 --self-contained false -o "$RELEASE_DIR"

echo "[5/5] Creating distribution archive..."
mkdir -p dist
cd dist
zip -r "v2raySybau-${VERSION}.zip" "v2raySybau-${VERSION}/" -x "*.pdb"
cd ..

echo ""
echo "========================================"
echo " READY FOR RELEASE!"
echo "========================================"
echo ""
echo "Release package: dist/v2raySybau-${VERSION}.zip"
echo ""
echo "Instructions for users:"
echo "  1. Extract the ZIP"
echo "  2. Download xray.exe and place in the extracted folder"
echo "  3. Run V2RaySybau.exe"
echo ""
