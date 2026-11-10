# Publish this clean snapshot to a fresh repository. Existing Git history and
# machine-local settings are deliberately absent. This script never touches Git.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectRoot
$version = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
if (!$version) { throw 'Cargo.toml must declare a version' }
$exportRoot = Join-Path $projectRoot ("target/source-export/markora-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $exportRoot -Force | Out-Null
foreach ($item in @('.github', '.gitignore', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'package.json', 'package-lock.json', 'README.md', 'ARCHITECTURE.md', 'DESIGN.md', 'PUBLISHING.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'licenses', 'src', 'tests', 'tools', 'assets')) {
    Copy-Item -LiteralPath $item -Destination $exportRoot -Recurse -Force
}
if (Get-ChildItem -LiteralPath (Join-Path $exportRoot 'assets/ui/fonts') -Filter 'segoe*.ttf') {
    throw 'Source export must not contain Windows system fonts'
}
New-Item -ItemType Directory -Path dist -Force | Out-Null
$archive = Join-Path $projectRoot "dist/markora-$version-source.zip"
# Include the hidden .github and .gitignore entries, which Compress-Archive skips.
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
$zip = [System.IO.Compression.ZipFile]::Open($archive, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($file in Get-ChildItem -LiteralPath $exportRoot -Recurse -File -Force) {
        $entryName = $file.FullName.Substring($exportRoot.Length + 1).Replace('\', '/')
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $entryName) | Out-Null
    }
} finally {
    $zip.Dispose()
}
Write-Output "Clean source snapshot: $archive"
