param(
    [string]$Version,
    [string]$Sha256,
    [string]$FormulaSha256,
    [string]$ZipPath,
    [string]$TarGzPath,
    [string]$CaskPath = "Casks/clickless.rb",
    [string]$FormulaPath = "Formula/clickless.rb",
    [string]$TapDir
)

$ErrorActionPreference = 'Stop'

$origPwd = $PWD.Path
if ($TapDir -and -not [System.IO.Path]::IsPathRooted($TapDir)) {
    $TapDir = [System.IO.Path]::GetFullPath((Join-Path $origPwd $TapDir))
}
if ($ZipPath -and -not [System.IO.Path]::IsPathRooted($ZipPath)) {
    $ZipPath = [System.IO.Path]::GetFullPath((Join-Path $origPwd $ZipPath))
}
if ($TarGzPath -and -not [System.IO.Path]::IsPathRooted($TarGzPath)) {
    $TarGzPath = [System.IO.Path]::GetFullPath((Join-Path $origPwd $TarGzPath))
}
if ($CaskPath -and -not [System.IO.Path]::IsPathRooted($CaskPath) -and (Test-Path (Join-Path $origPwd $CaskPath))) {
    $CaskPath = [System.IO.Path]::GetFullPath((Join-Path $origPwd $CaskPath))
}
if ($FormulaPath -and -not [System.IO.Path]::IsPathRooted($FormulaPath) -and (Test-Path (Join-Path $origPwd $FormulaPath))) {
    $FormulaPath = [System.IO.Path]::GetFullPath((Join-Path $origPwd $FormulaPath))
}

Push-Location (Split-Path $PSScriptRoot -Parent)

try {
    if (-not $Version) {
        if (Test-Path Cargo.toml) {
            $content = Get-Content Cargo.toml -Raw
            if ($content -match '(?m)^version\s*=\s*"([^"]+)"') {
                $Version = $matches[1]
            }
        }
        if (-not $Version) {
            throw "Could not determine version from Cargo.toml. Please specify -Version."
        }
    }

    if (-not $ZipPath) {
        $ZipPath = "target/release/Clickless-macos.zip"
    }

    if (-not $Sha256) {
        $shaFile = "$ZipPath.sha256"
        if (Test-Path $shaFile) {
            $Sha256 = (Get-Content $shaFile -Raw).Trim().Split()[0]
        } elseif (Test-Path $ZipPath) {
            $Sha256 = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }

    if (-not $FormulaSha256 -and $TarGzPath -and (Test-Path $TarGzPath)) {
        $FormulaSha256 = (Get-FileHash -Path $TarGzPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    if (-not (Test-Path $CaskPath)) {
        throw "Cask file not found: $CaskPath"
    }

    Write-Host "Updating Homebrew Cask '$CaskPath' to version $Version..."
    $caskContent = Get-Content $CaskPath -Raw
    $caskContent = $caskContent -replace 'version "[^"]+"', "version `"$Version`""

    if ($Sha256) {
        Write-Host "Updating SHA256 in Cask: $Sha256"
        $caskContent = $caskContent -replace 'sha256 "[^"]+"', "sha256 `"$Sha256`""
    } else {
        Write-Warning "No SHA256 computed or provided. Retaining existing sha256."
    }
    Set-Content -Path $CaskPath -Value $caskContent -NoNewline

    if (Test-Path $FormulaPath) {
        Write-Host "Updating Homebrew Formula '$FormulaPath' version reference..."
        $formulaContent = Get-Content $FormulaPath -Raw
        $formulaContent = $formulaContent -replace '/archive/refs/tags/v[0-9.]+\.tar\.gz', "/archive/refs/tags/v$Version.tar.gz"
        if ($FormulaSha256) {
            Write-Host "Updating SHA256 in Formula: $FormulaSha256"
            $formulaContent = $formulaContent -replace 'sha256 "[^"]+"', "sha256 `"$FormulaSha256`""
        }
        Set-Content -Path $FormulaPath -Value $formulaContent -NoNewline
    }

    if ($TapDir) {
        Write-Host "Synchronizing formulas to Tap directory '$TapDir'..."
        $caskDestDir = Join-Path $TapDir "Casks"
        $formulaDestDir = Join-Path $TapDir "Formula"
        if (-not (Test-Path $caskDestDir)) { New-Item -ItemType Directory -Path $caskDestDir -Force | Out-Null }
        if (-not (Test-Path $formulaDestDir)) { New-Item -ItemType Directory -Path $formulaDestDir -Force | Out-Null }
        Copy-Item $CaskPath -Destination (Join-Path $caskDestDir "clickless.rb") -Force
        if (Test-Path $FormulaPath) {
            Copy-Item $FormulaPath -Destination (Join-Path $formulaDestDir "clickless.rb") -Force
        }
    }

    Write-Host "Homebrew update completed successfully."
} finally {
    Pop-Location
}
