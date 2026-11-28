# Relatório — preparação do BWSmartClips 1.4.0 para a Microsoft Store

Data: 28/11/2026 · Branch: `loja-microsoft` · Versão preservada: branch `preservado/v1.3.0` (+ releases v1.0–v1.3 intactas)

## 1. Resultado em uma frase

O pacote MSIX da edição Microsoft Store está pronto e foi testado num Windows limpo do GitHub. Ele instala, roda todas as ferramentas de dentro do pacote, consulta a licença na Store e passou no Kit de Certificação de Aplicativos do Windows (WACK) com resultado geral **PASS**. Para publicar faltam dados que só você tem (identidade do Partner Center, nome/e-mail nos documentos) e algumas decisões listadas na seção 8.

## 2. O alerta de "Trojan" (prioridade 1)

**Investigação.** O alerta era `Trojan:Win32/Bearfoos.A!ml`. O sufixo `!ml` indica detecção por aprendizado de máquina, não assinatura de vírus. Não tratei como falso positivo automático: revisei o que o app faz e encontrei comportamentos que, juntos, parecem os de um programa malicioso. Corrigi todos os que eram falhas reais:

| Comportamento | Situação antes | Agora |
|---|---|---|
| Instalador sem assinatura digital | Toda versão | Edição Store: pacote **assinado pela Microsoft** (grátis) |
| Lia cookies de 5 navegadores sozinho | Automático | Só se o usuário escolher um navegador |
| Logins salvos em texto puro | `*-cookies.txt` | Criptografados pelo Windows (DPAPI); antigos convertidos e apagados |
| Chaves de API em texto puro | `settings.json` | Criptografadas (DPAPI) |
| Baixava e executava programas sem conferir | yt-dlp, ffmpeg, etc. | Só aceita com o **SHA-256 publicado** conferido; edição Store não baixa programas |
| Modelos de IA sem conferência | Hugging Face | SHA-256 publicado conferido |

**Solução definitiva.** Na Microsoft Store, o pacote é assinado pela Microsoft e entregue pela própria Store. Isso elimina a causa principal: um executável novo, sem assinatura e sem reputação. Para o instalador direto (fora da Store), a única solução definitiva é um certificado de assinatura de código, que é pago; não contratei, conforme sua instrução.

**Limitação honesta.** Não consigo rodar o Microsoft Defender nem o VirusTotal daqui. Recomendo enviar o `.msix` ao VirusTotal antes de publicar.

## 3. O que foi feito

### Microsoft Store
- **Pacote MSIX** (`store/`): manifesto com `runFullTrust`, dependência do runtime VC++ (a Store instala sozinha), ícones gerados do seu ícone e alias de linha de comando para testes.
- **Ferramentas dentro do pacote**, em versões fixas e com SHA-256 conferido (`store/tools.lock.json`): FFmpeg 9.1, whisper.cpp b5907 e llama.cpp b12835 (CPU e Vulkan). Só os arquivos necessários entram no pacote.
- **A edição Store nunca baixa programas** (política 10.2.2). Ela baixa só modelos de IA, que são arquivos de dados conferidos por SHA-256, e ignora caminhos manuais de ferramentas.
- **Licença pela Store** (`src-tauri/src/license.rs`): `Windows.Services.Store.StoreContext.GetAppLicenseAsync`. Detalhes:
  - Sem servidor próprio e sem mensalidade.
  - Funciona offline com a licença guardada pela Store, e tolera até 30 dias sem resposta da Store.
  - Executável copiado para fora do pacote não roda.
  - A verificação fica no lado nativo (Rust) antes de cada trabalho, e a tela "Licença necessária" tem os botões "Abrir na Microsoft Store" e "Verificar novamente".
  - Reconhece licença de avaliação, caso você ofereça.
- **IA generativa (política 11.16)**: aviso em cada corte, botão de denúncia em Configurações → Sobre e texto na descrição da Store.
- **CI** (`.github/workflows/build.yml`, job "Pacote Microsoft Store"): compila a edição Store, monta o `.msix` sem assinatura (o que vai para o Partner Center) e uma cópia de teste assinada, instala, roda o autoteste e o WACK.

### Segurança (as duas edições)
- DPAPI para logins e chaves (`crates/bwsc-core/src/secret.rs`).
- Navegadores só sob escolha do usuário.
- SHA-256 em todos os downloads.
- Política de segurança de conteúdo (CSP) sem conteúdo remoto. As janelas de login (sites externos) não têm acesso ao app.
- Trabalho cancelado ou com erro não deixa lixo na pasta de cortes.

### Proteção do código
- **Licença do projeto trocada de MIT para proprietária.** As versões 1.0 a 1.3 foram publicadas como MIT, e essa permissão não pode ser revogada para quem já copiou.
- Binário com LTO completo, uma unidade de código e sem símbolos; interface minificada na edição Store.
- No Windows, o pacote da Store fica em `WindowsApps`, uma pasta protegida em que o próprio usuário não consegue executar os arquivos diretamente (confirmado no teste: "Access is denied").
- **O repositório ainda está público** (veja a seção 8).

### Documentos (PT-BR e EN)
Os documentos ficam em `docs/legal/` e dentro do app (Configurações → Sobre):
- Política de Privacidade.
- Termos de Uso.
- Licença de Uso (EULA).
- Avisos de terceiros, com 470 bibliotecas, gerados automaticamente por `scripts/gerar-documentos.py`, e os textos completos das licenças.

O guia de publicação e os textos da página da Store estão em `docs/loja/`.

## 4. Licenças de dependências e modelos

| Componente | Licença | Comercialização |
|---|---|---|
| 470 bibliotecas Rust (Tauri etc.) | MIT/Apache/BSD/ISC/Zlib/Unicode/MPL-2.0 | Permitida; MPL exige só aviso (sem modificação) |
| whisper.cpp, llama.cpp | MIT | Permitida |
| FFmpeg (build de gyan.dev) | **GPL-3.0** | Permitida como programa separado, com aviso e oferta do código-fonte (já incluídos). Alternativa mais simples: build LGPL + codificador do Windows (veja a seção 8) |
| Qwen 3.5 | Apache-2.0 | Permitida |
| Whisper, Silero VAD | MIT | Permitida |
| Fonte Montserrat | OFL-1.1 | Permitida (embutida no app) |
| SeetaFace (rosto) | BSD-2 | Permitida |
| yt-dlp / Deno / gerador de token | Unlicense / MIT / a confirmar | Só na edição direta |

Observação: codificar vídeo em H.264 pode envolver patentes (Via LA). Isso não é parecer jurídico: recomendo revisão por um advogado.

## 5. Download de vídeos — incompatibilidade e alternativas

Pela regra 10.1.6 da Store, quem acessa conteúdo de um serviço de terceiros precisa de permissão nos termos desse serviço. Os termos do YouTube e da Twitch proíbem baixar conteúdo fora dos meios autorizados. O gerador de token existe justamente para contornar a checagem anti-robô do YouTube, o que agrava o risco. **Não removi nada.** A função virou uma opção de compilação (`link-download`), e o pacote Store de teste foi gerado **sem** ela. Alternativas:

- **A (recomendada):** a Store fica só com arquivos locais, e o app orienta a usar a gravação da live ou o vídeo baixado pelo YouTube Studio ou pela Twitch (o criador baixa os próprios vídeos pelos meios oficiais). A edição direta mantém o download.
- **B:** a edição direta continua só para uso pessoal ou interno, sem venda.
- **C:** ligar o download também na Store. Isso traz risco alto de reprovação ou remoção e risco jurídico; não recomendo.

## 6. Testes realizados

| Teste | Resultado |
|---|---|
| Testes automáticos (Linux), edição direta e Store | 47 aprovados, 0 falhas |
| Clippy (análise estática), as duas edições | Sem avisos |
| Compilação Windows: instalador direto e edição Store | OK |
| SHA-256 publicado de 7 ferramentas e 9 modelos | Todos encontrados; arquivo adulterado é recusado (teste real) |
| MSIX: instalar num Windows limpo | OK (`C:\Program Files\WindowsApps\...`) |
| Ferramentas executadas de dentro do pacote | ffmpeg, ffprobe, whisper-cli, llama-server e llama-server Vulkan: OK |
| Vídeo de teste gerado e lido no pacote | 1280×720, 3 s, com áudio: OK |
| Licença consultada na Store (pacote fora da Store) | "no-license", como esperado |
| WACK | **PASS** geral. Único item opcional com falha: "Blocked executables", porque o app executa as próprias ferramentas; segundo a Microsoft, os testes opcionais são só informativos |
| Interface: tela de licença, aba Sobre, CSP | Conferidas em prévia e no app |

**Não testado:** o fluxo completo (transcrição, IA e renderização) dentro do MSIX, porque o computador de teste não tem os modelos; a compra real na Store, que exige publicar (dá para testar em audiência privada); e Windows no Modo S.

## 7. Custos

- **Microsoft Store:** comissão de 15% sobre a venda de apps pelo sistema da Microsoft. A assinatura do pacote e a hospedagem são grátis.
- **GitHub, se o repositório ficar privado:** as compilações passam a consumir a cota do plano grátis (2.000 minutos/mês; o Windows conta em dobro). Uma rodada completa usa cerca de 70 a 90 minutos da cota. Dá para rodar o Windows só sob demanda.
- **Opcionais:** certificado de assinatura para o instalador direto (pago, anual) e revisão jurídica.

## 8. Pendências que dependem de você

1. **Tornar o repositório privado** (GitHub → Settings → General → Change visibility). Efeitos: o código deixa de ficar exposto; os links das releases v1.0–1.3 deixam de funcionar para outras pessoas; as compilações passam a usar cota.
2. **Escolher a alternativa do download** (seção 5).
3. **Edição direta:** com a venda pela Store, distribuir o instalador grátis no GitHub concorre com ela. Decidir se para, fica privado ou continua.
4. **Partner Center:** reservar o nome, pegar a identidade do app e criar as 4 variáveis no GitHub (`docs/loja/guia-de-publicacao.md`).
5. **Documentos:** preencher nome/razão social, CPF/CNPJ, cidade, e-mail de suporte e data. Pôr o e-mail em `SUPPORT_EMAIL` (`ui/app.js`). Publicar a Política de Privacidade num endereço público. Revisão jurídica recomendada.
6. **Juntar o branch `loja-microsoft` ao `main`**: isso publica automaticamente a v1.4.0 da edição direta em Releases, então só faço com a sua autorização.
7. **FFmpeg:** manter o GPL (já em conformidade) ou trocar pela build LGPL com o codificador do Windows (exige testar a qualidade).
8. **Publicar na Store:** só você envia a submissão.

## 9. Como publicar

Siga `docs/loja/guia-de-publicacao.md`: reservar o nome → configurar as variáveis no GitHub → rodar o workflow → baixar o `.msix` (o arquivo sem "TESTE") → criar a submissão (preço pago único, propriedades, classificação etária, declaração de IA, justificativa do runFullTrust, textos de `docs/loja/listagem-da-loja.md`, notas para a certificação) → enviar.
