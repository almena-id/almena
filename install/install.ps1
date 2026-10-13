# Installs the almena CLI on Windows from its GitHub release. Served at
# https://almena.id/install.ps1:
#
#   irm https://almena.id/install.ps1 | iex
#
# Only Windows on x86_64 has a release binary. The archive is checked against
# the release's SHA256SUMS and the binary's Authenticode signature before
# anything is copied. No administrator rights are needed: the binary goes to
# the user's own programs folder, which is added to the user's PATH.
#
# Settings, through the environment (set before running it):
#
#   ALMENA_INSTALL_VERSION   the version to install (e.g. 2026.10.1); empty: the latest
#   ALMENA_INSTALL_DIR       where almena.exe goes; empty: %LOCALAPPDATA%\Programs\almena
#
# It runs through Invoke-Expression in the caller's session, so it never calls
# `exit` (that would close their PowerShell) and keeps everything inside one
# script block, called on the last line: a download cut off halfway runs nothing.

& {
    Set-StrictMode -Version 3.0
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'

    $Repo = 'almena-id/almena'
    $Target = 'x86_64-pc-windows-msvc'

    function Say([string] $Message) {
        Write-Host "almena: $Message"
    }

    # Windows PowerShell 5.1 does not offer TLS 1.2 by default; GitHub requires it.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    # A 32-bit PowerShell on 64-bit Windows reports x86 here and the real
    # architecture in PROCESSOR_ARCHITEW6432.
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') {
        throw "almena: Windows on $arch has no binary; only x86_64 (AMD64) is supported"
    }

    # The latest release's version, from where /releases/latest redirects to
    # (no API call, so no rate limit).
    $version = "$env:ALMENA_INSTALL_VERSION".Trim() -replace '^v', ''
    if (-not $version) {
        $request = [Net.WebRequest]::Create("https://github.com/$Repo/releases/latest")
        $request.Method = 'HEAD'
        try {
            $response = $request.GetResponse()
        } catch {
            throw "almena: could not reach GitHub to find the latest release: $($_.Exception.Message)"
        }
        $url = $response.ResponseUri.AbsoluteUri
        $response.Close()
        if ($url -notmatch '/tag/v([^/]+)$') {
            throw "almena: $Repo has no published release yet"
        }
        $version = $Matches[1]
    }

    $name = "almena-$version-$Target"
    $base = "https://github.com/$Repo/releases/download/v$version"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("almena-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null

    try {
        Say "downloading almena $version for $Target"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$name.zip" -OutFile "$tmp\$name.zip"
        } catch {
            throw "almena: could not download $base/$name.zip (is $version a published version?)"
        }
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS"
        } catch {
            throw "almena: could not download $base/SHA256SUMS"
        }

        $expected = Get-Content "$tmp\SHA256SUMS" |
            ForEach-Object { $hash, $file = $_ -split '\s+', 2; if ($file -in "$name.zip", "*$name.zip") { $hash } } |
            Select-Object -First 1
        if (-not $expected) {
            throw "almena: SHA256SUMS has no line for $name.zip"
        }
        $actual = (Get-FileHash -Algorithm SHA256 "$tmp\$name.zip").Hash
        if ($actual -ne $expected) {
            throw "almena: checksum mismatch for $name.zip (expected $expected, got $($actual.ToLower()))"
        }

        Expand-Archive -Path "$tmp\$name.zip" -DestinationPath $tmp
        $exe = "$tmp\$name\almena.exe"
        if (-not (Test-Path $exe)) {
            throw 'almena: the archive has no almena.exe'
        }
        $signature = Get-AuthenticodeSignature $exe
        if ($signature.Status -ne 'Valid') {
            throw "almena: almena.exe has no valid Authenticode signature ($($signature.Status))"
        }

        $dir = if ($env:ALMENA_INSTALL_DIR) { $env:ALMENA_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\almena' }
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        try {
            Copy-Item -Force $exe (Join-Path $dir 'almena.exe')
        } catch {
            throw "almena: could not write $dir\almena.exe (is almena running?): $($_.Exception.Message)"
        }
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    Say "installed $(& (Join-Path $dir 'almena.exe') --version) at $dir\almena.exe"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @("$userPath" -split ';' | Where-Object { $_ })
    if ($entries -notcontains $dir) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $dir) -join ';'), 'User')
        Say "added $dir to your PATH; open a new terminal elsewhere to run almena by name"
    }
    if (@($env:Path -split ';') -notcontains $dir) {
        $env:Path = "$env:Path;$dir"
    }
    Say 'next: almena auth login'
}
