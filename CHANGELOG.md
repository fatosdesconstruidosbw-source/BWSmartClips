# Histórico de versões

Formato: cada versão tem uma seção `## [X.Y.Z] - AAAA-MM-DD`. O texto da seção vira a página da versão em
[Releases](https://github.com/fatosdesconstruidosbw-source/BWSmartClips/releases).

## [Não lançado]

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
- **Escolha a pasta dos modelos de IA** (Configurações → Ferramentas e modelos, e também na primeira configuração):
  guarde os modelos em outro disco (ex.: `D:`) para não lotar o C. Se você já tem arquivos `.gguf`/`.bin` numa pasta,
  é só escolher essa pasta: o app usa os modelos de lá, sem copiar. Os modelos que já estavam na pasta antiga continuam
  funcionando, e o botão **"Mover para a nova pasta"** leva todos para o novo lugar (com progresso, sem perder nada se
  for interrompido).

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
