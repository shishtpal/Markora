$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$cargoManifest = Get-Content -LiteralPath Cargo.toml -Raw
$version = [regex]::Match($cargoManifest, '(?m)^version = "([^"]+)"').Groups[1].Value
if (!$version) { throw 'Cargo.toml must declare a version' }
if ([Environment]::Is64BitProcess -eq $false -or [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -ne 'X64') {
    throw 'Release packaging currently supports x86_64 only'
}
$isWindowsHost = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)
if (!$isWindowsHost -and ![System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Linux)) {
    throw 'Release packaging currently supports Windows and Linux only'
}
$platform = if ($isWindowsHost) { 'windows-x86_64' } else { 'linux-x86_64' }
$binaryName = if ($isWindowsHost) { 'markora.exe' } else { 'markora' }
$downloadName = if ($isWindowsHost) { 'markora.exe' } else { 'markora' }
$packageName = "markora-$version-$platform"
$packageRoot = Join-Path $projectRoot "target/packages/$packageName"
New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
Copy-Item -LiteralPath "target/release/$binaryName" -Destination (Join-Path $packageRoot $downloadName)
foreach ($file in @('README.md', 'PUBLISHING.md', 'ARCHITECTURE.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'target/THIRD_PARTY_LICENSES.txt')) {
    Copy-Item -LiteralPath $file -Destination $packageRoot
}
Copy-Item -LiteralPath 'licenses' -Destination $packageRoot -Recurse -Force
New-Item -ItemType Directory -Path dist -Force | Out-Null
if ($isWindowsHost) {
    $archive = Join-Path $projectRoot "dist/$packageName.zip"
    Compress-Archive -LiteralPath $packageRoot -DestinationPath $archive -Force
} else {
    & chmod 755 (Join-Path $packageRoot $downloadName)
    if ($LASTEXITCODE -ne 0) { throw 'Could not mark the Linux binary executable' }
    $archive = Join-Path $projectRoot "dist/$packageName.tar.gz"
    & tar -czf $archive -C (Split-Path $packageRoot -Parent) $packageName
    if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
}
$digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
$checksum = "$digest  $(Split-Path $archive -Leaf)`n"
[System.IO.File]::WriteAllText("$archive.sha256", $checksum, [System.Text.UTF8Encoding]::new($false))
Write-Output "Packaged $archive"
