# Guia de publicação do BWSmartClips na Microsoft Store

Este guia leva o pacote gerado pelo GitHub Actions até a Store. Nada é publicado sem você enviar a submissão no Partner Center.

## Visão geral

| O que | Onde |
|---|---|
| Pacote para enviar | Artefato **BWSmartClips-MicrosoftStore-MSIX** do GitHub Actions → arquivo `BWSmartClips_X.Y.Z.0_x64.msix` (o que **não** tem "TESTE" no nome) |
| Textos da página | `docs/loja/listagem-da-loja.md` |
| Política de Privacidade, Termos, EULA | `docs/legal/` (PT-BR e EN) |
| Avisos de terceiros | `docs/legal/*/avisos-de-terceiros.md` / `third-party-notices.md` |

Por que MSIX: só pacotes MSIX usam a cobrança, a licença e as atualizações da própria Microsoft Store, e a Microsoft assina o pacote de graça. Instaladores .exe/.msi não têm cobrança pela Store e exigiriam certificado de assinatura pago.

## Passo 1 — Reservar o nome e pegar a identidade do app

1. Partner Center → **Apps e jogos** → **Novo produto** → **Aplicativo MSIX ou PWA** → reserve **BWSmartClips** (ou o nome disponível).
2. Abra o app → **Gerenciamento de produto** → **Identidade do produto**. Anote:
   - `Package/Identity/Name` (ex.: `12345SeuNome.BWSmartClips`)
   - `Package/Identity/Publisher` (ex.: `CN=XXXXXXXX-XXXX-...`)
   - `Package/Properties/PublisherDisplayName`
   - **Store ID** (ex.: `9NXXXXXXXXXX`)

## Passo 2 — Configurar o GitHub para gerar o pacote com a sua identidade

GitHub → repositório → **Settings → Secrets and variables → Actions → Variables → New repository variable**. Crie:

| Variável | Valor |
|---|---|
| `STORE_IDENTITY_NAME` | Package/Identity/Name |
| `STORE_PUBLISHER` | Package/Identity/Publisher (com o `CN=`) |
| `STORE_PUBLISHER_DISPLAY_NAME` | PublisherDisplayName |
| `STORE_ID` | Store ID (o botão "Abrir na Microsoft Store" do app usa esse valor) |

Depois: **Actions → Build → Run workflow** (no branch que será publicado). Baixe o artefato **BWSmartClips-MicrosoftStore-MSIX**.

Sem essas variáveis o pacote sai com uma identidade de teste e **o Partner Center recusa o envio**.

## Passo 3 — Preencher os documentos

Nos arquivos de `docs/legal/`, substitua: `[NOME DO TITULAR OU RAZÃO SOCIAL]`, `[CPF/CNPJ]`, `[CIDADE/UF]`, `[E-MAIL DE SUPORTE]` e `[DATA DE PUBLICAÇÃO]` (e os equivalentes em inglês). Coloque o mesmo e-mail em `SUPPORT_EMAIL` no início da seção "edição, licença e documentos" de `ui/app.js` e rode `python3 scripts/gerar-documentos.py`. Recomendo revisão por um advogado.

A Política de Privacidade precisa estar num **endereço público (URL)**. Opções sem custo: uma página publicada do Claude, Google Sites, Notion público ou GitHub Pages num repositório público só com os documentos.

## Passo 4 — Criar a submissão

1. **Preços e disponibilidade:** modelo **Pago**, preço único (ex.: faixa de R$ ...), mercados desejados. Avaliação gratuita é opcional; o app já reconhece licença de avaliação.
2. **Propriedades:** categoria **Foto e vídeo** (subcategoria edição de vídeo); URL da Política de Privacidade; site e contato de suporte; requisitos: 8 GB de RAM, x64.
3. **Classificação etária:** responda o questionário (IARC). O app não tem chat, compras internas nem conteúdo gerado por outros usuários; ele usa IA generativa local para títulos e hashtags.
4. **Declarações / IA generativa:** marque que o produto usa IA generativa (política 11.16). O app já informa isso na descrição e tem o botão para reportar conteúdo.
5. **Pacotes:** envie o `.msix`. Capacidade restrita **runFullTrust** — justificativa (cole):
   > BWSmartClips é um aplicativo de desktop Win32 (Tauri/Rust) empacotado em MSIX. Ele precisa de confiança total para executar, como processos locais, as ferramentas de processamento de vídeo e IA incluídas e assinadas dentro do próprio pacote (FFmpeg, whisper.cpp e llama.cpp), ler os vídeos escolhidos pelo usuário e gravar os cortes na pasta escolhida por ele. O app não baixa nem executa código fora do pacote.
6. **Listagem da Store:** textos de `docs/loja/listagem-da-loja.md`, capturas de tela e logotipo.
7. **Notas para a certificação** (cole):
   > Não é preciso conta nem login. Na primeira abertura o assistente baixa os modelos de IA (Hugging Face, cerca de 3 a 5 GB). Para testar: clique em "Carregar arquivo", escolha qualquer vídeo MP4 com fala (1 a 5 minutos) e clique em "Gerar cortes". O processamento é local; sem placa de vídeo pode levar alguns minutos. A licença é verificada pela Microsoft Store (Windows.Services.Store).

## Passo 5 — Enviar e acompanhar

Envie para certificação. A Microsoft roda testes de segurança, técnicos e de conteúdo; o retorno costuma levar de algumas horas a alguns dias. Se houver reprovação, o relatório indica a política; me envie o texto.

## Atualizações futuras

1. Aumente a versão (`Cargo.toml` e `src-tauri/tauri.conf.json`) e registre no `CHANGELOG.md`.
2. Rode o workflow, baixe o novo `.msix` e crie uma **nova submissão** com ele. A Store atualiza os clientes automaticamente.

## Testar o pacote antes de enviar (opcional)

O artefato traz `...TESTE-assinado.msix` e `BWSmartClips-certificado-de-teste.cer`. Para instalar no seu PC: clique duas vezes no `.cer` → Instalar certificado → **Máquina local** → **Pessoas Confiáveis**; depois abra o `.msix`. Fora da Store a licença não existe, então o app mostra a tela "Licença necessária": isso é o esperado. Desinstale depois e remova o certificado.
