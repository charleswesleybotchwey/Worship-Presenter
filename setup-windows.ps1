# One-time setup for building Worship Presenter on your own Windows PC.
# Run in PowerShell:  powershell -ExecutionPolicy Bypass -File setup-windows.ps1
# Uses winget (built into Windows 10 21H2+ and Windows 11). Takes 15-30 minutes.

$ErrorActionPreference = "Stop"
function Step($m) { Write-Host "`n== $m" -ForegroundColor Cyan }

Step "Visual Studio C++ Build Tools (needed by Rust on Windows)"
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

Step "Rust"
winget install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements

Step "Node.js LTS"
winget install --id OpenJS.NodeJS.LTS -e --accept-package-agreements --accept-source-agreements

Step "WebView2 runtime (already present on Windows 11)"
winget install --id Microsoft.EdgeWebView2Runtime -e --accept-package-agreements --accept-source-agreements 2>$null

Write-Host "`nDone. Close this window, open a NEW PowerShell in the project folder, then run:" -ForegroundColor Green
Write-Host "  rustup default stable-msvc"
Write-Host "  npm run setup"
Write-Host "  npm run dev"
