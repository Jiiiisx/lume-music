param(
    [switch]$PurgeUserData
)

$ErrorActionPreference = "Stop"

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\LumeMusic"
$startMenuLink = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Lume Music.lnk"
$desktopLink = Join-Path ([Environment]::GetFolderPath("Desktop")) "Lume Music.lnk"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LumeMusic"

$running = Get-Process -Name "lume-music" -ErrorAction SilentlyContinue
if ($running) {
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
}

foreach ($link in @($startMenuLink, $desktopLink)) {
    if (Test-Path $link) {
        Remove-Item -Path $link -Force -ErrorAction SilentlyContinue
        Write-Host "Removed shortcut: $link"
    }
}

if (Test-Path $uninstallKey) {
    Remove-Item -Path $uninstallKey -Recurse -Force -ErrorAction SilentlyContinue
}

if (Test-Path $InstallDir) {
    Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Removed install directory: $InstallDir"
}

if ($PurgeUserData) {
    $localData = Join-Path $env:LOCALAPPDATA "LumeMusic"
    $roamingConfig = Join-Path $env:APPDATA "LumeMusic"
    foreach ($dir in @($localData, $roamingConfig)) {
        if (Test-Path $dir) {
            Remove-Item -Path $dir -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Removed user data: $dir"
        }
    }
}

Write-Host "Lume Music uninstalled cleanly." -ForegroundColor Green
