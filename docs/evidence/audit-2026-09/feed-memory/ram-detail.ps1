# Per-process breakdown of ONE Wind instance (spike D-53): the same
# selection as e2e/measure-ram.ps1 (-AppPid + WebView2 profile in the
# command line), one JSON object per process with its WebView2 role
# (--type=gpu-process / renderer / utility ...). The headline figure
# stays measure-ram.ps1's; this is the "where" behind it.
param(
    [int] $AppPid = 0,
    [string] $Profil = ''
)
$motif = [regex]::Escape($Profil)
$procs = @()
$procs += Get-CimInstance Win32_Process -Filter "ProcessId=$AppPid"
$procs += Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
    Where-Object { $_.CommandLine -match $motif }
$ids = $procs.ProcessId
$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains $_.IDProcess }
$rows = foreach ($p in $procs) {
    $w = ($perf | Where-Object { $_.IDProcess -eq $p.ProcessId } | Select-Object -First 1).WorkingSetPrivate
    $role = 'app'
    if ($p.Name -eq 'msedgewebview2.exe') {
        $role = 'browser'
        if ($p.CommandLine -match '--type=([a-z-]+)') { $role = $Matches[1] }
        if ($role -eq 'utility' -and $p.CommandLine -match '--utility-sub-type=([A-Za-z.]+)') { $role = $Matches[1] }
    }
    [pscustomobject]@{ pid = $p.ProcessId; role = $role; mb = [math]::Round($w / 1MB, 1) }
}
ConvertTo-Json -Compress @($rows)
