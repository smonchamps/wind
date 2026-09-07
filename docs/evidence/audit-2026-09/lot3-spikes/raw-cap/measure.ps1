$ErrorActionPreference = 'Stop'
$spikeRoot = $PSScriptRoot
$exePath = Join-Path $spikeRoot 'target/debug/lot3-raw-cap-spike.exe'
$corpusPath = Join-Path $spikeRoot 'corpus'
$results = @()
$cases = @('honest-64k','missing-64k','lying-64k','honest-1m','missing-1m','lying-1m','honest-8m','missing-8m','lying-8m','long-line','aggregate','malformed','nested','deadline','missing-cap-minus','missing-cap-exact','missing-cap-plus')
foreach ($case in $cases) {
 foreach ($mode in @('bounded','unbounded')) {
  $stdoutPath = Join-Path $spikeRoot "$case-$mode.stdout.json"
  $stderrPath = Join-Path $spikeRoot "$case-$mode.stderr.txt"
  $process = Start-Process -FilePath $exePath -ArgumentList @($case,$mode,$corpusPath) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
  $sampledPrivateMax = 0L
  $peakWorkingSet = 0L
  $peakPagedMemory = 0L
  $samples = 0
  while (-not $process.HasExited) {
   $process.Refresh()
   if (-not $process.HasExited) {
    $sampledPrivateMax = [Math]::Max($sampledPrivateMax,$process.PrivateMemorySize64)
    $peakWorkingSet = [Math]::Max($peakWorkingSet,$process.PeakWorkingSet64)
    $peakPagedMemory = [Math]::Max($peakPagedMemory,$process.PeakPagedMemorySize64)
    $samples++
   }
   Start-Sleep -Milliseconds 2
  }
  if ($process.ExitCode -ne 0) { throw "Spike $case $mode failed: $(Get-Content -LiteralPath $stderrPath -Raw)" }
  $record = Get-Content -LiteralPath $stdoutPath -Raw | ConvertFrom-Json
  $record | Add-Member -NotePropertyName os_peak_working_set_bytes -NotePropertyValue $peakWorkingSet
  $record | Add-Member -NotePropertyName sampled_peak_private_commit_bytes -NotePropertyValue $sampledPrivateMax
  $record | Add-Member -NotePropertyName os_peak_paged_memory_bytes -NotePropertyValue $peakPagedMemory
  $record | Add-Member -NotePropertyName memory_samples -NotePropertyValue $samples
  $results += $record
 }
}
$results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $spikeRoot 'results.json') -Encoding utf8
$results | Select-Object case,bounded,status,body_command_admitted_bytes,returned_body_max,os_peak_working_set_bytes,poisoned,noop_reuse_succeeded | Format-Table -AutoSize
