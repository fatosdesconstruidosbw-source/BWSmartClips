# Histórico de versões

Formato: cada versão tem uma seção `## [X.Y.Z] - AAAA-MM-DD`. O texto da seção vira a página da versão em
[Releases](https://github.com/fatosdesconstruidosbw-source/BWSmartClips/releases).

## [Não lançado]

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
