# =============================================================================
#  Open-TmuxTabs - uninstall
#
#  Usage:
#    irm https://raw.githubusercontent.com/schlingensiepen/tmux-helper-rs/main/deploy/uninstall.ps1 | iex
#
#  Stops Open-TmuxTabs, removes the autostart entry and the Start menu entry and
#  deletes %LOCALAPPDATA%\Programs\OpenTmuxTabs (including OpenTmuxTabs.ini)
#  and %APPDATA%\OpenTmuxTabs.
# =============================================================================
& {
    $ErrorActionPreference = 'Continue'
    Get-Process -Name 'OpenTmuxTabs' -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'OpenTmuxTabs' -ErrorAction SilentlyContinue
    $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Open-TmuxTabs.lnk'
    if (Test-Path $lnk) { Remove-Item -Force $lnk }
    foreach ($d in @((Join-Path $env:LOCALAPPDATA 'Programs\OpenTmuxTabs'), (Join-Path $env:APPDATA 'OpenTmuxTabs'))) {
        if (Test-Path $d) { Remove-Item -Recurse -Force $d; Write-Host "Removed: $d" }
    }
    Write-Host 'Open-TmuxTabs has been removed.'
}
