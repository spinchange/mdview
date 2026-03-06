param(
  [string]$DllPath = "$(Resolve-Path "$PSScriptRoot\..\target\debug\win_preview_handler.dll")",
  [switch]$Unregister
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$clsid = "{4F831CA2-0DB6-4F14-A4F2-8AB7DE6F6601}"
$progId = "mdview.PreviewHandler"
$previewHandlerKey = "{8895B1C6-B41F-4C1C-A562-0D564250836F}"
$base = "HKCU:\Software\Classes"

function Ensure-Key {
  param([string]$Path)
  if (!(Test-Path $Path)) {
    New-Item -Path $Path -Force | Out-Null
  }
}

function Register-PreviewHandler {
  param([string]$ResolvedDllPath)

  Write-Host "[mdview] Registering preview handler from: $ResolvedDllPath"

  $clsidRoot = "$base\CLSID\$clsid"
  $inprocRoot = "$clsidRoot\InprocServer32"
  $progIdRoot = "$base\$progId"
  $progIdClsidRoot = "$progIdRoot\CLSID"

  Ensure-Key $clsidRoot
  Ensure-Key $inprocRoot
  Ensure-Key $progIdRoot
  Ensure-Key $progIdClsidRoot

  Set-ItemProperty -Path $clsidRoot -Name "(default)" -Value "mdview Markdown Preview Handler"
  Set-ItemProperty -Path $clsidRoot -Name "ProgID" -Value $progId
  Set-ItemProperty -Path $inprocRoot -Name "(default)" -Value $ResolvedDllPath
  Set-ItemProperty -Path $inprocRoot -Name "ThreadingModel" -Value "Apartment"
  Set-ItemProperty -Path $progIdRoot -Name "(default)" -Value "mdview Markdown Preview Handler"
  Set-ItemProperty -Path $progIdClsidRoot -Name "(default)" -Value $clsid

  $previewHandlersRoot = "$base\PreviewHandlers"
  Ensure-Key $previewHandlersRoot
  Set-ItemProperty -Path $previewHandlersRoot -Name $clsid -Value "mdview Markdown Preview Handler"

  $extensions = @(".md", ".markdown")
  foreach ($ext in $extensions) {
    $shellExRoot = "$base\$ext\shellex\$previewHandlerKey"
    Ensure-Key $shellExRoot
    Set-ItemProperty -Path $shellExRoot -Name "(default)" -Value $clsid
  }

  Write-Host "[mdview] Registration complete."
}

function Unregister-PreviewHandler {
  Write-Host "[mdview] Removing preview handler registration"

  $paths = @(
    "$base\CLSID\$clsid\InprocServer32",
    "$base\CLSID\$clsid",
    "$base\$progId\CLSID",
    "$base\$progId",
    "$base\.md\shellex\$previewHandlerKey",
    "$base\.markdown\shellex\$previewHandlerKey"
  )

  foreach ($path in $paths) {
    if (Test-Path $path) {
      Remove-Item -Path $path -Recurse -Force
    }
  }

  $previewHandlersRoot = "$base\PreviewHandlers"
  if (Test-Path $previewHandlersRoot) {
    Remove-ItemProperty -Path $previewHandlersRoot -Name $clsid -ErrorAction SilentlyContinue
  }

  Write-Host "[mdview] Unregistration complete."
}

if ($Unregister) {
  Unregister-PreviewHandler
  exit 0
}

$resolvedDll = (Resolve-Path $DllPath).Path
if (!(Test-Path $resolvedDll)) {
  throw "Preview handler DLL not found: $resolvedDll"
}

Register-PreviewHandler -ResolvedDllPath $resolvedDll
