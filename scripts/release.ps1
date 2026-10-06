# Builds the Windows installer, uploads it and merges the windows entry into latest.json
# on the draft release. Run from a Windows PC after copying the signing key there.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Get-Content (Join-Path $env:USERPROFILE ".config\vee\sign.env") |
  Where-Object { $_ -match '^\s*([A-Z_]+)=(.*)$' } |
  ForEach-Object { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2].Trim().Trim('"', "'"), "Process") }

$repo = "bob-park/vee-app"
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "v$version"

yarn tauri build
if ($LASTEXITCODE) { throw "tauri build failed" }

$setup = Get-ChildItem "src-tauri/target/release/bundle/nsis/*-setup.exe" | Select-Object -First 1
$signature = (Get-Content "$($setup.FullName).sig" -Raw).Trim()

gh release view $tag -R $repo *> $null
if ($LASTEXITCODE) {
  gh release create $tag -R $repo --draft --title "Vee $tag" --notes "Vee $tag"
  if ($LASTEXITCODE) { throw "could not create release $tag" }
}
gh release upload $tag -R $repo --clobber $setup.FullName
if ($LASTEXITCODE) { throw "upload failed" }

$work = New-Item -ItemType Directory (Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid()))
$latestPath = Join-Path $work "latest.json"
gh release download $tag -R $repo -p latest.json -D $work 2>$null
if (Test-Path $latestPath) {
  # -DateKind String keeps pub_date exactly as written (needs PowerShell 7.5+).
  $latest = Get-Content $latestPath -Raw | ConvertFrom-Json -DateKind String
} else {
  $latest = [pscustomobject]@{
    version   = $version
    notes     = "Vee $tag"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [pscustomobject]@{}
  }
}
$entry = [pscustomobject]@{
  signature = $signature
  url       = "https://github.com/$repo/releases/download/$tag/$($setup.Name)"
}
$latest.platforms | Add-Member -NotePropertyName "windows-x86_64" -NotePropertyValue $entry -Force
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath -Encoding utf8NoBOM
gh release upload $tag -R $repo --clobber $latestPath
if ($LASTEXITCODE) { throw "latest.json upload failed" }

Remove-Item $work -Recurse -Force
Write-Host "Draft $tag updated with the Windows build."
Write-Host "Once macOS is uploaded too: gh release edit $tag -R $repo --draft=false"
