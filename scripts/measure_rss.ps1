# Mesure le pic de memoire resident (RSS) d'un processus Windows.
# Usage: measure_rss.ps1 -Exe <chemin> -Arguments "<ligne de commande>"
#
# `-Arguments` est une ligne de commande unique, pas un tableau : `Start-Process`
# recolle un tableau avec des espaces, sans quoter, ce qui casse les phrases.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Arguments,
    [int]$IntervalMs = 150
)

$proc = Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru -NoNewWindow
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

Write-Output ("PEAK_RSS_MB=" + [math]::Round($peak / 1MB, 1))
Write-Output ("EXIT_CODE=" + $proc.ExitCode)
