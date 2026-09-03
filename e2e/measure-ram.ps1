# Sum of the PRIVATE working sets: wind-desktop.exe + its WebView2
# processes (identified by their dev.elements.wind command line).
#
# -AppPid and -Profil RESTRICT the measurement to ONE instance. Without
# them, the script summed every instance on the machine: a window opened
# by the user during the measurement was added to the bench's, and the
# total meant nothing anymore. Found at gate 3 -- 14 processes instead
# of 7, 202 Mo for two applications.
param(
    [int] $AppPid = 0,
    [string] $Profil = ''
)

$ids = @()
if ($AppPid -gt 0) {
    $ids += $AppPid
} else {
    $ids += (Get-CimInstance Win32_Process -Filter "Name='wind-desktop.exe'").ProcessId
}

$webview = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'"
if ($Profil -ne '') {
    # The user data folder identifies the instance reliably: WebView2
    # passes it to each of its child processes. The profile is ENOUGH: an
    # e2e profile (target\e2e\webview2) does not carry dev.elements.wind
    # in its command line -- filtered before the profile, the measurement
    # only counted the exe and one process (field finding STOP 2
    # PLAN-AUDIT-V2: 6 Mo on 2 processes, a lie).
    $motif = [regex]::Escape($Profil)
    $webview = $webview | Where-Object { $_.CommandLine -match $motif }
} else {
    $webview = $webview | Where-Object { $_.CommandLine -match 'dev\.elements\.wind' }
}
$ids += $webview.ProcessId

$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains $_.IDProcess }
$sum = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum
"{0:N1} Mo (private working set, {1} processes)" -f ($sum / 1MB), @($ids).Count
