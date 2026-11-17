# Ставит скиллы, workflows и правила Sad Launcher для Antigravity.
# Запуск из корня проекта:  powershell -ExecutionPolicy Bypass -File antigravity\setup-antigravity.ps1
# Ключ -Global дополнительно кладёт скиллы в общую папку пользователя (~\.gemini\antigravity\skills).
param([switch]$Global)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$pack = Join-Path $root 'antigravity'

function Sync-Dir($from, $to) {
    New-Item -ItemType Directory -Force -Path $to | Out-Null
    Copy-Item -Path (Join-Path $from '*') -Destination $to -Recurse -Force
}

# Antigravity читает .agent/ (в новых версиях .agents/) — кладём в обе папки, лишнее ему не мешает.
foreach ($dir in '.agent', '.agents') {
    Sync-Dir (Join-Path $pack 'skills')    (Join-Path $root "$dir\skills")
    Sync-Dir (Join-Path $pack 'workflows') (Join-Path $root "$dir\workflows")
    Sync-Dir (Join-Path $pack 'rules')     (Join-Path $root "$dir\rules")
}

if ($Global) {
    $g = Join-Path $env:USERPROFILE '.gemini\antigravity\skills'
    Sync-Dir (Join-Path $pack 'skills') $g
    Write-Host "Глобально: $g"
}

$n = (Get-ChildItem (Join-Path $root '.agent\skills') -Directory).Count
$w = (Get-ChildItem (Join-Path $root '.agent\workflows') -File).Count
Write-Host "Готово: скиллов $n, workflows $w, правило allskill.md включено."
Write-Host "Проверка: в Antigravity открой Customizations → Skills/Rules/Workflows и убедись, что allskill виден."
