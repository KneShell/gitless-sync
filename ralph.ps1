# Ralph Loop
# Usage:
#   .\ralph.ps1 plan          # Planning mode (default 3 iterations)
#   .\ralph.ps1 build         # Build mode (default 20 iterations)
#   .\ralph.ps1 build 5       # Build mode (5 iterations max)

param(
    [string]$Mode = "build",
    [int]$MaxIterations = 0
)

# UTF-8 encoding
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null

$promptFile = switch ($Mode) {
    "plan" {
        if ($MaxIterations -eq 0) { $MaxIterations = 3 }
        "docs/ralph/prompt-plan.md"
    }
    "build" {
        if ($MaxIterations -eq 0) { $MaxIterations = 20 }
        "docs/ralph/prompt-build.md"
    }
    default {
        Write-Host "Usage: .\ralph.ps1 {plan|build} [max_iterations]" -ForegroundColor Red
        exit 1
    }
}

if (-not (Test-Path $promptFile)) {
    Write-Host "[ralph] Prompt file not found: $promptFile" -ForegroundColor Red
    exit 1
}

$sessionStart = Get-Date
$outputFile = [System.IO.Path]::GetTempFileName()

Write-Host ""
Write-Host "==========================================" -ForegroundColor Green
Write-Host "[ralph] Mode: $Mode | Max: $MaxIterations iterations" -ForegroundColor Green
Write-Host "[ralph] Prompt: $promptFile" -ForegroundColor Green
Write-Host "[ralph] Started: $($sessionStart.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
Write-Host ""

$completedIterations = 0

try {
    for ($i = 1; $i -le $MaxIterations; $i++) {
        $iterStart = Get-Date
        $timestamp = $iterStart.ToString("yyyy-MM-dd HH:mm:ss")

        Write-Host ""
        Write-Host "==========================================" -ForegroundColor Green
        Write-Host "[ralph] Iteration $i / $MaxIterations [$timestamp]" -ForegroundColor Green
        Write-Host "==========================================" -ForegroundColor Green

        $promptContent = Get-Content $promptFile -Raw

        $promptContent | claude --dangerously-skip-permissions --effort max 2>&1 | Tee-Object -FilePath $outputFile -Encoding UTF8

        $exitCode = $LASTEXITCODE
        $iterDuration = (Get-Date) - $iterStart

        if ($exitCode -ne 0) {
            Write-Host "[ralph] Claude exited with code $exitCode. Pausing 30s..." -ForegroundColor Red
            Start-Sleep -Seconds 30
        }

        if (Select-String -Path $outputFile -Pattern "<promise>COMPLETE</promise>" -Quiet) {
            Write-Host "[ralph] Completion signal received." -ForegroundColor Green
            $completedIterations = $i
            break
        }

        $completedIterations = $i
        Write-Host "[ralph] Iteration $i done ($([math]::Round($iterDuration.TotalMinutes, 1))m)" -ForegroundColor Green

        if ($i -lt $MaxIterations) {
            Start-Sleep -Seconds 5
        }
    }
}
finally {
    Remove-Item -Path $outputFile -ErrorAction SilentlyContinue

    $sessionEnd = Get-Date
    $totalDuration = $sessionEnd - $sessionStart

    Write-Host ""
    Write-Host "==========================================" -ForegroundColor Green
    Write-Host "[ralph] SESSION COMPLETE" -ForegroundColor Green
    Write-Host "[ralph] Iterations: $completedIterations / $MaxIterations" -ForegroundColor Green
    Write-Host "[ralph] Duration: $([math]::Floor($totalDuration.TotalMinutes))m $($totalDuration.Seconds)s" -ForegroundColor Green
    Write-Host "[ralph] Ended: $($sessionEnd.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor Green
    Write-Host "==========================================" -ForegroundColor Green
}
