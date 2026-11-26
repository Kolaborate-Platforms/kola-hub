# Installs the built installer for the current user, runs bench mode once, checks sizes and uninstalls.
# Budgets are from docs/BUILD_SPEC.md section 4. Windows PowerShell 5.1, no admin rights.
param(
    [Parameter(Mandatory = $true)][string]$InstallerDir,
    [Parameter(Mandatory = $true)][string]$Csv,
    [Parameter(Mandatory = $true)][int]$InstallerLimitMB
)

$ErrorActionPreference = 'Stop'
$InstalledLimitMB = 60

$setup = @(Get-ChildItem -Path $InstallerDir -Filter '*-setup.exe')
if ($setup.Count -ne 1) { throw "Expected one installer in $InstallerDir, found $($setup.Count)" }
$setupMB = $setup[0].Length / 1MB
Write-Output ('Installer: {0} {1:N1} MB (limit {2})' -f $setup[0].Name, $setupMB, $InstallerLimitMB)

$install = Start-Process -FilePath $setup[0].FullName -ArgumentList '/S' -Wait -PassThru
if ($install.ExitCode -ne 0) { throw "Install failed with code $($install.ExitCode)" }

$folder = Join-Path $env:LOCALAPPDATA 'Kola Hub'
$exe = Join-Path $folder 'kola-hub.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "App not found after install: $exe" }
if (-not (Test-Path -LiteralPath (Join-Path $folder 'kola-hub-mcp.exe'))) { throw 'The AI server was not installed' }
$installedMB = (Get-ChildItem -LiteralPath $folder -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
Write-Output ('Installed: {0:N1} MB (limit {1})' -f $installedMB, $InstalledLimitMB)

try {
    & (Join-Path $PSScriptRoot 'measure-shell.ps1') -Exe $exe -Csv $Csv -Runs 3
} finally {
    $uninstaller = Join-Path $folder 'uninstall.exe'
    Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait
}

# The uninstaller hands over to a copy of itself, so give it a moment to finish.
$deadline = (Get-Date).AddSeconds(60)
while ((Test-Path -LiteralPath $exe) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
if (Test-Path -LiteralPath $exe) { throw 'Uninstall left the app behind' }
# Size budgets are checked last, so an oversized build still shows whether it installs and runs.
if ($setupMB -gt $InstallerLimitMB) { throw 'Installer is over its size budget' }
if ($installedMB -gt $InstalledLimitMB) { throw 'Installed size is over its budget' }
Write-Output 'Smoke test passed'
