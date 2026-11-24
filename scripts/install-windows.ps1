param(
    [switch]$Build,
    [switch]$NoDesktopShortcut,
    [string]$Version = "1.0.0"
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ReleaseExe = Join-Path $RepoRoot "target\release\lume-music.exe"
$SourceIco = Join-Path $RepoRoot "crates\lume-ui\assets\icons\lume-music.ico"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\LumeMusic"
$InstalledExe = Join-Path $InstallDir "lume-music.exe"
$InstalledIco = Join-Path $InstallDir "lume-music.ico"
$InstalledUninstall = Join-Path $InstallDir "uninstall.ps1"

if ($Build -or -not (Test-Path $ReleaseExe)) {
    Write-Host "Building Lume Music release binary..." -ForegroundColor Cyan
    Push-Location $RepoRoot
    try {
        cargo build --release -p lume-ui
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $ReleaseExe)) {
    throw "Release binary not found at: $ReleaseExe"
}

# Close any running instance so the binary can be updated cleanly.
$running = Get-Process -Name "lume-music" -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "Closing running Lume Music instance..." -ForegroundColor Yellow
    $running | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 600
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Path $ReleaseExe -Destination $InstalledExe -Force
if (Test-Path $SourceIco) {
    Copy-Item -Path $SourceIco -Destination $InstalledIco -Force
}
Copy-Item -Path (Join-Path $PSScriptRoot "uninstall-windows.ps1") -Destination $InstalledUninstall -Force

$iconTarget = if (Test-Path $InstalledIco) { "$InstalledIco,0" } else { "$InstalledExe,0" }
$wsh = New-Object -ComObject WScript.Shell

function New-AppShortcut {
    param([string]$LinkPath)
    $sc = $wsh.CreateShortcut($LinkPath)
    $sc.TargetPath = $InstalledExe
    $sc.WorkingDirectory = $InstallDir
    $sc.IconLocation = $iconTarget
    $sc.Description = "Lume Music — Native Rust Desktop Music Player"
    $sc.Save()
}

$startMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$startMenuLink = Join-Path $startMenuDir "Lume Music.lnk"
New-AppShortcut -LinkPath $startMenuLink
Write-Host "Created Start Menu shortcut: $startMenuLink" -ForegroundColor Green

if (-not $NoDesktopShortcut) {
    $desktopDir = [Environment]::GetFolderPath("Desktop")
    $desktopLink = Join-Path $desktopDir "Lume Music.lnk"
    New-AppShortcut -LinkPath $desktopLink
    Write-Host "Created Desktop shortcut:    $desktopLink" -ForegroundColor Green
}

# Register in HKCU Uninstall so Lume Music appears in Windows Settings -> Installed Apps.
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LumeMusic"
New-Item -Path $uninstallKey -Force | Out-Null
$sizeKb = [int][Math]::Ceiling((Get-Item $InstalledExe).Length / 1KB)
Set-ItemProperty -Path $uninstallKey -Name "DisplayName" -Value "Lume Music"
Set-ItemProperty -Path $uninstallKey -Name "DisplayVersion" -Value $Version
Set-ItemProperty -Path $uninstallKey -Name "Publisher" -Value "Lume Music"
Set-ItemProperty -Path $uninstallKey -Name "InstallLocation" -Value $InstallDir
Set-ItemProperty -Path $uninstallKey -Name "DisplayIcon" -Value $(if (Test-Path $InstalledIco) { $InstalledIco } else { $InstalledExe })
Set-ItemProperty -Path $uninstallKey -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$InstalledUninstall`""
Set-ItemProperty -Path $uninstallKey -Name "NoModify" -Type DWord -Value 1
Set-ItemProperty -Path $uninstallKey -Name "NoRepair" -Type DWord -Value 1
Set-ItemProperty -Path $uninstallKey -Name "EstimatedSize" -Type DWord -Value $sizeKb

# Notify Windows Shell to refresh shortcut/app icons immediately.
try {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class ShellNotify {
    [DllImport("shell32.dll")]
    public static extern void SHChangeNotify(uint wEventId, uint uFlags, IntPtr dwItem1, IntPtr dwItem2);
}
"@ -ErrorAction SilentlyContinue
    [ShellNotify]::SHChangeNotify(0x08000000, 0x0000, [IntPtr]::Zero, [IntPtr]::Zero)
} catch {}

Write-Host ""
Write-Host "Lume Music v$Version installed to $InstallDir" -ForegroundColor Cyan
Write-Host "You can now launch it from Start Menu ('Lume Music'), Desktop, or Pin to Taskbar." -ForegroundColor Cyan
