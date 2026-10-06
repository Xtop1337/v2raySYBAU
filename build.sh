#!/bin/bash
set -e

echo "🔨 Building V2Ray Sybau for Windows..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Clean previous builds
rm -rf bin/ obj/

# Restore dependencies
echo "📦 Restoring packages..."
dotnet restore V2RaySybau.sln

# Build Release
echo "🔨 Building Release (x64)..."
dotnet build V2RaySybau.sln -c Release --no-restore

# Publish self-contained (optional, for standalone exe)
echo "📤 Publishing Release build..."
dotnet publish src/V2RaySybau/V2RaySybau.csproj -c Release -r win-x64 --self-contained false -o bin/Release/publish

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Build complete!"
echo ""
echo "📍 Executable locations:"
echo "   • Debug:   bin/Debug/net8.0-windows/V2RaySybau.exe"
echo "   • Release: bin/Release/publish/V2RaySybau.exe"
echo ""
echo "📋 Next steps:"
echo "   1. Download xray.exe from https://github.com/XTLS/Xray-core/releases"
echo "   2. Place xray.exe next to V2RaySybau.exe"
echo "   3. Run V2RaySybau.exe"
echo ""
