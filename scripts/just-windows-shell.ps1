if ($args.Count -eq 0) {
    [Console]::Error.WriteLine("Windows just shell adapter expected a recipe command.")
    exit 1
}

$Command = $args[0]
$ForwardedArgs = @($args | Select-Object -Skip 1)

$pwsh = Get-Command pwsh.exe -ErrorAction SilentlyContinue
if (-not $pwsh) {
    [Console]::Error.WriteLine("PowerShell 7.4+ ('pwsh') is required for Windows just recipes. Run 'just install' to install it.")
    exit 1
}

$version = & $pwsh.Source -NoLogo -NoProfile -Command '$PSVersionTable.PSVersion.ToString()'
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

if ([version] $version -lt [version] "7.4") {
    [Console]::Error.WriteLine("PowerShell 7.4+ ('pwsh') is required for Windows just recipes. Run 'just install' to update it.")
    exit 1
}

& $pwsh.Source -NoLogo -NoProfile -CommandWithArgs $Command @ForwardedArgs
exit $LASTEXITCODE
