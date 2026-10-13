param([string]$TargetDir = 'target')

$ErrorActionPreference = 'Stop'

$root = $PSScriptRoot
$targetPath = if ([IO.Path]::IsPathRooted($TargetDir)) { $TargetDir } else { Join-Path $root $TargetDir }
$exe = Join-Path $targetPath 'release\kafka-message-tool.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw "Release executable not found. Run cargo build --release first."
}

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Path $dist -Force | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$bundle = Join-Path $dist "kafka-message-tool-$stamp"
New-Item -ItemType Directory -Path $bundle | Out-Null
Copy-Item -LiteralPath $exe -Destination $bundle
Copy-Item -LiteralPath (Join-Path $root 'services') -Destination $bundle -Recurse

$zip = "$bundle.zip"
Compress-Archive -LiteralPath $bundle -DestinationPath $zip
Write-Output $zip
