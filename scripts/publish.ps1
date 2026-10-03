# Run after cargo login. Cargo waits for each upload to reach the registry.
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $packages = @(
        'clickless-core', 'clickless-backend-api', 'clickless-config',
        'clickless-output-enigo', 'clickless-linux', 'clickless-macos',
        'clickless-windows', 'clickless'
    )
    foreach ($package in $packages) {
        & cargo.exe publish -p $package --dry-run --locked
        if ($LASTEXITCODE -ne 0) { throw "Dry run failed: $package" }
        & cargo.exe publish -p $package --locked
        if ($LASTEXITCODE -ne 0) { throw "Publication failed: $package" }
    }
} finally {
    Pop-Location
}
