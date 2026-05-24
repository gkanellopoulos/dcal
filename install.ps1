$ErrorActionPreference = "Stop"

$Repo = "gkanellopoulos/dcal"
$InstallDir = if ($env:DCAL_INSTALL_DIR) { $env:DCAL_INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\dcal" }

$Tag = (Invoke-WebRequest -Uri "https://github.com/$Repo/releases/latest" -MaximumRedirection 0 -ErrorAction SilentlyContinue -UseBasicParsing).Headers.Location -replace ".*/tag/", ""

if (-not $Tag) {
    Write-Error "Could not determine latest release."
    exit 1
}

$FileName = "dcal-x86_64-pc-windows-msvc.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$FileName"

Write-Host "Installing dcal $Tag..."

$TempDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "dcal-install-$(Get-Random)")
$ZipPath = Join-Path $TempDir $FileName

try {
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
    Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force

    if (-not (Test-Path (Join-Path $TempDir "dcal.exe"))) {
        Write-Error "Binary not found in archive."
        exit 1
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Move-Item -Path (Join-Path $TempDir "dcal.exe") -Destination (Join-Path $InstallDir "dcal.exe") -Force

    # Add to user PATH if not already there
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        Write-Host "Added $InstallDir to your PATH."
        Write-Host "Restart your terminal for PATH changes to take effect."
    }

    Write-Host "Installed dcal $Tag to $InstallDir\dcal.exe"
    Write-Host "Run 'dcal init' to get started."
}
finally {
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
