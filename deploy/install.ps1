# =============================================================================
#  Open-TmuxTabs - install / update and start
#
#  Usage (PowerShell, no admin rights required):
#    irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/install.ps1 | iex
#
#  Downloads OpenTmuxTabs.exe from the latest GitHub release to
#  %LOCALAPPDATA%\Programs\OpenTmuxTabs, verifies the SHA-256 checksum and
#  adds a Start menu entry. A running Open-TmuxTabs is stopped first. Finally
#  it asks whether to start it; if yes, it starts with its settings dialog open.
#  Optional: $env:OPENTMUXTABS_VERSION = 'v1.0.0' selects a specific version.
# =============================================================================
& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference    = 'SilentlyContinue'   # speeds up Invoke-WebRequest in PS 5.1
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'schlingensiepen/tmux-helper-rs'
    $tag  = $env:OPENTMUXTABS_VERSION
    $base = if ($tag) { "https://github.com/$repo/releases/download/$tag" } else { "https://github.com/$repo/releases/latest/download" }
    $dir  = Join-Path $env:LOCALAPPDATA 'Programs\OpenTmuxTabs'
    $exe  = Join-Path $dir 'OpenTmuxTabs.exe'
    $tmp  = "$exe.download"

    Write-Host "Open-TmuxTabs: downloading $base/OpenTmuxTabs.exe ..."
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri "$base/OpenTmuxTabs.exe" -OutFile $tmp
    $expected = (Invoke-WebRequest -UseBasicParsing -Uri "$base/OpenTmuxTabs.exe.sha256").Content
    if ($expected -is [byte[]]) { $expected = [Text.Encoding]::ASCII.GetString($expected) }
    $expected = ($expected -split '\s+')[0].Trim().ToLowerInvariant()
    $actual   = (Get-FileHash -Algorithm SHA256 -Path $tmp).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        Remove-Item -Force $tmp
        throw "Checksum mismatch (expected $expected, got $actual). Aborting."
    }

    Get-Process -Name 'OpenTmuxTabs' -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300
    Move-Item -Force -Path $tmp -Destination $exe
    Unblock-File -Path $exe

    # Report the hotkey actually configured (an existing OpenTmuxTabs.ini may override Ctrl+Alt+T).
    $hotkey = 'Ctrl+Alt+T'
    $ini = @((Join-Path $dir 'OpenTmuxTabs.ini'), (Join-Path $env:APPDATA 'OpenTmuxTabs\OpenTmuxTabs.ini')) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($ini) {
        $cfg = @{}
        foreach ($line in Get-Content $ini) { if ($line -match '^\s*(\w+)\s*=\s*(.*?)\s*$') { $cfg[$Matches[1]] = $Matches[2] } }
        $mods = 0; $vk = 0
        if ([int]::TryParse([string]$cfg['HotkeyMods'], [ref]$mods) -and [int]::TryParse([string]$cfg['HotkeyKey'], [ref]$vk) -and $mods -ne 0 -and $vk -ne 0) {
            $parts = @()
            if ($mods -band 8) { $parts += 'Win' }
            if ($mods -band 2) { $parts += 'Ctrl' }
            if ($mods -band 1) { $parts += 'Alt' }
            if ($mods -band 4) { $parts += 'Shift' }
            $names = @{ 0x20 = 'Space'; 0x09 = 'Tab'; 0xBE = '.'; 0xBC = ','; 0xBD = '-'; 0xBB = '+' }
            if (($vk -ge 0x30 -and $vk -le 0x39) -or ($vk -ge 0x41 -and $vk -le 0x5A)) { $key = [string][char]$vk }
            elseif ($vk -ge 0x70 -and $vk -le 0x87) { $key = 'F' + ($vk - 0x6F) }
            elseif ($names.ContainsKey($vk)) { $key = $names[$vk] }
            else { $key = '#' + $vk }
            $hotkey = (($parts + $key) -join '+')
        }
        Write-Host "Using existing settings: $ini"
    }
    # Start menu entry
    $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Open-TmuxTabs.lnk'
    $ws = New-Object -ComObject WScript.Shell
    $sc = $ws.CreateShortcut($lnk)
    $sc.TargetPath = $exe
    $sc.WorkingDirectory = $dir
    $sc.Description = 'Open-TmuxTabs - tmux sessions as Windows Terminal tabs'
    $sc.Save()
    Write-Host "Open-TmuxTabs installed to $dir. Hotkey: $hotkey"

    # Ask whether to start now (Enter = yes; J/Y = yes, N = no). Non-interactive: start.
    $start = $true
    try {
        $answer = Read-Host 'Start Open-TmuxTabs now? The settings dialog will open [Y/n]'
        if ($answer -match '^\s*[nN]') { $start = $false }
    } catch { }
    if ($start) {
        Start-Process -FilePath $exe -ArgumentList '--settings'
        Write-Host 'Open-TmuxTabs started. Check or change the hotkey in the settings dialog.'
    } else {
        Write-Host "Open-TmuxTabs was not started. Start it later from the Start menu or with: & '$exe'"
    }
}
