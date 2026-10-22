# Mesure le pic de memoire resident (RSS) d'un processus Windows.
# Usage: measure_rss.ps1 -Exe <chemin> -Args <tableau>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string[]]$ProcessArgs = @(),
    [int]$IntervalMs = 150
)

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe
foreach ($a in $ProcessArgs) { $psi.ArgumentList.Add($a) }
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true

$proc = [System.Diagnostics.Process]::Start($psi)
$stdout = $proc.StandardOutput.ReadToEndAsync()
$stderr = $proc.StandardError.ReadToEndAsync()

$peak = 0
while (-not $proc.HasExited) {
    try {
        $proc.Refresh()
        if ($proc.WorkingSet64 -gt $peak) { $peak = $proc.WorkingSet64 }
    } catch { }
    Start-Sleep -Milliseconds $IntervalMs
}
$proc.WaitForExit()
try { $proc.Refresh(); if ($proc.WorkingSet64 -gt $peak) { $peak = $proc.WorkingSet64 } } catch { }

Write-Output $stdout.Result
if ($stderr.Result) { Write-Output $stderr.Result }
Write-Output ("PEAK_RSS_MB=" + [math]::Round($peak / 1MB, 1))
Write-Output ("EXIT_CODE=" + $proc.ExitCode)
