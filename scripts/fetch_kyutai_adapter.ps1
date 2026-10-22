#!/usr/bin/env powershell
# Script pour récupérer l'adaptateur Kyutai depuis kyutai-labs/pocket-tts

$ErrorActionPreference = "Stop"

# URL de base pour les fichiers raw
$baseUrl = "https://raw.githubusercontent.com/kyutai-labs/pocket-tts/main/rust/src"

# Liste des fichiers à récupérer
$files = @(
    "config.rs",
    "text.rs", 
    "model.rs",
    "tokenizer.rs",
    "weights.rs",
    "rng.rs"
)

# Dossier de destination
$destDir = "$PSScriptRoot/../src/kyutai"
if (-not (Test-Path $destDir)) {
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
}

# Télécharger chaque fichier
foreach ($file in $files) {
    $url = "$baseUrl/$file"
    $destPath = Join-Path $destDir $file
    
    Write-Host "Téléchargement de $file..."
    try {
        $content = Invoke-RestMethod -Uri $url -UseBasicParsing
        $content | Out-File -FilePath $destPath -Encoding utf8
        Write-Host "  ✓ $file sauvegardé dans $destPath" -ForegroundColor Green
    } catch {
        Write-Host "  ✗ Échec du téléchargement de $file : $($_.Exception.Message)" -ForegroundColor Red
        exit 1
    }
}

Write-Host "" -ForegroundColor Cyan
Write-Host "Tous les fichiers de l'adaptateur Kyutai ont été récupérés dans : $destDir" -ForegroundColor Green
