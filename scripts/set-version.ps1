# Sets the application version everywhere from a tag such as v1.2.3.
param([Parameter(Mandatory = $true)][string]$Tag)

$ErrorActionPreference = 'Stop'
$version = $Tag.TrimStart('v', 'V')
if ($version -notmatch '^\d+\.\d+\.\d+([-+].+)?$') { throw "Tag '$Tag' is not a semantic version" }
$root = Join-Path $PSScriptRoot '..'

$conf = Join-Path $root 'src-tauri\tauri.conf.json'
(Get-Content $conf -Raw) -replace '"version":\s*"[^"]+"', "`"version`": `"$version`"" | Set-Content $conf -NoNewline

$cargo = Join-Path $root 'Cargo.toml'
(Get-Content $cargo -Raw) -replace '(?m)^version = "[^"]+"', "version = `"$version`"" | Set-Content $cargo -NoNewline

$package = Join-Path $root 'package.json'
(Get-Content $package -Raw) -replace '"version":\s*"[^"]+"', "`"version`": `"$version`"" | Set-Content $package -NoNewline

Write-Host "Version set to $version"
