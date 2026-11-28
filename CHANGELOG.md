# Histórico de versões

Formato: cada versão tem uma seção `## [X.Y.Z] - AAAA-MM-DD`. O texto da seção vira a página da versão em
[Releases](https://github.com/fatosdesconstruidosbw-source/BWSmartClips/releases).

## [Não lançado]

## [1.4.0] - 2026-11-28

### Novidades
- **Edição Microsoft Store** (pacote MSIX): compra única com licença vitalícia pela própria Store, ferramentas de vídeo e
  IA dentro do pacote assinado pela Microsoft, nenhum programa baixado depois da instalação.
- **Configurações → Sobre:** versão, edição, situação da licença, Política de Privacidade, Termos de Uso, Licença de
  Uso (EULA) e avisos de licenças de terceiros, em português e inglês.
- Aviso em cada corte de que título, nota e hashtags são gerados por IA, com botão para reportar conteúdo.

### Segurança
- Logins do YouTube, Twitch e Kick e as chaves de API (OpenAI/Anthropic) passam a ser guardados **criptografados pelo
  Windows** (só a sua conta, neste PC, consegue abrir). Os arquivos antigos em texto puro são convertidos e apagados.
- O app **não lê mais os cookies dos navegadores por conta própria**: só se você escolher um navegador em Avançado.
- Ferramentas (yt-dlp, ffmpeg, Deno, whisper.cpp, llama.cpp, gerador de token) e modelos de IA só são aceitos se o
  **SHA-256 publicado** conferir; arquivo adulterado ou corrompido é descartado.
- Política de segurança de conteúdo (CSP) na interface; executável otimizado e sem símbolos de depuração.

### Correções
- Trabalho cancelado ou com erro não deixa mais pasta pela metade na pasta de cortes (o vídeo baixado é mantido para a
  próxima tentativa); download interrompido é apagado.

### Licença
- A partir desta versão o BWSmartClips é **software proprietário** (todos os direitos reservados). As versões 1.0.0 a
  1.3.0 continuam sob a licença MIT com que foram publicadas.

## [1.3.0] - 2026-11-28

### Novidades
- **Escolha onde salvar os cortes** direto no painel "Cortes": "Escolher pasta…" ou deixar na pasta padrão do app
  (Vídeos\BWSmartClips). Clique no caminho para abrir a pasta. A lista de Projetos continua mostrando os cortes
  salvos em pastas usadas antes.
- Ao clicar em **Gerar cortes**, a tela rola sozinha até o andamento da renderização.
- Botão flutuante **voltar ao topo** no canto da tela.
- **Instale o programa em qualquer disco:** escolha a pasta (ex.: `D:\BWSmartClips`) no instalador, que agora está em
  português. Configurações, ferramentas, modelos, cache e os dados do navegador interno passam a ficar **junto do
  programa**, em vez do disco C.
- Se já existia uma instalação no disco C, o app avisa e o botão **"Mover para a pasta do programa"** leva ferramentas,
  modelos e cache para a nova pasta (com progresso; as configurações são aproveitadas automaticamente).
- **IA local na placa de vídeo** (AMD, NVIDIA ou Intel Arc, via Vulkan): a escolha dos cortes fica bem mais rápida e
  modelos maiores (ex.: Qwen 9B) passam a ser viáveis. O app encontra a placa sozinho, ignora o vídeo integrado e,
  se a placa falhar, volta para o processador com um aviso. Pode ser desligado em Configurações → IA. Para quem já
  tem o app, "Instalar o que falta" baixa o suporte (~60 MB).
- **Escolha a pasta dos modelos de IA** (Configurações → Ferramentas e modelos, e também na primeira configuração):
  guarde os modelos em outro disco (ex.: `D:`) para não lotar o C. Se você já tem arquivos `.gguf`/`.bin` numa pasta,
  é só escolher essa pasta: o app usa os modelos de lá, sem copiar. Os modelos que já estavam na pasta antiga continuam
  funcionando, e o botão **"Mover para a nova pasta"** leva todos para o novo lugar (com progresso, sem perder nada se
  for interrompido).

### Observações
- Use o arquivo **`-setup.exe`** (o `.msi` instala em "Arquivos de Programas", no disco C, e não permite escolher a pasta).
- O instalador ainda não tem assinatura digital, e o Windows Defender pode marcá-lo por engano (alertas terminados em
  `!ml` são suspeitas automáticas, não um vírus conhecido). O instalador é gerado pelo GitHub a partir do código público
  deste repositório; confira no VirusTotal se quiser.

## [1.2.0] - 2026-11-27

### Novidades
- **Layout Gameplay** (Reenquadrar): para lives de jogos, a webcam do streamer fica cravada em cima e o jogo embaixo,
  com a legenda na divisa. O app encontra a webcam sozinho em cada corte (o rosto que fica parado num canto
  e as bordas da moldura), e dá para conferir ou marcar a posição à mão num quadro do vídeo.
- Tamanho da webcam ajustável (25% a 55% da altura). Se um corte não tiver webcam, ele usa "Preencher".

### Correções
- Renderização podia parecer travada no Windows: o ffmpeg agora usa só a fonte das legendas, sem indexar todas as
  fontes do sistema na primeira vez.
- Detecção da webcam e renderização têm limite de tempo: se algo travar, o app segue (ou mostra o erro) em vez de
  ficar parado; a detecção mostra o progresso em %.
- **Layout Gameplay parava no meio** ("Procurando a webcam… 100%"): uma conta do recorte da webcam podia abortar o
  trabalho por arredondamento. Corrigido, com teste de milhares de posições de webcam.
- Qualquer erro interno inesperado agora aparece na tela (e no registro) em vez de deixar o app esperando para sempre;
  no Gameplay, o corte com problema é refeito com "Preencher".
- Registro do último trabalho (`logs\ultimo-trabalho.log`) com a hora de cada etapa, e botão "Abrir registro" quando dá erro.

## [1.1.0] - 2026-11-27

### Novidades
- **Nova identidade visual:** logotipo novo no topo do app e ícone novo no instalador, na janela, na barra de tarefas e nos atalhos.
- **Contas e downloads em um só lugar** (Configurações → Ferramentas e modelos): conecte **YouTube, Twitch e Kick**
  numa janela de login do próprio app, veja o status de cada conta e desconecte quando quiser.
- Quando uma plataforma pede login no meio do trabalho, aparece **"Entrar no YouTube/Twitch/Kick e tentar de novo"**,
  e o corte recomeça sozinho depois do login.
- **Prévia das legendas com vídeo real:** o estúdio de legendas agora mostra um podcast de verdade, em loop e sem áudio.

### Melhorias
- Arquivo cookies.txt e leitura de cookies do navegador foram para a seção "Avançado" e agora valem para qualquer plataforma.

## [1.0.0] - 2026-11-27

Primeira versão do BWSmartClips: cortes virais de vídeos longos, estilo Opus Clip, rodando **100% offline**.

### Novidades
- **Cortes com IA local:** transcrição palavra por palavra (Whisper) e escolha dos melhores momentos por IA local
  (Qwen 3.5 via llama.cpp), com nota de viralidade, título, motivo e hashtags.
- **Reenquadramento 9:16** seguindo o rosto de quem fala, além de Preencher, Fundo desfocado e Original.
- **Legendas animadas** palavra por palavra com 7 estilos prontos (Viral, Karaokê, Impacto, Caixa, Neon, Foco, Clássico)
  e ajustes de fonte, cores, posição e efeitos.
- **Links do YouTube, Twitch e Kick** (VODs e clipes) ou arquivos do computador (inclusive arrastando para a janela).
- **YouTube sem login** na maioria dos casos (token de origem automático) e, se o YouTube bloquear,
  botão **"Entrar no YouTube"** dentro do próprio app.
- **Assistente de primeira abertura** que baixa ferramentas e modelos uma única vez, com opção de importar modelos de um pendrive.
- **"Como funciona":** tour guiado em 8 passos.
- **Cache:** transcrição, análise e download são reaproveitados ao gerar de novo.
- APIs online opcionais: Claude, OpenAI (ou compatível) e Ollama.
