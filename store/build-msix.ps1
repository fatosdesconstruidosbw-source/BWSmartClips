<#
  Monta o pacote MSIX da edição Microsoft Store do BWSmartClips.

  - Copia o executável da edição Store (compilado com --features store).
  - Baixa as ferramentas de store/tools.lock.json (versões fixas) e só aceita se o SHA-256
    conferir; copia apenas os arquivos necessários (nada de executáveis extras).
  - Inclui os ícones, os documentos legais e os avisos de licença de terceiros.
  - Gera o .msix SEM assinatura (é o que se envia ao Partner Center: a Microsoft assina).
  - Com -TestSign, gera também uma cópia assinada com um certificado de teste, só para
    instalar e testar fora da Store.
#>
param(
  [Parameter(Mandatory)] [string] $AppExe,
  [Parameter(Mandatory)] [string] $Version,          # 1.4.0  (vira 1.4.0.0)
  [string] $IdentityName = "BWSmartClips.Teste",
  [string] $Publisher = "CN=BWSmartClips Teste",
  [string] $PublisherDisplayName = "BWSmartClips",
  [string] $OutDir = "out/store",
  [switch] $TestSign
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$staging = Join-Path $OutDir "staging"
$cache = Join-Path $OutDir "downloads"
Remove-Item -Recurse -Force $staging -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $staging, $cache, "$staging/tools", "$staging/licenses" | Out-Null

# --- versão no formato do MSIX (o último número precisa ser 0 na Store)
$parts = $Version.Split("-")[0].Split(".")
while ($parts.Count -lt 3) { $parts += "0" }
$msixVersion = "$($parts[0]).$($parts[1]).$($parts[2]).0"

# --- executável
Copy-Item $AppExe "$staging/BWSmartClips.exe"

# --- ferramentas com versão fixa e SHA-256 conferido
$lock = Get-Content "$PSScriptRoot/tools.lock.json" -Raw | ConvertFrom-Json
foreach ($t in $lock.tools) {
  $zip = Join-Path $cache ([IO.Path]::GetFileName($t.url))
  if (-not (Test-Path $zip)) {
    Write-Host "Baixando $($t.name)…"
    Invoke-WebRequest -Uri $t.url -OutFile $zip -UseBasicParsing
  }
  $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
  if ($hash -ne $t.sha256) { throw "SHA-256 de $($t.name) não confere: $hash (esperado $($t.sha256))" }
  Write-Host "$($t.name): SHA-256 confere"
  $ex = Join-Path $cache ("x-" + [IO.Path]::GetFileNameWithoutExtension($zip))
  Remove-Item -Recurse -Force $ex -ErrorAction SilentlyContinue
  Expand-Archive $zip -DestinationPath $ex
  $dest = if ($t.dest) { Join-Path "$staging/tools" $t.dest } else { "$staging/tools" }
  New-Item -ItemType Directory -Force $dest | Out-Null
  foreach ($pattern in $t.files) {
    $found = Get-ChildItem $ex -Recurse -File -Filter $pattern
    if (-not $found) { throw "$($t.name): $pattern não encontrado no pacote" }
    foreach ($f in $found) {
      if ($f.Name -eq "LICENSE") { Copy-Item $f.FullName "$staging/licenses/$($t.name)-LICENSE.txt" }
      else { Copy-Item $f.FullName $dest -Force }
    }
  }
}

# --- ícones, documentos legais e avisos de terceiros
Copy-Item -Recurse "$PSScriptRoot/Assets" "$staging/Assets"
if (Test-Path "$root/docs/legal") { Copy-Item -Recurse "$root/docs/legal" "$staging/legal" }
if (Test-Path "$root/docs/legal/licencas") { Copy-Item "$root/docs/legal/licencas/*" "$staging/licenses/" -Force }

# --- manifesto
$manifest = Get-Content "$PSScriptRoot/AppxManifest.template.xml" -Raw
$manifest = $manifest.Replace("{{IDENTITY_NAME}}", $IdentityName).Replace("{{PUBLISHER}}", [Security.SecurityElement]::Escape($Publisher))
$manifest = $manifest.Replace("{{PUBLISHER_DISPLAY_NAME}}", [Security.SecurityElement]::Escape($PublisherDisplayName)).Replace("{{VERSION}}", $msixVersion)
Set-Content -Path "$staging/AppxManifest.xml" -Value $manifest -Encoding utf8

# --- empacotar
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $sdk) { throw "makeappx.exe não encontrado (Windows SDK)" }
$bin = Split-Path $sdk.FullName
$msix = Join-Path $OutDir "BWSmartClips_${msixVersion}_x64.msix"
& "$bin\makeappx.exe" pack /d $staging /p $msix /o /h SHA256
if ($LASTEXITCODE -ne 0) { throw "makeappx falhou" }
Write-Host "Pacote para a Store: $msix"

if ($TestSign) {
  $pfxPass = "teste-bwsc"
  $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName "BWSmartClips (teste)" `
    -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
  $pfx = Join-Path $OutDir "teste.pfx"
  Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $pfxPass -AsPlainText -Force) | Out-Null
  Export-Certificate -Cert $cert -FilePath (Join-Path $OutDir "BWSmartClips-certificado-de-teste.cer") | Out-Null
  $signed = Join-Path $OutDir "BWSmartClips_${msixVersion}_x64-TESTE-assinado.msix"
  Copy-Item $msix $signed
  & "$bin\signtool.exe" sign /fd SHA256 /f $pfx /p $pfxPass $signed
  if ($LASTEXITCODE -ne 0) { throw "signtool falhou" }
  Remove-Item $pfx
  Write-Host "Pacote de teste assinado: $signed"
}
"msix=$msix" | Out-File -Append -Encoding utf8 $env:GITHUB_OUTPUT -ErrorAction SilentlyContinue
"version=$msixVersion" | Out-File -Append -Encoding utf8 $env:GITHUB_OUTPUT -ErrorAction SilentlyContinue
