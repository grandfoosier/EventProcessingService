Param(
    [int]$StartDelaySeconds = 1
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
Set-Location $repoRoot

Write-Host "Building project..."
& cargo build
if ($LASTEXITCODE -ne 0) { Write-Error "cargo build failed"; exit $LASTEXITCODE }

$exe = Join-Path $repoRoot "target\debug\event_processing_service.exe"
if (-Not (Test-Path $exe)) { Write-Error "Executable not found: $exe"; exit 1 }

Write-Host "Starting server..."
$proc = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds $StartDelaySeconds

try {
    Write-Host "\nGET /healthz"
    $h = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:3000/healthz" -Method GET -ErrorAction Stop
    if ($h.Content -is [System.Byte[]]) { $htext = [System.Text.Encoding]::UTF8.GetString($h.Content) } else { $htext = $h.Content }
    Write-Host $htext

    Write-Host "\nGET /metrics (first 20 lines)"
    $m = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:3000/metrics" -Method GET -ErrorAction Stop
    if ($m.Content -is [System.Byte[]]) { $mtext = [System.Text.Encoding]::UTF8.GetString($m.Content) } else { $mtext = $m.Content }
    $lines = $mtext -split "`n"
    $lines[0..([math]::Min(19, $lines.Length-1))] | ForEach-Object { Write-Host $_ }

    Write-Host "\nPOST /events"
    $body = @{ event_id = "smoke-1"; event_type = "smoke"; occurred_at = (Get-Date).ToString("o"); payload = @{ foo = "bar" } } | ConvertTo-Json -Compress
    $post = Invoke-RestMethod -UseBasicParsing -Uri "http://127.0.0.1:3000/events" -Method Post -ContentType "application/json" -Body $body -ErrorAction Stop
    $post | ConvertTo-Json -Depth 5 | Write-Host

    Start-Sleep -Seconds 1

    Write-Host "\nGET /events/smoke-1"
    $get = Invoke-RestMethod -UseBasicParsing -Uri "http://127.0.0.1:3000/events/smoke-1" -Method Get -ErrorAction Continue
    if ($get -eq $null) {
        Write-Host "GET returned no JSON; raw response:"
        $raw = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:3000/events/smoke-1" -ErrorAction SilentlyContinue
        Write-Host $raw.Content
    } else {
        $get | ConvertTo-Json -Depth 5 | Write-Host
    }

} finally {
    if ($proc -and $proc.Id) {
        Write-Host "\nStopping server (PID $($proc.Id))..."
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 200
    }
}

Write-Host "Done."
