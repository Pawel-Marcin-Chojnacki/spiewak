param([ValidateSet('ARM64','x64')][string]$Architecture='ARM64')
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rid=if($Architecture -eq 'ARM64'){'win-arm64'}else{'win-x64'}
$exe=Join-Path $root "windows\Player\bin\$Architecture\Release\net10.0-windows10.0.26100.0\$rid\Spiewak.exe"
if(!(Test-Path $exe)){throw 'Build the app with tools/build.ps1 first.'}
Start-Process $exe
