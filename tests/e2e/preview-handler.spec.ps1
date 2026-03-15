param(
  [string]$LogPath,
  [switch]$ClearLog,
  [switch]$AllowMissingUnload,
  [switch]$RequireShow,
  [int]$Tail = 120
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-PreviewLogPath {
  param([string]$RequestedPath)

  if ($RequestedPath) {
    return (Resolve-Path -Path $RequestedPath).Path
  }

  $candidates = @(
    (Join-Path $env:LOCALAPPDATA "Temp\Low\mdview-preview.log"),
    (Join-Path $env:LOCALAPPDATA "Temp\mdview-preview.log")
  )

  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }

  throw "Preview log not found. Checked: $($candidates -join ', ')"
}

function Assert-LogContains {
  param(
    [string[]]$Lines,
    [string]$Pattern,
    [string]$Message
  )

  if (-not ($Lines | Select-String -Pattern $Pattern -Quiet)) {
    throw $Message
  }
}

function Get-MatchCount {
  param(
    [string[]]$Lines,
    [string]$Pattern
  )

  return @($Lines | Select-String -Pattern $Pattern).Count
}

$resolvedLogPath = Resolve-PreviewLogPath -RequestedPath $LogPath

if ($ClearLog) {
  if (Test-Path $resolvedLogPath) {
    Clear-Content -Path $resolvedLogPath
    Write-Host "[mdview-preview-spec] Cleared log: $resolvedLogPath"
  } else {
    Write-Host "[mdview-preview-spec] Log did not exist yet: $resolvedLogPath"
  }
  exit 0
}

$lines = Get-Content -Path $resolvedLogPath
if ($lines.Count -eq 0) {
  throw "Preview log is empty: $resolvedLogPath"
}

$setWindowCount = Get-MatchCount -Lines $lines -Pattern "SetWindow"
$doPreviewCount = Get-MatchCount -Lines $lines -Pattern "DoPreview"
$setRectCount = Get-MatchCount -Lines $lines -Pattern "SetRect"
$threadChildCount = Get-MatchCount -Lines $lines -Pattern "thread: child="
$unloadCount = Get-MatchCount -Lines $lines -Pattern "^Unload$"
$immediateShowCount = Get-MatchCount -Lines $lines -Pattern "DoPreview .+do_show=True|DoPreview .+do_show=true"
$deferredSetRectCount = Get-MatchCount -Lines $lines -Pattern "SetRect .+do_show=True|SetRect .+do_show=true"

Assert-LogContains -Lines $lines -Pattern "SetWindow" -Message "Expected at least one SetWindow entry."
Assert-LogContains -Lines $lines -Pattern "DoPreview" -Message "Expected at least one DoPreview entry."

if (($setRectCount -eq 0) -and ($immediateShowCount -eq 0)) {
  throw "Expected either a SetRect entry or an immediate DoPreview show entry."
}

if ($RequireShow) {
  Assert-LogContains -Lines $lines -Pattern "thread: child=" -Message "Expected at least one preview child creation entry."
}

if (-not $AllowMissingUnload) {
  Assert-LogContains -Lines $lines -Pattern "^Unload$" -Message "Expected at least one Unload entry."
}

Write-Host "[mdview-preview-spec] Log path: $resolvedLogPath"
Write-Host "[mdview-preview-spec] Entries:"
Write-Host "  SetWindow      : $setWindowCount"
Write-Host "  DoPreview      : $doPreviewCount"
Write-Host "  SetRect        : $setRectCount"
Write-Host "  thread child   : $threadChildCount"
Write-Host "  Unload         : $unloadCount"
Write-Host "  immediate show : $immediateShowCount"
Write-Host "  deferred show  : $deferredSetRectCount"

if ($threadChildCount -eq 0) {
  Write-Warning "No child creation entries were found. This can happen if Explorer never reached a successful preview render."
}

if (($immediateShowCount -eq 0) -and ($deferredSetRectCount -eq 0)) {
  Write-Warning "No explicit show markers were found. Check whether the current run exercised first-open or file-switch paths."
}

Write-Host "[mdview-preview-spec] Tail:"
Get-Content -Path $resolvedLogPath -Tail $Tail | ForEach-Object {
  Write-Host "  $_"
}
