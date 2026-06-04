param(
    [string]$Repo = "C:\Users\Chan\Projects\Ous"
)

Push-Location $Repo

$local  = git rev-parse HEAD
$remote = git rev-parse "@{u}" 2>$null
$status = git status --short

if (-not $remote) {
    Write-Host "NO_UPSTREAM" -ForegroundColor Yellow
    Pop-Location; exit 1
}

$behind = git rev-list "$local...$remote" --count
$ahead  = git rev-list "$remote...$local" --count
$dirty  = if ($status) { $status.Count } else { 0 }

Write-Host "LOCAL  $($local.Substring(0,7))"
Write-Host "REMOTE $($remote.Substring(0,7))"
Write-Host "AHEAD  $ahead | BEHIND $behind | DIRTY $dirty"

if ($ahead -eq 0 -and $behind -eq 0 -and $dirty -eq 0) {
    Write-Host "SYNC: OK" -ForegroundColor Green
} elseif ($dirty -gt 0) {
    Write-Host "SYNC: UNCOMMITTED ($dirty files)" -ForegroundColor Yellow
} elseif ($behind -gt 0) {
    Write-Host "SYNC: PULL NEEDED" -ForegroundColor Red
} elseif ($ahead -gt 0) {
    Write-Host "SYNC: PUSH NEEDED" -ForegroundColor Cyan
}

Pop-Location
