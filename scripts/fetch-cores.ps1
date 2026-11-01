# Downloads the latest Xray-core, sing-box and geoip/geosite into src-tauri/resources/cores,
# which the installer ships and the app copies to %LOCALAPPDATA%\V2RaySybau\cores on first run.
# Their license files and the exact versions / source links go to src-tauri/resources/licenses.
param(
    [string]$Destination = (Join-Path $PSScriptRoot '..\src-tauri\resources\cores'),
    [string]$Licenses = (Join-Path $PSScriptRoot '..\src-tauri\resources\licenses')
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$headers = @{ 'User-Agent' = 'v2raysybau-ci'; 'Accept' = 'application/vnd.github+json' }
if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $env:GITHUB_TOKEN" }

New-Item -ItemType Directory -Force -Path $Destination, $Licenses | Out-Null
$temp = Join-Path ([IO.Path]::GetTempPath()) ("cores-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $temp | Out-Null
$sources = [System.Collections.Generic.List[object]]::new()

function Get-LatestAsset([string]$repo, [scriptblock]$match) {
    $release = Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$repo/releases/latest"
    $asset = $release.assets | Where-Object $match | Select-Object -First 1
    if (-not $asset) { throw "No matching asset in $repo $($release.tag_name)" }
    Write-Host "$repo $($release.tag_name): $($asset.name)"
    $file = Join-Path $temp $asset.name
    Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $file
    return [pscustomobject]@{ File = $file; Tag = $release.tag_name }
}

# Records what is shipped and where its source code is (GPL components get their source
# archive attached to the release by release.yml).
function Add-Source([string]$name, [string]$repo, [string]$tag, [string]$license, [bool]$attachSource) {
    $sources.Add([pscustomobject]@{
        name = $name
        repo = $repo
        version = $tag
        license = $license
        source = "https://github.com/$repo/tree/$tag"
        archive = "https://github.com/$repo/archive/refs/tags/$tag.tar.gz"
        attachSource = $attachSource
    })
}

# The LICENSE shipped in a release archive, or the repository's LICENSE at that tag.
function Save-License([string]$dir, [string]$repo, [string]$tag, [string]$target) {
    $found = Get-ChildItem $dir -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING)(\.(txt|md))?$' } | Select-Object -First 1
    if ($found) {
        Copy-Item $found.FullName (Join-Path $Licenses $target) -Force
    } else {
        Invoke-WebRequest -Headers $headers -Uri "https://raw.githubusercontent.com/$repo/$tag/LICENSE" -OutFile (Join-Path $Licenses $target)
    }
}

# Xray-core (+ its geo files as a fallback)
$xray = Get-LatestAsset 'XTLS/Xray-core' { $_.name -eq 'Xray-windows-64.zip' }
$xrayDir = Join-Path $temp 'xray'
Expand-Archive -Path $xray.File -DestinationPath $xrayDir
Copy-Item (Join-Path $xrayDir 'xray.exe') $Destination -Force
Get-ChildItem $xrayDir -Filter '*.dat' | Copy-Item -Destination $Destination -Force
Get-ChildItem $xrayDir -Filter '*.dll' | Copy-Item -Destination $Destination -Force
Save-License $xrayDir 'XTLS/Xray-core' $xray.Tag 'Xray-core.LICENSE.txt'
Add-Source 'Xray-core' 'XTLS/Xray-core' $xray.Tag 'MPL-2.0' $false

# Wintun driver DLL for Xray's built-in TUN (sing-box embeds its own copy); its license always.
$wintunVersion = '0.14.1'
$wintunZip = Join-Path $temp 'wintun.zip'
Invoke-WebRequest -Uri "https://www.wintun.net/builds/wintun-$wintunVersion.zip" -OutFile $wintunZip
$wintunDir = Join-Path $temp 'wintun'
Expand-Archive -Path $wintunZip -DestinationPath $wintunDir
if (-not (Test-Path (Join-Path $Destination 'wintun.dll'))) {
    Copy-Item (Join-Path $wintunDir 'wintun\bin\amd64\wintun.dll') $Destination -Force
}
Copy-Item (Join-Path $wintunDir 'wintun\LICENSE.txt') (Join-Path $Licenses 'wintun.LICENSE.txt') -Force
$sources.Add([pscustomobject]@{
    name = 'Wintun'; repo = ''; version = $wintunVersion; license = 'Wintun Prebuilt Binaries License'
    source = 'https://git.zx2c4.com/wintun'; archive = "https://www.wintun.net/builds/wintun-$wintunVersion.zip"; attachSource = $false
})

# sing-box (TUN, per-app proxy, Hysteria2, TUIC)
$sing = Get-LatestAsset 'SagerNet/sing-box' { $_.name -like 'sing-box-*-windows-amd64.zip' -and $_.name -notlike '*legacy*' }
$singDir = Join-Path $temp 'sing-box'
Expand-Archive -Path $sing.File -DestinationPath $singDir
Get-ChildItem $singDir -Recurse -Include 'sing-box.exe', '*.dll' | Copy-Item -Destination $Destination -Force
Save-License $singDir 'SagerNet/sing-box' $sing.Tag 'sing-box.LICENSE.txt'
Add-Source 'sing-box' 'SagerNet/sing-box' $sing.Tag 'GPL-3.0-or-later' $true

# Richer routing lists (superset of v2fly: category-ru, category-ads-all, ...)
$rules = Invoke-RestMethod -Headers $headers -Uri 'https://api.github.com/repos/Loyalsoldier/v2ray-rules-dat/releases/latest'
foreach ($name in 'geoip.dat', 'geosite.dat') {
    Invoke-WebRequest -Headers $headers -Uri "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/download/$($rules.tag_name)/$name" -OutFile (Join-Path $Destination $name)
}
Add-Source 'geoip.dat / geosite.dat (v2ray-rules-dat)' 'Loyalsoldier/v2ray-rules-dat' $rules.tag_name 'GPL-3.0; GeoIP data CC BY-SA 4.0 (Loyalsoldier/geoip, MaxMind GeoLite2)' $true

# What was shipped: a readable list for users and a machine-readable one for release.yml.
$lines = @('V2Ray Sybau ships these third-party components unmodified.', 'See THIRD-PARTY-NOTICES.md for their licenses.', '')
foreach ($s in $sources) {
    $lines += "$($s.name) $($s.version)"
    $lines += "  License: $($s.license)"
    $lines += "  Source:  $($s.source)"
    if ($s.attachSource) { $lines += "  The complete source of this version is attached to the V2Ray Sybau release: https://github.com/Xtop1337/v2raySYBAU/releases" }
    $lines += ''
}
Set-Content -Path (Join-Path $Licenses 'SOURCES.txt') -Value $lines -Encoding utf8
ConvertTo-Json -InputObject $sources.ToArray() -Depth 3 | Set-Content -Path (Join-Path $Licenses 'sources.json') -Encoding utf8
Copy-Item (Join-Path $PSScriptRoot '..\THIRD-PARTY-NOTICES.md') $Licenses -Force

Remove-Item $temp -Recurse -Force
Get-ChildItem $Destination, $Licenses | Format-Table Name, Length
