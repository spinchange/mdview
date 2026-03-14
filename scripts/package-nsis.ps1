param(
  [string]$Configuration = "release",
  [string]$Version = "",
  [switch]$SkipBuild,
  [switch]$StageOnly,
  [switch]$AllUsers
)

$ErrorActionPreference = "Stop"

if ($AllUsers) {
  throw "All-users installer mode is not implemented yet. The current installer path is per-user only."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$stagingRoot = Join-Path $repoRoot "dist\nsis\staging"
$outputRoot = Join-Path $repoRoot "dist\nsis"
$nsisScript = Join-Path $scriptDir "nsis\mdview-installer.nsi"

function Get-Version {
  param([string]$ConfiguredVersion)

  if ($ConfiguredVersion) {
    return $ConfiguredVersion
  }

  $tauriConfigPath = Join-Path $repoRoot "apps\viewer-shell\src-tauri\tauri.conf.json"
  $tauriConfig = Get-Content $tauriConfigPath | ConvertFrom-Json
  return [string]$tauriConfig.version
}

function Resolve-MakeNsis {
  $command = Get-Command makensis -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  $commonPaths = @(
    "C:\Program Files (x86)\NSIS\makensis.exe",
    "C:\Program Files\NSIS\makensis.exe"
  )

  foreach ($candidate in $commonPaths) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }

  return $null
}

function Invoke-Step {
  param(
    [string]$FilePath,
    [string[]]$Arguments,
    [string]$WorkingDirectory = $repoRoot
  )

  Write-Host "[mdview] > $FilePath $($Arguments -join ' ')"
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code ${LASTEXITCODE}: $FilePath $($Arguments -join ' ')"
  }
}

function Assert-FileExists {
  param([string]$Path, [string]$Label)

  if (!(Test-Path $Path)) {
    throw "$Label not found: $Path"
  }
}

$resolvedVersion = Get-Version -ConfiguredVersion $Version
$exePath = Join-Path $repoRoot "target\$Configuration\viewer-shell.exe"
$dllPath = Join-Path $repoRoot "target\$Configuration\win_preview_handler.dll"
$installerName = "mdview-$resolvedVersion-setup.exe"
$installerPath = Join-Path $outputRoot $installerName

if (!$SkipBuild) {
  Invoke-Step -FilePath "npm" -Arguments @("run", "build:web")
  Invoke-Step -FilePath "cargo" -Arguments @("build", "--package", "viewer-shell", "--$Configuration")
  Invoke-Step -FilePath "cargo" -Arguments @("build", "--package", "win-preview-handler", "--$Configuration")
}

Assert-FileExists -Path $exePath -Label "viewer-shell executable"
Assert-FileExists -Path $dllPath -Label "preview handler DLL"
Assert-FileExists -Path $nsisScript -Label "NSIS installer script"

if (Test-Path $stagingRoot) {
  Remove-Item -Recurse -Force $stagingRoot
}
New-Item -ItemType Directory -Force -Path $stagingRoot | Out-Null
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null

Copy-Item $exePath (Join-Path $stagingRoot "viewer-shell.exe")
Copy-Item $dllPath (Join-Path $stagingRoot "win_preview_handler.dll")

Write-Host "[mdview] Staged installer payload in $stagingRoot"

if ($StageOnly) {
  Write-Host "[mdview] StageOnly requested. Skipping NSIS compilation."
  exit 0
}

$makensis = Resolve-MakeNsis
if (!$makensis) {
  throw "NSIS was not found. Install NSIS and rerun this script, or use -StageOnly to prepare the payload only."
}

Invoke-Step -FilePath $makensis -Arguments @(
  "/DMDVIEW_SOURCE_DIR=$stagingRoot",
  "/DMDVIEW_OUTFILE=$installerPath",
  "/DMDVIEW_VERSION=$resolvedVersion",
  $nsisScript
)

Write-Host "[mdview] Installer created: $installerPath"
