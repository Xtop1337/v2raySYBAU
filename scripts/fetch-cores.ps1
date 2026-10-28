# Downloads the latest Xray-core, sing-box and geoip/geosite into src-tauri/resources/cores,
# which the installer ships and the app copies to %LOCALAPPDATA%\V2RaySybau\cores on first run.
param([string]$Destination = (Join-Path $PSScriptRoot '..\src-tauri\resources\cores'))

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$headers = @{ 'User-Agent' = 'v2raysybau-ci'; 'Accept' = 'application/vnd.github+json' }
if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $env:GITHUB_TOKEN" }

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$temp = Join-Path ([IO.Path]::GetTempPath()) ("cores-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $temp | Out-Null

function Get-LatestAsset([string]$repo, [scriptblock]$match) {
    $release = Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$repo/releases/latest"
    $asset = $release.assets | Where-Object $match | Select-Object -First 1
    if (-not $asset) { throw "No matching asset in $repo $($release.tag_name)" }
    Write-Host "$repo $($release.tag_name): $($asset.name)"
    $file = Join-Path $temp $asset.name
    Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $file
    return $file
}

# Xray-core (+ its geo files as a fallback)
$xrayZip = Get-LatestAsset 'XTLS/Xray-core' { $_.name -eq 'Xray-windows-64.zip' }
$xrayDir = Join-Path $temp 'xray'
Expand-Archive -Path $xrayZip -DestinationPath $xrayDir
Copy-Item (Join-Path $xrayDir 'xray.exe') $Destination -Force
Get-ChildItem $xrayDir -Filter '*.dat' | Copy-Item -Destination $Destination -Force

# sing-box (TUN, per-app proxy, Hysteria2, TUIC)
$singZip = Get-LatestAsset 'SagerNet/sing-box' { $_.name -like 'sing-box-*-windows-amd64.zip' -and $_.name -notlike '*legacy*' }
$singDir = Join-Path $temp 'sing-box'
Expand-Archive -Path $singZip -DestinationPath $singDir
Get-ChildItem $singDir -Recurse -Include 'sing-box.exe', '*.dll' | Copy-Item -Destination $Destination -Force

# Richer routing lists (superset of v2fly: category-ru, category-ads-all, ...)
foreach ($name in 'geoip.dat', 'geosite.dat') {
    Invoke-WebRequest -Headers $headers -Uri "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/$name" -OutFile (Join-Path $Destination $name)
}

Remove-Item $temp -Recurse -Force
Get-ChildItem $Destination | Format-Table Name, Length
