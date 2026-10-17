# Sync Live2D runtime (PetL2D.dll + shader cache + Haru model) into <Target>\live2d
# Usage: powershell -File scripts\sync-live2d.ps1 -Target 'F:\软件\LingPet'
#        (no -Target => <repo>\pet-native\target\release)
param(
    [string]$Target
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$src = Join-Path $root 'live2d-render\build'
if (-not $Target) { $Target = Join-Path $root 'target\release' }
$dst = Join-Path $Target 'live2d'

if (-not (Test-Path -LiteralPath (Join-Path $src 'PetL2D.dll'))) {
    throw "PetL2D.dll not found in $src - build live2d-render first"
}

New-Item -ItemType Directory -Force -Path $dst | Out-Null
Copy-Item -LiteralPath (Join-Path $src 'PetL2D.dll') -Destination $dst -Force
Copy-Item -LiteralPath (Join-Path $src 'Live2DCubismCore.dll') -Destination $dst -Force
robocopy (Join-Path $src 'FrameworkShaders') (Join-Path $dst 'FrameworkShaders') /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
robocopy (Join-Path $src 'Resources') (Join-Path $dst 'Resources') /MIR /NFL /NDL /NJH /NJS /NP | Out-Null

$n = Get-ChildItem -LiteralPath $dst -Recurse -File | Measure-Object -Sum Length
Write-Output ("live2d -> {0}  ({1} files, {2:N1} MB)" -f $dst, $n.Count, ($n.Sum / 1MB))
