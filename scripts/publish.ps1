# Run after cargo login. Cargo waits for each upload to reach the registry.
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $packages = @(
        'clickless-core', 'clickless-backend-api', 'clickless-config', 'clickless-ui',
        'clickless-output-enigo', 'clickless-linux', 'clickless-macos',
        'clickless-windows', 'clickless'
    )
    $metadataJson = & cargo.exe metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package versions' }
    $metadata = $metadataJson | ConvertFrom-Json
    foreach ($package in $packages) {
        $version = ($metadata.packages | Where-Object name -eq $package).version
        if (-not $version) { throw "Package missing from workspace: $package" }
        $published = $null
        try {
            $published = Invoke-RestMethod "https://crates.io/api/v1/crates/$package/$version" -Headers @{
                'User-Agent' = 'clickless-publication-script'
            }
        } catch {
            if ([int]$_.Exception.Response.StatusCode -ne 404) { throw }
        }
        if ($published) {
            if ($published.version.yanked) { throw "Published version is yanked: $package $version" }
            Write-Host "Already published: $package $version. Skipping."
            continue
        }
        & cargo.exe publish -p $package --dry-run --locked
        if ($LASTEXITCODE -ne 0) { throw "Dry run failed: $package" }
        & cargo.exe publish -p $package --locked
        if ($LASTEXITCODE -ne 0) { throw "Publication failed: $package" }
    }
} finally {
    Pop-Location
}
