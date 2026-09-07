$ErrorActionPreference = 'Stop'
$spikeRoot = $PSScriptRoot
$exePath = Join-Path $spikeRoot 'target/release/lot3-raw-cap-spike.exe'
$corpusPath = Join-Path $spikeRoot 'corpus'
$candidateDir = Join-Path $spikeRoot 'candidates'
New-Item -ItemType Directory -Force -Path $candidateDir | Out-Null
$results = @()
foreach ($capMiB in @(8,16,32)) {
 $capBytes = $capMiB * 1048576
 $jobs = @()
 foreach ($metadata in @('honest','missing','lying')) {
  foreach ($edge in @('cap-minus','cap-exact','cap-plus','abuse64')) {
   $jobs += @{ Case = "$metadata-$edge"; Conversion = 'parse' }
  }
 }
 foreach ($content in @('honest-cap-exact','honest-html','honest-mixed')) {
  foreach ($conversion in @('parse','convert')) {
   # honest plain parser-only already exists in the edge matrix.
   if ($content -eq 'honest-cap-exact' -and $conversion -eq 'parse') { continue }
   $jobs += @{ Case = $content; Conversion = $conversion }
  }
 }
 foreach ($job in $jobs) {
  $case = $job.Case
  $conversion = $job.Conversion
  $stem = "$capMiB-$case-$conversion"
  $stdoutPath = Join-Path $candidateDir "$stem.stdout.json"
  $stderrPath = Join-Path $candidateDir "$stem.stderr.txt"
  $process = Start-Process -FilePath $exePath -ArgumentList @($case,'bounded',$corpusPath,$capBytes,$conversion) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
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
  if ($process.ExitCode -ne 0) { throw "Candidate $stem failed: $(Get-Content -LiteralPath $stderrPath -Raw)" }
  $record = Get-Content -LiteralPath $stdoutPath -Raw | ConvertFrom-Json
  $record | Add-Member -NotePropertyName os_peak_working_set_bytes -NotePropertyValue $peakWorkingSet
  $record | Add-Member -NotePropertyName os_peak_paged_memory_bytes -NotePropertyValue $peakPagedMemory
  $record | Add-Member -NotePropertyName sampled_peak_private_commit_bytes -NotePropertyValue $sampledPrivateMax
  $record | Add-Member -NotePropertyName memory_samples -NotePropertyValue $samples
  $results += $record
  $results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $spikeRoot 'candidate-results.json') -Encoding utf8
  Write-Output "$stem $($record.status), WS=$peakWorkingSet, elapsed=$($record.elapsed_ms) ms"
 }
}
