<#
.SYNOPSIS
  Builds and publishes a StoneAge 2 release from this PC in one step (no GitHub Actions needed).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Version 2.0.1 -Notes "Fixes ..."

  1. sets the version in Cargo.toml (Cargo.lock follows)
  2. runs the tests
  3. builds x86 + x64 release binaries WITHOUT the original StoneAge assets
     (assets/original is hidden during the build and always restored)
  4. checks the binaries contain no original text
  5. commits Cargo.toml/Cargo.lock, tags vX.Y.Z, pushes main + tag
  6. creates the GitHub release with StoneAge2-x86.exe, StoneAge2-x64.exe,
     stoneage-windows-x86.zip, stoneage-windows-x64.zip and SHA256SUMS.txt
     (the zips + checksums are what the in-app updater downloads)
#>
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$')][string]$Version,
    [string]$Notes = "",
    [string]$Repo = "Tamer-Mansour/stoneage2"
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:CARGO_TARGET_DIR = Join-Path $root 'target\integrator'
$tag = "v$Version"

function Run([string]$exe, [string[]]$a) {
    & $exe @a
    if ($LASTEXITCODE -ne 0) { throw "$exe $($a -join ' ') failed ($LASTEXITCODE)" }
}

if ((git branch --show-current) -ne 'main') { throw "not on main" }
if (git tag --list $tag) { throw "tag $tag already exists" }
$dirty = git status --porcelain --untracked-files=no
if ($dirty) { throw "commit or stash your changes first:`n$dirty" }

# 1. version
$toml = Get-Content Cargo.toml -Raw
$toml = [regex]::Replace($toml, '(?m)(^\[workspace\.package\][^\[]*?^version\s*=\s*")[^"]+(")', "`${1}$Version`${2}")
[IO.File]::WriteAllText((Join-Path $root 'Cargo.toml'), $toml)

# 2. tests (also updates Cargo.lock)
Run cargo @('test', '--workspace', '--quiet')

# 3. public release builds
$orig = 'crates\stoneage-core\assets\original'
$hidden = 'crates\stoneage-core\assets\_hidden_for_release'
$env:RUSTFLAGS = '-C target-feature=+crt-static'
$targets = @(@('i686-pc-windows-msvc', 'x86'), @('x86_64-pc-windows-msvc', 'x64'))
if (Test-Path $orig) { Rename-Item $orig (Split-Path $hidden -Leaf) }
try {
    foreach ($t in $targets) { Run cargo @('build', '--release', '--locked', '-p', 'stoneage-app', '--target', $t[0]) }
} finally {
    if (Test-Path $hidden) { Rename-Item $hidden (Split-Path $orig -Leaf) }
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
}

# 4. no original text in the binaries
foreach ($t in $targets) {
    $exe = Join-Path $env:CARGO_TARGET_DIR "$($t[0])\release\stoneage.exe"
    $s = [Text.Encoding]::GetEncoding(1252).GetString([IO.File]::ReadAllBytes($exe))
    foreach ($bad in @('K-S Analysis', 'Lizenzvereinbarung', 'Waltershausen')) {
        if ($s.Contains($bad)) { throw "$exe contains original text '$bad' - not publishing" }
    }
}

# package
$dist = Join-Path $env:CARGO_TARGET_DIR 'dist'
Remove-Item -Recurse -Force $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory $dist | Out-Null
foreach ($t in $targets) {
    $exe = Join-Path $env:CARGO_TARGET_DIR "$($t[0])\release\stoneage.exe"
    $name = "stoneage-windows-$($t[1])"; $dir = Join-Path $dist $name
    New-Item -ItemType Directory -Force (Join-Path $dir 'licenses') | Out-Null
    Copy-Item $exe $dir
    Copy-Item README.md $dir
    Copy-Item crates\stoneage-app\assets\icons\LICENSE-tlp-icons.txt, crates\stoneage-app\assets\icons\NOTICE-tlp-icons.md (Join-Path $dir 'licenses')
    Compress-Archive -Path (Join-Path $dir '*') -DestinationPath (Join-Path $dist "$name.zip") -CompressionLevel Optimal
    Copy-Item $exe (Join-Path $dist "StoneAge2-$($t[1]).exe")
    Remove-Item -Recurse -Force $dir
}
$sums = Get-ChildItem $dist -Filter 'stoneage-windows-*.zip' | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name
}
[IO.File]::WriteAllText((Join-Path $dist 'SHA256SUMS.txt'), (($sums -join "`n") + "`n"))

# 5. commit + tag + push (only the version files)
Run git @('add', 'Cargo.toml', 'Cargo.lock')
$staged = git diff --staged --name-only
if ($staged) { Run git @('commit', '-q', '-m', "Release $tag") }
Run git @('tag', $tag)
Run git @('push', '-q', 'origin', 'main')
Run git @('push', '-q', 'origin', $tag)

# 6. GitHub release
if (-not $Notes) { $Notes = "StoneAge 2 $Version" }
$Notes += "`n`n**Downloads:** StoneAge2-x86.exe for the PCSGU250 oscilloscope (32-bit driver), StoneAge2-x64.exe without the device. Installed copies update themselves (Updates section)."
$files = @('StoneAge2-x86.exe', 'StoneAge2-x64.exe', 'stoneage-windows-x86.zip', 'stoneage-windows-x64.zip', 'SHA256SUMS.txt') | ForEach-Object { Join-Path $dist $_ }
$pre = @(); if ($Version.Contains('-')) { $pre = @('--prerelease') }
Run gh (@('release', 'create', $tag, '-R', $Repo, '--verify-tag', '--title', "StoneAge $Version", '--notes', $Notes) + $pre + $files)
Write-Host "Released $tag : https://github.com/$Repo/releases/tag/$tag"
