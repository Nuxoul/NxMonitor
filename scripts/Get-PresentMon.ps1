[CmdletBinding()]
param([string]$Destination = (Join-Path $PSScriptRoot '..\tools'))
$ErrorActionPreference = 'Stop'
$version = '2.6.0'
$expected = 'B2A706BC6AD475749E3B7E3409263AA1E6906D45BDCF993F6DBC0F660188F1AF'
$url = "https://github.com/GameTechDev/PresentMon/releases/download/v$version/PresentMon-$version-x64.exe"
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$file = Join-Path $Destination "PresentMon-$version-x64.exe"
if (!(Test-Path -LiteralPath $file)) {
    $temporary = "$file.download"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $temporary
        if ((Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash -ne $expected) { throw 'PresentMon SHA-256 mismatch; refusing downloaded file.' }
        $signature = Get-AuthenticodeSignature -LiteralPath $temporary
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Intel Corporation') { throw 'PresentMon must have a valid Intel Authenticode signature.' }
        Move-Item -LiteralPath $temporary -Destination $file
    } finally { if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary } }
}
if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $expected) { throw 'Existing helper has unexpected SHA-256; not overwriting it.' }
$signature = Get-AuthenticodeSignature -LiteralPath $file
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Intel Corporation') { throw 'Invalid Intel signature on existing helper.' }
Write-Output ([IO.Path]::GetFullPath($file))
