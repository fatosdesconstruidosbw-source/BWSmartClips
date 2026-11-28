#!/usr/bin/env python3
"""Gera os avisos de licenças de terceiros (PT-BR e EN) e as versões HTML dos documentos legais
usadas dentro do app (ui/legal). Rode após mudar dependências ou documentos:

    cargo license --avoid-dev-deps --avoid-build-deps --json > /tmp/lic.json
    python3 scripts/gerar-documentos.py /tmp/lic.json
"""
import json, sys, pathlib, html
import markdown

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEGAL = ROOT / "docs" / "legal"

TOOLS = [
    # nome, licença, uso, edições, observação PT, observação EN
    ("FFmpeg (build \"essentials\" de gyan.dev)", "GPL-3.0-or-later", "Store e direta",
     "Programa separado, executado pelo app. Código-fonte correspondente: https://ffmpeg.org/download.html e configuração da build em https://www.gyan.dev/ffmpeg/builds/ ; também enviamos uma cópia do código-fonte da versão incluída a quem pedir por e-mail, por 3 anos.",
     "Separate program run by the app. Corresponding source: https://ffmpeg.org/download.html and build configuration at https://www.gyan.dev/ffmpeg/builds/ ; we will also send a copy of the source of the included version to anyone who asks by e-mail, for 3 years."),
    ("whisper.cpp (ggml-org)", "MIT", "Store e direta", "https://github.com/ggml-org/whisper.cpp", "https://github.com/ggml-org/whisper.cpp"),
    ("llama.cpp e ggml (ggml-org), CPU e Vulkan", "MIT", "Store e direta", "https://github.com/ggml-org/llama.cpp", "https://github.com/ggml-org/llama.cpp"),
    ("yt-dlp", "Unlicense", "somente direta (download por link)", "https://github.com/yt-dlp/yt-dlp", "https://github.com/yt-dlp/yt-dlp"),
    ("Deno", "MIT", "somente direta (download por link)", "https://github.com/denoland/deno", "https://github.com/denoland/deno"),
    ("bgutil-ytdlp-pot-provider-rs", "conferir na fonte (projeto da comunidade)", "somente direta (download por link)",
     "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs — não incluído na edição Microsoft Store.",
     "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs — not included in the Microsoft Store edition."),
    ("Fonte Montserrat ExtraBold", "OFL-1.1", "Store e direta", "Ver Montserrat-OFL.txt", "See Montserrat-OFL.txt"),
    ("Modelo de detecção de rosto SeetaFace (via rustface)", "BSD-2-Clause", "Store e direta", "Ver SeetaFace-BSD-2-Clause.txt", "See SeetaFace-BSD-2-Clause.txt"),
]
MODELS = [
    ("Qwen3.5 2B / 4B / 9B (Alibaba Qwen), quantizações GGUF de bartowski", "Apache-2.0", "https://huggingface.co/Qwen"),
    ("Whisper (OpenAI), conversões ggml de whisper.cpp", "MIT", "https://huggingface.co/ggerganov/whisper.cpp"),
    ("Silero VAD (conversão ggml)", "MIT", "https://github.com/snakers4/silero-vad"),
]

def crates_table(lic_json):
    rows = {}
    for c in json.load(open(lic_json)):
        if c["name"].startswith(("bwsc-", "bwsmartclips")):
            continue
        rows.setdefault((c["name"], c.get("license") or "?"), set()).add(c["version"])
    lines = ["| Componente | Versão | Licença |", "|---|---|---|"]
    for (name, lic), vers in sorted(rows.items()):
        lines.append(f"| {name} | {', '.join(sorted(vers))} | {lic} |")
    return "\n".join(lines), len(rows)

def notices(lang, lic_json):
    table, n = crates_table(lic_json)
    pt = lang == "pt-BR"
    out = []
    if pt:
        out += ["# Avisos de licenças de terceiros", "",
                "O BWSmartClips é software proprietário. Ele usa os componentes de código aberto abaixo, cada um sob sua própria licença. "
                "Os textos completos das licenças acompanham o app na pasta `licenses` (e em `docs/legal/licencas` no código).", "",
                "Para componentes sob licenças que dão direitos ao usuário (por exemplo GPL e MPL), esses direitos continuam valendo integralmente.", "",
                "## Ferramentas incluídas ou baixadas pelo app", "", "| Ferramenta | Licença | Edição | Observação |", "|---|---|---|---|"]
        out += [f"| {t[0]} | {t[1]} | {t[2]} | {t[3]} |" for t in TOOLS]
        out += ["", "## Modelos de IA (baixados a seu pedido, não fazem parte do pacote)", "", "| Modelo | Licença | Origem |", "|---|---|---|"]
        out += [f"| {m[0]} | {m[1]} | {m[2]} |" for m in MODELS]
        out += ["", f"## Bibliotecas compiladas no app ({n})", "",
                "Bibliotecas Rust (crates.io). Quando há várias licenças separadas por OR, o componente é usado sob a licença permissiva (MIT ou Apache-2.0). "
                "Componentes MPL-2.0 são usados sem modificação; o código-fonte deles está em https://crates.io.", "", table]
    else:
        out += ["# Third-party license notices", "",
                "BWSmartClips is proprietary software. It uses the open-source components below, each under its own license. "
                "Full license texts ship with the app in the `licenses` folder.", "",
                "Rights granted to users by licenses such as GPL and MPL remain fully in effect for those components.", "",
                "## Tools included or downloaded by the app", "", "| Tool | License | Edition | Notes |", "|---|---|---|---|"]
        ed = {"Store e direta": "Store and direct", "somente direta (download por link)": "direct only (link download)"}
        out += [f"| {t[0]} | {t[1].replace('conferir na fonte (projeto da comunidade)', 'check upstream (community project)')} | {ed.get(t[2], t[2])} | {t[4]} |" for t in TOOLS]
        out += ["", "## AI models (downloaded on request, not part of the package)", "", "| Model | License | Source |", "|---|---|---|"]
        out += [f"| {m[0]} | {m[1]} | {m[2]} |" for m in MODELS]
        out += ["", f"## Libraries compiled into the app ({n})", "",
                "Rust libraries (crates.io). Where several licenses are separated by OR, the component is used under the permissive one (MIT or Apache-2.0). "
                "MPL-2.0 components are used unmodified; their source is available at https://crates.io.", "", table.replace("| Componente | Versão | Licença |", "| Component | Version | License |")]
    return "\n".join(out) + "\n"

def to_html(md_path, out_path):
    text = md_path.read_text(encoding="utf-8")
    body = markdown.markdown(text, extensions=["tables"])
    # links .md entre documentos viram .html
    body = body.replace('.md"', '.html"')
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(f'<!doctype html><html><head><meta charset="utf-8"><title>{html.escape(md_path.stem)}</title></head><body>{body}</body></html>\n', encoding="utf-8")

if __name__ == "__main__":
    lic = sys.argv[1] if len(sys.argv) > 1 else "/tmp/lic.json"
    (LEGAL / "pt-BR" / "avisos-de-terceiros.md").write_text(notices("pt-BR", lic), encoding="utf-8")
    (LEGAL / "en" / "third-party-notices.md").write_text(notices("en", lic), encoding="utf-8")
    for lang in ("pt-BR", "en"):
        for md in sorted((LEGAL / lang).glob("*.md")):
            to_html(md, ROOT / "ui" / "legal" / lang / (md.stem + ".html"))
    print("ok")
