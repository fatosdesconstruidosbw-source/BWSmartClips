// BWSmartClips — interface (JavaScript puro, sem build).
"use strict";

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const listen = (ev, cb) => T.event.listen(ev, (e) => cb(e.payload));
const fileSrc = (p) => T.core.convertFileSrc(p);

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

let settings = null;
let currentJob = null;
let currentProject = null;
const STAGES = ["download", "probe", "transcribe", "analyze", "render"];

function toast(msg, ms = 2600) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => t.classList.add("hidden"), ms);
}

function fmtTime(s) {
  s = Math.max(0, s);
  const m = Math.floor(s / 60);
  const r = Math.floor(s % 60);
  return `${m}:${String(r).padStart(2, "0")}`;
}

// ------------------------------------------------------------- opções do trabalho

// Estilos prontos de legenda. `box` = caixa atrás do texto; `up` = cor das palavras ainda não faladas.
const PRESETS = [
  { id: "none", name: "Sem legendas", enabled: false },
  { id: "viral", name: "Viral", color: "#FFFFFF", hl: "#FFE600", outline: "#000000", upper: true, italic: false, pop: true },
  { id: "karaoke", name: "Karaokê", color: "#FFFFFF", hl: "#22F04A", outline: "#000000", upper: true, italic: false, pop: false },
  { id: "impacto", name: "Impacto", color: "#FFFFFF", hl: "#39FF14", outline: "#000000", upper: true, italic: true, pop: true },
  { id: "caixa", name: "Caixa", color: "#111111", hl: "#111111", outline: "#FFFFFF", upper: false, italic: false, pop: false, box: "#FFFFFF", up: "#A3A3A3" },
  { id: "neon", name: "Neon", color: "#FF4FD8", hl: "#FFFFFF", outline: "#2A0029", upper: true, italic: false, pop: true },
  { id: "foco", name: "Foco", color: "#FFFFFF", hl: "#2EE6C5", outline: "#000000", upper: true, italic: false, pop: false, up: "#8E8E8E" },
  { id: "classico", name: "Clássico", color: "#FFFFFF", hl: "#FFFFFF", outline: "#000000", upper: false, italic: false, pop: false },
];
const SAMPLE = ["como", "viralizar", "rápido"];
let capState = {};

function applyPreset(id) {
  const p = PRESETS.find((x) => x.id === id);
  if (!p) return;
  capState.preset = id;
  capState.enabled = p.enabled !== false;
  if (p.enabled !== false) {
    Object.assign(capState, {
      color: p.color, highlight_color: p.hl, outline_color: p.outline, uppercase: p.upper,
      italic: p.italic, pop: p.pop, box_color: p.box || "", upcoming_color: p.up || "",
    });
  }
  syncCaptionInputs();
  renderPresets();
}

function syncCaptionInputs() {
  $("cap-enabled").checked = capState.enabled;
  $("cap-color").value = capState.color;
  $("cap-highlight").value = capState.highlight_color;
  $("cap-outline-color").value = capState.outline_color;
  $("cap-size").value = capState.font_size;
  $("cap-words").value = capState.max_words;
  $("cap-position").value = capState.position;
  $("cap-upper").checked = capState.uppercase;
  $("cap-italic").checked = capState.italic;
  $("cap-pop").checked = capState.pop;
}

// Ajuste manual em Fonte/Efeitos: deixa de ser um estilo pronto.
function readCaptionInputs() {
  Object.assign(capState, {
    enabled: $("cap-enabled").checked,
    color: $("cap-color").value,
    highlight_color: $("cap-highlight").value,
    outline_color: $("cap-outline-color").value,
    font_size: parseInt($("cap-size").value, 10) || capState.font_size,
    max_words: parseInt($("cap-words").value, 10) || capState.max_words,
    position: $("cap-position").value,
    uppercase: $("cap-upper").checked,
    italic: $("cap-italic").checked,
    pop: $("cap-pop").checked,
  });
}
["cap-color", "cap-highlight", "cap-outline-color", "cap-size", "cap-words", "cap-position", "cap-upper", "cap-italic", "cap-pop", "cap-enabled"]
  .forEach((id) => $(id).addEventListener("input", () => {
    readCaptionInputs();
    if (capState.preset !== "none") capState.preset = "custom";
    renderPresets();
  }));

/** HTML de uma frase de exemplo com a palavra `active` em destaque. */
function sampleHtml(s, active) {
  const style = (k) => {
    let c = s.color;
    if (k === active) c = s.highlight_color;
    else if (k > active && s.upcoming_color) c = s.upcoming_color;
    const pop = k === active && s.pop ? "transform:scale(1.12);margin:0 .32em;" : "";
    const stroke = s.box_color ? "" : `-webkit-text-stroke:1.2px ${s.outline_color};paint-order:stroke fill;text-shadow:0 2px 0 ${s.outline_color};`;
    return `color:${c};${pop}${stroke}${s.italic ? "font-style:italic;" : ""}`;
  };
  const words = SAMPLE.map((w, k) => `<span style="${style(k)}">${esc(s.uppercase ? w.toUpperCase() : w)}</span>`).join(" ");
  return s.box_color
    ? `<span class="line boxed" style="background:${s.box_color}">${words}</span>`
    : `<span class="line">${words}</span>`;
}

function renderPresets() {
  $("cap-presets").innerHTML = PRESETS.map((p) => {
    const s = p.enabled === false ? null : { color: p.color, highlight_color: p.hl, outline_color: p.outline, uppercase: p.upper, italic: p.italic, pop: p.pop, box_color: p.box || "", upcoming_color: p.up || "" };
    const pv = s ? sampleHtml(s, 1) : `<svg viewBox="0 0 24 24" style="width:22px;height:22px;fill:none;stroke:#a1a1aa;stroke-width:2"><circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/></svg>`;
    return `<button class="preset ${capState.preset === p.id ? "active" : ""}" data-preset="${p.id}"><div class="pv">${pv}</div><span class="nm">${p.name}</span></button>`;
  }).join("") + (capState.preset === "custom" ? `<p class="muted small" style="text-align:center">Estilo personalizado (Fonte/Efeitos)</p>` : "");
}
$("cap-presets").addEventListener("click", (e) => {
  const b = e.target.closest("[data-preset]");
  if (b) applyPreset(b.dataset.preset);
});

// Prévia animada no "celular".
let stageWord = 0;
function renderStage() {
  const el = $("stage-caption");
  el.className = `stage-caption pos-${capState.position || "bottom"}`;
  el.style.fontSize = `${Math.round(17 * (capState.font_size || 82) / 82)}px`;
  el.innerHTML = capState.enabled ? sampleHtml(capState, stageWord) : "";
  stageWord = (stageWord + 1) % SAMPLE.length;
}
setInterval(renderStage, 650);

document.querySelectorAll(".cap-tab").forEach((t) => (t.onclick = () => {
  document.querySelectorAll(".cap-tab").forEach((x) => x.classList.toggle("active", x === t));
  document.querySelectorAll(".cap-body").forEach((b) => b.classList.toggle("hidden", b.id !== t.dataset.cap));
}));

// Layouts de reenquadramento.
function setReframe(v) {
  $("opt-reframe").value = v;
  document.querySelectorAll(".layout").forEach((l) => l.classList.toggle("active", l.dataset.value === v));
  $("gameplay-opts").classList.toggle("hidden", v !== "gameplay");
}

// ---- layout Gameplay: tamanho e posição da webcam
let gpFacecam = null; // retângulo normalizado {x,y,w,h} ou null (automático)

function updateCamUi() {
  $("cam-ratio-label").textContent = `${$("opt-cam-ratio").value}%`;
  const manual = !!gpFacecam;
  $("cam-pos-text").innerHTML = manual
    ? `<span class="on">● Webcam marcada à mão</span> (vale para todos os cortes)`
    : "Webcam: detectada automaticamente em cada corte";
  $("btn-auto-cam").classList.toggle("hidden", !manual);
}
$("opt-cam-ratio").addEventListener("input", updateCamUi);
$("btn-auto-cam").onclick = () => { gpFacecam = null; updateCamUi(); };

let fcRect = null;
async function openFacecamModal() {
  const source = $("source").value.trim();
  if (!source) return toast("Escolha o vídeo primeiro (link ou arquivo).");
  $("facecam-modal").classList.remove("hidden");
  $("fc-loading").classList.remove("hidden");
  $("fc-loading").textContent = "Lendo o vídeo e procurando a webcam…";
  $("fc-box").classList.add("hidden");
  try {
    const r = await invoke("facecam_preview", { source });
    $("fc-img").src = fileSrc(r.image);
    fcAuto = r.detected;
    fcRect = gpFacecam || r.detected;
    $("fc-hint").textContent = r.detected
      ? "Esta é a webcam que o app encontrou. Se estiver errada, arraste para desenhar outra."
      : "Não encontrei a webcam automaticamente. Arraste para desenhar um retângulo em volta dela.";
    $("fc-loading").classList.add("hidden");
    drawFcBox();
  } catch (e) {
    $("fc-loading").textContent = String(e);
  }
}
let fcAuto = null;

function drawFcBox() {
  const b = $("fc-box");
  if (!fcRect) return b.classList.add("hidden");
  b.classList.remove("hidden");
  Object.assign(b.style, { left: `${fcRect.x * 100}%`, top: `${fcRect.y * 100}%`, width: `${fcRect.w * 100}%`, height: `${fcRect.h * 100}%` });
}

(() => {
  const stage = $("fc-stage");
  let drag = null;
  const pos = (e) => {
    const r = $("fc-img").getBoundingClientRect();
    return { x: Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), y: Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)) };
  };
  stage.addEventListener("mousedown", (e) => {
    if (!$("fc-loading").classList.contains("hidden")) return;
    const p = pos(e);
    const inside = fcRect && p.x > fcRect.x && p.x < fcRect.x + fcRect.w && p.y > fcRect.y && p.y < fcRect.y + fcRect.h;
    drag = inside ? { mode: "move", p, start: { ...fcRect } } : { mode: "draw", p };
    e.preventDefault();
  });
  window.addEventListener("mousemove", (e) => {
    if (!drag) return;
    const p = pos(e);
    if (drag.mode === "move") {
      const s = drag.start;
      fcRect = { ...s, x: Math.min(1 - s.w, Math.max(0, s.x + p.x - drag.p.x)), y: Math.min(1 - s.h, Math.max(0, s.y + p.y - drag.p.y)) };
    } else {
      fcRect = { x: Math.min(p.x, drag.p.x), y: Math.min(p.y, drag.p.y), w: Math.abs(p.x - drag.p.x), h: Math.abs(p.y - drag.p.y) };
    }
    drawFcBox();
  });
  window.addEventListener("mouseup", () => {
    if (drag && fcRect && (fcRect.w < 0.03 || fcRect.h < 0.03)) fcRect = drag.start || null;
    drag = null;
    drawFcBox();
  });
})();

$("btn-mark-cam").onclick = openFacecamModal;
$("fc-detect").onclick = () => {
  gpFacecam = null;
  $("facecam-modal").classList.add("hidden");
  updateCamUi();
  toast(fcAuto ? "Usando a detecção automática em cada corte" : "Detecção automática ativada (se não achar a webcam, o corte usa Preencher)");
};
$("fc-save").onclick = () => {
  if (!fcRect) return toast("Desenhe o retângulo em volta da webcam.");
  gpFacecam = { ...fcRect };
  $("facecam-modal").classList.add("hidden");
  updateCamUi();
  toast("Posição da webcam salva");
};
document.querySelectorAll(".layout").forEach((l) => (l.onclick = () => setReframe(l.dataset.value)));

// Atalhos dos "chips" de recursos.
document.querySelectorAll(".fchip").forEach((c) => (c.onclick = () => {
  document.querySelectorAll(".fchip").forEach((x) => x.classList.toggle("active", x === c));
  $(c.dataset.goto)?.scrollIntoView({ behavior: "smooth", block: "start" });
}));

function fillOptions(o) {
  $("opt-count").value = o.clip_count;
  $("opt-min").value = o.min_duration;
  $("opt-max").value = o.max_duration;
  $("opt-lang").value = o.language;
  $("opt-focus").value = o.focus;
  $("opt-reanalyze").checked = false;
  setReframe(o.reframe);
  $("opt-cam-ratio").value = Math.round((o.cam_ratio || 0.38) * 100);
  gpFacecam = o.facecam || null;
  updateCamUi();
  capState = { ...o.captions };
  if (!capState.enabled) capState.preset = "none";
  if (!capState.preset) capState.preset = "custom";
  syncCaptionInputs();
  renderPresets();
}

function readOptions() {
  const base = settings.defaults;
  return {
    ...base,
    clip_count: parseInt($("opt-count").value, 10) || base.clip_count,
    min_duration: parseFloat($("opt-min").value) || base.min_duration,
    max_duration: parseFloat($("opt-max").value) || base.max_duration,
    language: $("opt-lang").value,
    reframe: $("opt-reframe").value,
    focus: $("opt-focus").value.trim(),
    reanalyze: $("opt-reanalyze").checked,
    cam_ratio: parseInt($("opt-cam-ratio").value, 10) / 100,
    facecam: gpFacecam,
    captions: { ...base.captions, ...capState },
  };
}

// Arrastar e soltar um vídeo na janela.
const VIDEO_EXT = /\.(mp4|mov|mkv|webm|avi|m4v|flv|wmv|ts)$/i;
listen("tauri://drag-enter", () => $("dropbar").classList.add("dragging"));
listen("tauri://drag-leave", () => $("dropbar").classList.remove("dragging"));
listen("tauri://drag-drop", (p) => {
  $("dropbar").classList.remove("dragging");
  const file = (p.paths || []).find((f) => VIDEO_EXT.test(f));
  if (file) {
    $("source").value = file;
    toast("Vídeo pronto — clique em Gerar cortes");
  } else if ((p.paths || []).length) {
    toast("Solte um arquivo de vídeo (mp4, mov, mkv…)");
  }
});

function updateModePill() {
  const online = !["local", "ollama"].includes(settings.llm_provider) || settings.transcription !== "local";
  const pill = $("mode-pill");
  pill.textContent = online ? "● Usando API online" : "● 100% offline";
  pill.classList.toggle("online", online);
}


// ------------------------------------------------------------- ferramentas e modelos

let toolStatus = null;
const gb = (mb) => (mb >= 1024 ? `${(mb / 1024).toFixed(1).replace(".", ",")} GB` : `${mb} MB`);

function fillModelSelect(sel, installed, current, autoLabel) {
  sel.innerHTML = `<option value="">${autoLabel}</option>` +
    installed.map((m) => `<option value="${esc(m.path)}">${esc(m.name)} (${gb(m.size_mb)})</option>`).join("");
  sel.value = current || "";
  if (sel.value !== (current || "")) {
    sel.insertAdjacentHTML("beforeend", `<option value="${esc(current)}">${esc(current)}</option>`);
    sel.value = current;
  }
}

function fillCatalog(sel) {
  const cat = sel.id.endsWith("llm") ? toolStatus.llm_catalog : toolStatus.whisper_catalog;
  const prev = sel.dataset.filled ? sel.value : null;
  sel.innerHTML = cat.map((m) => `<option value="${m.id}">${esc(m.label)} · ${gb(m.size_mb)}</option>`).join("") +
    `<option value="">(não baixar)</option>`;
  sel.value = prev ?? (cat.find((m) => m.recommended)?.id ?? "");
  sel.dataset.filled = "1";
}

async function refreshTools() {
  const st = await invoke("tool_status");
  toolStatus = st;
  const t = st.tools;
  const localWhisper = settings.transcription === "local";
  const localAi = settings.llm_provider === "local";
  const missing = [];
  if (!t.ffmpeg) missing.push("ffmpeg");
  if (!t.ffprobe) missing.push("ffprobe");
  if (!t.ytdlp) missing.push("yt-dlp (para links)");
  if (localWhisper && !t.whisper) missing.push("whisper-cli");
  if (localWhisper && !t.whisper_model) missing.push("modelo do Whisper");
  if (localAi && !t.llama) missing.push("llama-server (IA local)");
  if (localAi && !t.llm_model) missing.push("modelo de IA local");
  if (localAi && settings.llm_gpu && st.tools_installer_supported && t.llama && !t.llama_gpu) missing.push("suporte à placa de vídeo");
  $("tools-missing").textContent = missing.join(", ");
  $("tools-banner").classList.toggle("hidden", missing.length === 0);

  const rows = [
    ["ffmpeg", t.ffmpeg], ["ffprobe", t.ffprobe], ["yt-dlp", t.ytdlp],
    ["whisper-cli", t.whisper], ["modelo Whisper", t.whisper_model], ["detector de voz", t.vad_model],
    ["llama-server", t.llama], ["llama-server (placa de vídeo)", t.llama_gpu], ["modelo de IA", t.llm_model],
  ];
  $("tools-table").innerHTML = rows
    .map(([n, p]) => `<div>${p ? '<span class="ok">✔</span>' : '<span class="bad">✘</span>'} ${n}</div>
      <div class="path">${p ? esc(p) : "não encontrado"}</div>`)
    .join("") +
    `<div class="muted">Ferramentas</div><div class="path">${esc(st.tools_dir)}</div>` +
    `<div class="muted">Modelos</div><div class="path">${esc(st.models_dir)}</div>`;

  fillModelSelect($("s-whisper-model"), st.whisper_models, settings.whisper_model, "Automático (maior modelo instalado)");
  fillModelSelect($("s-llm-model"), st.llm_models, settings.llm_model, "Automático (recomendado ou o menor instalado)");
  ["install-whisper", "install-llm", "setup-whisper", "setup-llm"].forEach((id) => fillCatalog($(id)));
  renderModelsDir(st);
  renderDataDir(st);
  refreshGpu();
  setInstalling(st.installing);
  $("btn-update").classList.toggle("hidden", !st.tools_installer_supported);
  updateEstimates();
  return st;
}

// Placa de vídeo para a IA local.
async function refreshGpu() {
  const el = $("gpu-status");
  el.textContent = "Procurando a placa de vídeo…";
  let g;
  try { g = await invoke("gpu_info"); } catch (_) { el.textContent = ""; return; }
  if (!g.installed) {
    el.innerHTML = toolStatus?.tools_installer_supported
      ? 'Suporte à placa de vídeo não instalado — use <b>Instalar o que falta</b> em Ferramentas e modelos.'
      : "Neste sistema, use um llama-server compilado com suporte à sua placa.";
  } else if (g.device) {
    const [name, mem] = g.device.split(" (");
    el.innerHTML = `Placa de vídeo encontrada: <b>${esc(name)}</b>${mem ? ` (${esc(mem.replace(")", "").replace("free", "livres"))})` : ""}`;
  } else {
    el.textContent = g.all.length
      ? "Só há vídeo integrado neste PC — a IA vai usar o processador."
      : "Nenhuma placa de vídeo compatível encontrada (atualize o driver de vídeo). A IA vai usar o processador.";
  }
}

// Pasta do programa e dados de uma instalação anterior que ficaram no disco C.
function renderDataDir(st) {
  $("data-dir").textContent = st.data_dir;
  const legacy = st.legacy_data;
  const html = legacy
    ? `<span>Ferramentas, modelos e cache de uma instalação anterior (<b>${gb(legacy[1])}</b>) ainda estão em
        <span class="path">${esc(legacy[0])}</span>. Eles não são usados nesta instalação.</span>
        <button class="primary" data-act="migrate">Mover para a pasta do programa</button>`
    : "";
  document.querySelectorAll("[data-legacy]").forEach((el) => {
    el.classList.toggle("hidden", !legacy);
    el.innerHTML = html;
    const b = el.querySelector('[data-act="migrate"]');
    if (b) b.onclick = async () => {
      try {
        await invoke("migrate_data");
        $("install-fill").style.width = "0%";
        setInstalling(true);
        if ($("settings-modal").classList.contains("hidden")) openSettings("tab-tools");
      } catch (e) {
        toast(String(e), 5000);
      }
    };
  });
  $("legacy-banner").classList.toggle("hidden", !legacy);
  if (legacy) $("tools-banner").classList.add("hidden");
  if (legacy) $("legacy-size").textContent = gb(legacy[1]);
}

// Pasta dos modelos (vazio = padrão, dentro dos dados do app).
function renderModelsDir(st) {
  const custom = (settings.models_dir || "").trim() && settings.models_dir.trim() !== st.default_models_dir;
  document.querySelectorAll("[data-models-dir]").forEach((box) => {
    box.innerHTML = `<div class="out-row">
        <button class="out-path" data-act="open" title="${esc(st.models_dir)}\n(clique para abrir)">
          <svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
          <span class="out-text">${esc(st.models_dir)}</span>
          <span class="out-tag ${custom ? "custom" : ""}">${custom ? "Escolhida" : "Padrão"}</span>
        </button>
        <button class="secondary" data-act="pick">Escolher pasta…</button>
      </div>
      ${custom ? '<button class="link out-reset" data-act="reset">Voltar para a pasta padrão</button>' : ""}
      ${st.free_space_mb != null ? `<span class="muted small">Espaço livre nesse disco: <b>${gb(st.free_space_mb)}</b></span>` : ""}`;
    box.querySelector('[data-act="pick"]').onclick = async () => {
      const p = await invoke("pick_folder");
      if (p) setModelsDir(p);
    };
    box.querySelector('[data-act="open"]').onclick = () => invoke("reveal", { path: st.models_dir }).catch(() => toast("A pasta será criada no primeiro download"));
    const r = box.querySelector('[data-act="reset"]');
    if (r) r.onclick = () => setModelsDir("");
  });
  const [n, mb] = st.leftover_models || [0, 0];
  document.querySelectorAll("[data-leftover]").forEach((el) => {
    el.classList.toggle("hidden", !(custom && n > 0));
    if (!(custom && n > 0)) return;
    el.innerHTML = `<span>Ainda há <b>${n} modelo(s)</b> (${gb(mb)}) na pasta padrão <span class="path">${esc(st.default_models_dir)}</span>.
      Eles continuam funcionando, mas ocupam espaço nesse disco.</span>
      <button class="primary" data-act="move">Mover para a nova pasta</button>`;
    el.querySelector('[data-act="move"]').onclick = async () => {
      try {
        await invoke("move_models");
        $("install-fill").style.width = "0%";
        setInstalling(true);
      } catch (e) {
        toast(String(e), 5000);
      }
    };
  });
}

async function setModelsDir(dir) {
  const prev = settings.models_dir;
  settings.models_dir = dir;
  try {
    await invoke("save_settings", { settings });
    settings = await invoke("get_settings");
  } catch (e) {
    settings.models_dir = prev;
    return toast(String(e), 6000);
  }
  await refreshTools();
  toast(dir ? "Os modelos agora ficam na pasta escolhida" : "Voltando para a pasta padrão dos modelos");
}

function installRequest(prefix, force = false) {
  return {
    whisper_model: $(`${prefix}-whisper`).value,
    llm_model: $(`${prefix}-llm`).value,
    vad: $(`${prefix}-vad`).checked,
    force,
  };
}

async function updateEstimates() {
  if (!toolStatus) return;
  const free = toolStatus.free_space_mb;
  for (const [prefix, out] of [["install", "install-estimate"], ["setup", "setup-space"]]) {
    const mb = await invoke("estimate_install", { request: installRequest(prefix) });
    const enough = free == null || free > mb + 1024;
    const el = $(out);
    el.innerHTML = mb > 0
      ? `A baixar: <b>${gb(mb)}</b>${free != null ? ` · Espaço livre: <b>${gb(free)}</b>` : ""}${enough ? "" : " — <b>espaço insuficiente!</b>"}`
      : "Tudo o que foi escolhido já está instalado ✔";
    el.classList.toggle("bad", !enough);
  }
}
["install-whisper", "install-llm", "install-vad", "setup-whisper", "setup-llm", "setup-vad"].forEach((id) =>
  $(id).addEventListener("change", updateEstimates)
);

function setInstalling(on) {
  for (const bar of ["install-bar", "setup-bar"]) $(bar).classList.toggle("hidden", !on);
  $("btn-install-cancel").classList.toggle("hidden", !on);
  ["btn-install", "btn-update", "btn-import", "setup-go", "setup-import"].forEach((id) => ($(id).disabled = on));
}

async function startInstall(prefix, force) {
  try {
    await invoke("install_tools", { request: installRequest(prefix, force) });
    $("install-fill").style.width = $("setup-fill").style.width = "0%";
    setInstalling(true);
  } catch (e) {
    toast(String(e));
  }
}

async function startImport() {
  try {
    if (await invoke("import_models")) setInstalling(true);
  } catch (e) {
    toast(String(e));
  }
}

listen("install://progress", (p) => {
  const pct = `${Math.round(p.progress * 100)}%`;
  $("install-fill").style.width = $("setup-fill").style.width = pct;
  $("install-msg").textContent = $("setup-msg").textContent = p.message;
});
listen("install://done", async () => {
  setInstalling(false);
  settings = await invoke("get_settings");
  $("install-msg").textContent = $("setup-msg").textContent = "Pronto ✔";
  toast("Pronto ✔");
  if (!$("setup-modal").classList.contains("hidden")) await finishSetup();
  await refreshTools();
});
listen("install://error", async (msg) => {
  setInstalling(false);
  $("install-msg").textContent = $("setup-msg").textContent = "Erro: " + msg;
  await refreshTools();
});

$("btn-install").onclick = () => startInstall("install", false);
$("btn-update").onclick = () => startInstall("install", true);
$("btn-import").onclick = startImport;
$("btn-install-cancel").onclick = () => invoke("cancel_install");
$("btn-banner-install").onclick = () => openSettings("tab-tools");
$("btn-legacy-open").onclick = () => openSettings("tab-tools");

// ------------------------------------------------------------- assistente

function openSetup() {
  $("setup-modal").classList.remove("hidden");
  updateEstimates();
}

async function finishSetup() {
  settings.setup_done = true;
  await invoke("save_settings", { settings });
  $("setup-modal").classList.add("hidden");
  maybeFirstTour();
}

$("setup-go").onclick = () => startInstall("setup", false);
$("setup-import").onclick = startImport;
$("setup-skip").onclick = finishSetup;

// ------------------------------------------------------------- configurações

const S_FIELDS = {
  "s-llm": "llm_provider", "s-llm-model": "llm_model", "s-llm-context": "llm_context", "s-llm-threads": "llm_threads",
  "s-llm-candidates": "llm_candidates", "s-ollama-url": "ollama_base_url", "s-ollama-model": "ollama_model",
  "s-anthropic-key": "anthropic_api_key", "s-anthropic-model": "anthropic_model",
  "s-anthropic-url": "anthropic_base_url", "s-openai-key": "openai_api_key", "s-openai-model": "openai_llm_model",
  "s-openai-url": "openai_base_url", "s-engine": "transcription", "s-whisper-model": "whisper_model",
  "s-whisper-threads": "whisper_threads", "s-openai-tmodel": "openai_transcription_model",
  "s-ffmpeg": "ffmpeg_path", "s-ffprobe": "ffprobe_path", "s-ytdlp": "ytdlp_path", "s-whisper": "whisper_path",
  "s-llama": "llama_path", "s-cookies": "youtube_cookies_file", "s-cookies-browser": "cookies_browser", "s-output": "output_dir", "s-height": "output_height",
  "s-preset": "x264_preset", "s-crf": "x264_crf",
};
const S_CHECKS = { "s-vad": "whisper_vad", "s-cache": "use_cache", "s-llm-gpu": "llm_gpu" };
const NUMERIC = new Set(["whisper_threads", "output_height", "x264_crf", "llm_context", "llm_threads", "llm_candidates"]);

function showProviderFields() {
  const p = $("s-llm").value;
  document.querySelectorAll("fieldset[data-provider]").forEach((f) => f.classList.toggle("hidden", f.dataset.provider !== p));
}
$("s-llm").addEventListener("change", showProviderFields);
$("s-llm-candidates").addEventListener("input", () => ($("s-llm-candidates-2").value = $("s-llm-candidates").value));
$("s-llm-candidates-2").addEventListener("input", () => ($("s-llm-candidates").value = $("s-llm-candidates-2").value));

function fillSettings() {
  for (const [id, key] of Object.entries(S_FIELDS)) $(id).value = settings[key];
  for (const [id, key] of Object.entries(S_CHECKS)) $(id).checked = !!settings[key];
  $("s-llm-candidates-2").value = settings.llm_candidates;
  showProviderFields();
}

function readSettings() {
  const s = { ...settings };
  for (const [id, key] of Object.entries(S_FIELDS)) {
    const v = $(id).value;
    s[key] = NUMERIC.has(key) ? parseInt(v, 10) || 0 : v.trim();
  }
  for (const [id, key] of Object.entries(S_CHECKS)) s[key] = $(id).checked;
  if (s.llm_candidates < 4) s.llm_candidates = 16;
  if (s.llm_context < 2048) s.llm_context = 8192;
  return s;
}

$("btn-clear-cache").onclick = async () => {
  try {
    await invoke("clear_cache");
    toast("Cache limpo");
  } catch (e) {
    toast(String(e));
  }
};

function openSettings(tab) {
  fillSettings();
  refreshAccounts();
  refreshTools();
  $("settings-modal").classList.remove("hidden");
  if (tab) selectTab(tab);
}

function selectTab(id) {
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === id));
  document.querySelectorAll(".tab-body").forEach((b) => b.classList.toggle("hidden", b.id !== id));
}

document.querySelectorAll(".tab").forEach((t) => (t.onclick = () => selectTab(t.dataset.tab)));
document.querySelectorAll("[data-close]").forEach((b) => (b.onclick = () => b.closest(".modal").classList.add("hidden")));
document.querySelectorAll(".modal").forEach((m) => m.addEventListener("click", (e) => { if (e.target === m) m.classList.add("hidden"); }));
document.querySelectorAll("[data-pick]").forEach((b) => (b.onclick = async () => {
  const p = await invoke("pick_file", { title: b.dataset.title || "Escolha o executável" });
  if (p) $(b.dataset.pick).value = p;
}));
$("btn-pick-output").onclick = async () => {
  const p = await invoke("pick_folder");
  if (p) $("s-output").value = p;
};

// Pasta onde os cortes são salvos (vazio = padrão do app).
let defaultOutput = "";
function updateOutputUi() {
  let custom = (settings.output_dir || "").trim();
  if (custom === defaultOutput) custom = "";
  const shown = custom || defaultOutput;
  $("out-text").textContent = shown;
  $("out-path").title = shown + "\n(clique para abrir)";
  $("out-tag").textContent = custom ? "Escolhida" : "Padrão";
  $("out-tag").classList.toggle("custom", !!custom);
  $("btn-out-default").classList.toggle("hidden", !custom);
}
async function setOutputDir(dir) {
  settings.output_dir = dir;
  try {
    await invoke("save_settings", { settings });
    settings = await invoke("get_settings");
  } catch (e) {
    return toast("Não consegui salvar a pasta: " + e, 5000);
  }
  updateOutputUi();
  toast(dir ? "Os próximos cortes vão para a pasta escolhida" : "Voltando para a pasta padrão do app");
}
$("btn-out-pick").onclick = async () => {
  const p = await invoke("pick_folder");
  if (p) setOutputDir(p);
};
$("btn-out-default").onclick = () => setOutputDir("");
$("out-path").onclick = () => invoke("open_output_dir").catch((e) => toast("Não consegui abrir a pasta: " + e, 5000));

// Botão flutuante "voltar ao topo".
const toTop = $("btn-top");
const syncTop = () => toTop.classList.toggle("show", window.scrollY > 400);
window.addEventListener("scroll", syncTop, { passive: true });
toTop.onclick = () => window.scrollTo({ top: 0, behavior: "smooth" });

$("btn-settings").onclick = () => openSettings();
$("btn-save-settings").onclick = async () => {
  try {
    settings = readSettings();
    await invoke("save_settings", { settings });
    settings = await invoke("get_settings");
    updateOutputUi();
    $("settings-status").textContent = "Salvo ✔";
    updateModePill();
    setTimeout(() => ($("settings-status").textContent = ""), 2000);
    await refreshTools();
  } catch (e) {
    $("settings-status").textContent = "Erro: " + e;
  }
};

$("btn-save-defaults").onclick = async () => {
  settings.defaults = { ...readOptions(), reanalyze: false };
  await invoke("save_settings", { settings });
  toast("Opções salvas como padrão");
};

// ------------------------------------------------------------- trabalho

$("btn-pick").onclick = async () => {
  const p = await invoke("pick_video");
  if (p) $("source").value = p;
};

function setStage(stage, isUrl) {
  const idx = STAGES.indexOf(stage);
  document.querySelectorAll("#stages li").forEach((li, i) => {
    li.classList.toggle("active", i === idx);
    li.classList.toggle("done", stage === "done" || i < idx);
    li.classList.toggle("skip", li.dataset.stage === "download" && !isUrl);
  });
}

$("btn-start").onclick = async () => {
  const source = $("source").value.trim();
  if (!source) return toast("Cole um link ou escolha um arquivo.");
  const opts = readOptions();
  if (opts.min_duration >= opts.max_duration) return toast("A duração mínima deve ser menor que a máxima.");
  try {
    const isUrl = /^https?:\/\//i.test(source);
    currentJob = await invoke("start_job", { source, options: opts });
    $("progress-card").classList.remove("hidden");
    $("progress-error").classList.add("hidden");
    $("yt-login-actions").classList.add("hidden");
    $("log-actions").classList.add("hidden");
    $("progress-title").textContent = "Processando…";
    $("btn-cancel").classList.remove("hidden");
    $("bar-fill").style.width = "0%";
    $("progress-msg").textContent = "Iniciando…";
    $("btn-start").disabled = true;
    $("results").classList.add("hidden");
    setStage(isUrl ? "download" : "probe", isUrl);
    currentJob = { id: currentJob, isUrl };
    setTimeout(() => $("progress-card").scrollIntoView({ behavior: "smooth", block: "start" }), 60);
  } catch (e) {
    toast(String(e), 5000);
  }
};

$("btn-open-log").onclick = async () => invoke("reveal", { path: await invoke("log_path") });
$("btn-cancel").onclick = () => currentJob && invoke("cancel_job", { id: currentJob.id });

listen("job://progress", (p) => {
  if (!currentJob || p.id !== currentJob.id) return;
  $("bar-fill").style.width = `${(p.overall * 100).toFixed(1)}%`;
  $("progress-msg").textContent = p.message;
  setStage(p.stage, currentJob.isUrl);
});

listen("job://done", (d) => {
  if (!currentJob || d.id !== currentJob.id) return;
  $("progress-title").textContent = "Pronto! 🎉";
  $("bar-fill").style.width = "100%";
  $("progress-msg").textContent = `${d.result.clips.length} cortes gerados em ${d.result.project_dir}`;
  $("btn-cancel").classList.add("hidden");
  $("btn-start").disabled = false;
  setStage("done", currentJob.isUrl);
  currentJob = null;
  showResults(d.result);
});

listen("job://error", (d) => {
  if (!currentJob || d.id !== currentJob.id) return;
  $("btn-start").disabled = false;
  $("btn-cancel").classList.add("hidden");
  if (d.cancelled) {
    $("progress-title").textContent = "Cancelado";
    $("progress-msg").textContent = "";
  } else {
    const needsLogin = (d.kind || "").startsWith("login:");
    const NAMES = { youtube: "YouTube", twitch: "Twitch", kick: "Kick" };
    if (needsLogin) {
      loginPlatform = d.kind.slice(6);
      $("btn-yt-login-retry").textContent = `Entrar no ${NAMES[loginPlatform] || loginPlatform} e tentar de novo`;
    }
    $("progress-title").textContent = needsLogin ? `O ${NAMES[loginPlatform] || "site"} pediu login` : "Algo deu errado";
    $("progress-error").textContent = d.message;
    $("progress-error").classList.remove("hidden");
    $("yt-login-actions").classList.toggle("hidden", !needsLogin);
    $("log-actions").classList.toggle("hidden", needsLogin);
  }
  currentJob = null;
});

// ------------------------------------------------------------- resultados

function showResults(project) {
  currentProject = project;
  $("results").classList.remove("hidden");
  $("results-title").textContent = `${project.clips.length} cortes · ${project.source_title}`;
  const warn = project.warnings || [];
  $("results-warnings").innerHTML = warn.map((w) => `<p>⚠ ${esc(w)}</p>`).join("");
  $("results-warnings").classList.toggle("hidden", warn.length === 0);
  $("clips").innerHTML = project.clips
    .map((c, i) => {
      const dur = c.end - c.start;
      const cls = c.score >= 80 ? "" : c.score >= 60 ? "mid" : "low";
      const tags = (c.hashtags || []).join(" ");
      return `<article class="clip">
        <div class="video-wrap">
          <video src="${fileSrc(c.file)}" ${c.thumbnail ? `poster="${fileSrc(c.thumbnail)}"` : ""} controls preload="metadata"></video>
          ${c.score > 0 ? `<span class="score ${cls}"><small>NOTA</small><b>${Math.round(c.score)}</b></span>` : ""}
        </div>
        <div class="body">
          <h3>${esc(c.title)}</h3>
          <div class="meta">${fmtTime(c.start)} – ${fmtTime(c.end)} · ${Math.round(dur)}s</div>
          ${c.reason ? `<div class="reason">${esc(c.reason)}</div>` : ""}
          ${tags ? `<div class="tags">${esc(tags)}</div>` : ""}
          ${c.title || c.reason ? `<div class="ai-note">Título, nota e hashtags gerados por IA — revise antes de publicar ·
            <button class="link" data-act="report" data-i="${i}">reportar</button></div>` : ""}
          <div class="row">
            <button class="secondary" data-act="copy" data-i="${i}">📋 Legenda</button>
            <button class="secondary" data-act="reveal" data-i="${i}">📂 Pasta</button>
            <button class="secondary" data-act="open" data-i="${i}">▶ Abrir</button>
          </div>
        </div>
      </article>`;
    })
    .join("");
  $("clips").querySelectorAll("video").forEach((v, i) => {
    v.addEventListener("loadedmetadata", () => v.closest(".clip").classList.toggle("wide", v.videoWidth > v.videoHeight));
    // Se o WebView não conseguir tocar o arquivo, mostra a miniatura e abre no player do sistema.
    v.addEventListener("error", () => {
      const c = project.clips[i];
      const img = document.createElement("img");
      img.className = "fallback";
      img.title = "Abrir no player";
      if (c.thumbnail) img.src = fileSrc(c.thumbnail);
      img.onclick = () => invoke("open_file", { path: c.file });
      v.replaceWith(img);
    });
  });
  $("results").scrollIntoView({ behavior: "smooth", block: "start" });
}

$("clips").addEventListener("click", async (e) => {
  const b = e.target.closest("button[data-act]");
  if (!b || !currentProject) return;
  const c = currentProject.clips[+b.dataset.i];
  try {
    if (b.dataset.act === "report") return reportAi(`${c.title}\n${c.reason || ""}\n${(c.hashtags || []).join(" ")}`);
    if (b.dataset.act === "reveal") await invoke("reveal", { path: c.file });
    if (b.dataset.act === "open") await invoke("open_file", { path: c.file });
    if (b.dataset.act === "copy") {
      await navigator.clipboard.writeText(`${c.title}\n\n${(c.hashtags || []).join(" ")}`);
      toast("Título e hashtags copiados");
    }
  } catch (err) {
    toast(String(err));
  }
});

$("btn-open-folder").onclick = () => currentProject && invoke("reveal", { path: currentProject.project_dir });

$("btn-projects").onclick = async () => {
  const list = await invoke("list_projects");
  $("projects-list").innerHTML = list.length
    ? list.map((p, i) => `<div class="project" data-i="${i}">
        ${p.clips[0]?.thumbnail ? `<img src="${fileSrc(p.clips[0].thumbnail)}" />` : `<img />`}
        <div class="info"><b>${esc(p.source_title)}</b>
          <span class="muted small">${p.clips.length} cortes · ${esc(p.project_dir)}</span></div>
      </div>`).join("")
    : `<p class="muted">Nenhum projeto ainda.</p>`;
  $("projects-list").querySelectorAll(".project").forEach((el) => (el.onclick = () => {
    $("projects-modal").classList.add("hidden");
    showResults(list[+el.dataset.i]);
  }));
  $("projects-modal").classList.remove("hidden");
};

// ------------------------------------------------------------- contas (YouTube, Twitch, Kick)

let retryAfterLogin = null;
const ACC_ICON = { youtube: "▶", twitch: "T", kick: "K" };

async function refreshAccounts() {
  const list = await invoke("account_status");
  $("accounts-list").innerHTML = list.map((a) => `
    <div class="account">
      <div class="acc-logo ${a.id}">${ACC_ICON[a.id] || "•"}</div>
      <div class="acc-info"><b>${a.name}</b>
        <span class="${a.connected ? "on" : ""}">${a.connected ? "● Conectado" : "Não conectado"}</span></div>
      <button class="${a.connected ? "secondary" : "primary"}" data-login="${a.id}">${a.connected ? "Entrar de novo" : "Entrar"}</button>
      ${a.connected ? `<button class="ghost danger" data-logout="${a.id}">Desconectar</button>` : ""}
    </div>`).join("");
}

async function openLogin(platform, retry) {
  retryAfterLogin = retry ? platform : null;
  try {
    await invoke("account_login", { platform });
    toast("Faça login na janela que abriu. Ela fecha sozinha ao terminar.", 5000);
  } catch (e) {
    toast(String(e), 5000);
  }
}

$("accounts-list").addEventListener("click", async (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  if (b.dataset.login) openLogin(b.dataset.login, false);
  if (b.dataset.logout) {
    await invoke("account_logout", { platform: b.dataset.logout });
    toast("Conta desconectada");
    refreshAccounts();
  }
});

let loginPlatform = "youtube";
$("btn-yt-login-retry").onclick = () => openLogin(loginPlatform, true);

listen("account://login", async (ev) => {
  toast(ev.message, 4000);
  await refreshAccounts();
  if (ev.ok && retryAfterLogin === ev.platform) {
    retryAfterLogin = null;
    $("btn-start").click();
  }
});

// ------------------------------------------------------------- tour "Como funciona"

const TOUR = [
  { el: "#dropbar", title: "Escolha o vídeo",
    text: "Cole o link de um vídeo do <b>YouTube</b>, de um VOD ou clipe da <b>Twitch</b> ou da <b>Kick</b>. Também dá para usar um arquivo do computador: clique em <b>Carregar arquivo</b> ou arraste o vídeo para a janela." },
  { el: "#sec-clips .panel", title: "Diga o que você quer",
    text: "Escolha <b>quantos cortes</b> quer e a <b>duração</b> de cada um. No campo de foco, peça algo específico, como <i>momentos engraçados</i> ou <i>dicas práticas</i>, e a IA prioriza isso." },
  { el: "#sec-reframe", title: "Formato vertical",
    text: "<b>Seguir rosto</b> mantém quem está falando no centro do vídeo 9:16. Para lives de jogos, use <b>Gameplay</b>: o app acha a webcam do streamer e coloca ela em cima, com o jogo embaixo. Também há <b>Preencher</b>, <b>Fundo desfocado</b> e <b>Original</b>." },
  { el: "#sec-captions", title: "Legendas animadas",
    text: "Escolha um <b>estilo</b> pronto e veja a prévia no celular. Nas abas <b>Fonte</b> e <b>Efeitos</b> você ajusta cores, tamanho, posição e quantas palavras aparecem por vez." },
  { el: "#btn-start", title: "Gere os cortes",
    text: "Clique em <b>Gerar cortes</b>. O app baixa o vídeo, <b>transcreve</b> a fala, a <b>IA escolhe</b> os melhores momentos e <b>renderiza</b> cada corte, tudo no seu PC. Você acompanha cada etapa e pode cancelar quando quiser." },
  { el: "#btn-projects", title: "Seus cortes",
    text: "Os cortes aparecem na tela com a <b>nota de viralidade</b>, título, motivo e hashtags. Em <b>Projetos</b> você reabre trabalhos anteriores; os arquivos ficam em <b>Vídeos\\BWSmartClips</b>." },
  { el: "#btn-settings", title: "Configurações",
    text: "Aqui você troca os modelos de IA e de transcrição, conecta suas contas do <b>YouTube</b>, <b>Twitch</b> ou <b>Kick</b> se a plataforma pedir login e, se quiser, ativa as APIs online (Claude/OpenAI). Tudo é opcional." },
  { el: "#mode-pill", title: "Pronto!",
    text: "Este selo mostra se o app está trabalhando <b>100% offline</b>. Você pode rever este guia quando quiser em <b>Como funciona</b>." },
];
let tourStep = 0;

function tourShow(i) {
  tourStep = i;
  const step = TOUR[i];
  const el = document.querySelector(step.el);
  if (!el || el.offsetParent === null) return tourShow(i + 1 < TOUR.length ? i + 1 : 0);
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  $("tour-count").textContent = `Passo ${i + 1} de ${TOUR.length}`;
  $("tour-title").textContent = step.title;
  $("tour-text").innerHTML = step.text;
  $("tour-dots").innerHTML = TOUR.map((_, k) => `<i class="${k === i ? "on" : ""}"></i>`).join("");
  $("tour-prev").style.visibility = i === 0 ? "hidden" : "visible";
  $("tour-next").textContent = i === TOUR.length - 1 ? "Concluir" : "Próximo";
  setTimeout(tourPlace, 380);
  tourPlace();
}

function tourPlace() {
  if ($("tour").classList.contains("hidden")) return;
  const el = document.querySelector(TOUR[tourStep].el);
  if (!el) return;
  const r = el.getBoundingClientRect();
  const pad = 8;
  Object.assign($("tour-spot").style, {
    top: `${r.top - pad}px`, left: `${r.left - pad}px`, width: `${r.width + pad * 2}px`, height: `${r.height + pad * 2}px`,
  });
  const card = $("tour-card");
  const cw = card.offsetWidth, ch = card.offsetHeight;
  const vw = window.innerWidth, vh = window.innerHeight;
  let top = r.bottom + 16;
  if (top + ch > vh - 12) top = r.top - ch - 16;           // não cabe embaixo → em cima
  if (top < 12) top = Math.min(vh - ch - 12, Math.max(12, r.top + 12)); // elemento grande → sobreposto
  let left = r.left + r.width / 2 - cw / 2;
  left = Math.max(12, Math.min(vw - cw - 12, left));
  card.style.top = `${top}px`;
  card.style.left = `${left}px`;
}

function tourStart() {
  document.querySelectorAll(".modal").forEach((m) => m.classList.add("hidden"));
  $("tour").classList.remove("hidden");
  tourShow(0);
}

function tourEnd() {
  $("tour").classList.add("hidden");
  try { localStorage.setItem("bwsc.tour_done", "1"); } catch (_) {}
  if (settings && !settings.tour_done) {
    settings.tour_done = true;
    invoke("save_settings", { settings }).catch(() => {});
  }
}

$("btn-tour").onclick = tourStart;
$("tour-next").onclick = () => (tourStep + 1 < TOUR.length ? tourShow(tourStep + 1) : tourEnd());
$("tour-prev").onclick = () => tourStep > 0 && tourShow(tourStep - 1);
$("tour-skip").onclick = tourEnd;
$("tour").addEventListener("click", (e) => { if (e.target.id === "tour") tourEnd(); });
window.addEventListener("resize", tourPlace);
window.addEventListener("scroll", tourPlace, { passive: true });
document.addEventListener("keydown", (e) => {
  if ($("tour").classList.contains("hidden")) return;
  if (e.key === "Escape") tourEnd();
  if (e.key === "ArrowRight" || e.key === "Enter") $("tour-next").click();
  if (e.key === "ArrowLeft") $("tour-prev").click();
});

function maybeFirstTour() {
  // Quem já usava o app antes desta versão (setup feito) não vê o tour de novo.
  let done = !!settings.tour_done || !!settings.setup_done;
  try { done = done || localStorage.getItem("bwsc.tour_done") === "1"; } catch (_) {}
  if (!done) setTimeout(tourStart, 600);
}

// ------------------------------------------------------------- início

// ------------------------------------------------------------- edição, licença e documentos

// E-mail público de suporte (o mesmo informado no Partner Center). Também aparece nos documentos.
const SUPPORT_EMAIL = "";
let edition = null;

function openUrl(url) {
  return invoke("plugin:opener|open_url", { url }).catch((e) => toast(String(e), 5000));
}

function reportAi(text = "") {
  if (!SUPPORT_EMAIL) return toast("Canal de denúncia ainda não configurado.", 4000);
  const body = encodeURIComponent(`Conteúdo gerado pela IA que quero reportar:\n\n${text}\n\nMotivo:`);
  openUrl(`mailto:${SUPPORT_EMAIL}?subject=${encodeURIComponent("BWSmartClips — conteúdo da IA")}&body=${body}`);
}

async function loadEdition(recheck = false) {
  try {
    edition = await invoke("edition_info", { recheck });
  } catch (_) {
    return;
  }
  const e = edition.edition, lic = edition.license;
  $("about-version").textContent = "v" + edition.version;
  $("about-edition").textContent = e.store ? "Edição Microsoft Store" : "Edição direta (instalador próprio)";
  const box = $("about-license");
  box.className = "license-box " + (!lic.required ? "" : lic.ok ? "ok" : "bad");
  box.textContent = !lic.required
    ? "Esta edição não usa a licença da Microsoft Store."
    : lic.state === "ok" ? "Licença vitalícia ativa na Microsoft Store ✔"
    : lic.state === "trial" ? "Versão de avaliação da Microsoft Store."
    : lic.message;
  if (!e.link_download) {
    $("source").placeholder = "Arraste um vídeo para cá ou clique em Carregar arquivo";
    $("platforms").classList.add("hidden");
    document.querySelector("fieldset.accounts")?.classList.add("hidden");
    TOUR[0].text = "Use um vídeo do seu computador: clique em <b>Carregar arquivo</b> ou arraste o vídeo para a janela. " +
      "A gravação da sua live ou o vídeo do seu canal baixado pelo YouTube Studio servem.";
  }
  if (e.store) {
    // Na edição Store só rodam as ferramentas assinadas que vêm no pacote.
    $("manual-paths").classList.add("hidden");
    $("data-dir-hint").innerHTML = "Configurações, modelos baixados e cache ficam na pasta do app no Windows. Para levar tudo para outro disco, use " +
      "<b>Configurações do Windows → Aplicativos → BWSmartClips → Mover</b>. Os modelos também podem ficar em outra pasta (abaixo).";
  }
  const locked = lic.required && !lic.ok;
  $("license-lock").classList.toggle("hidden", !locked);
  if (locked) $("lock-msg").textContent = lic.message;
  let pill = document.querySelector(".trial-pill");
  if (lic.trial && !pill) {
    pill = document.createElement("span");
    pill.className = "trial-pill";
    pill.textContent = "Avaliação";
    $("mode-pill").after(pill);
    pill.onclick = () => invoke("open_store");
  }
}

$("btn-lock-store").onclick = () => invoke("open_store").catch((e) => toast(String(e), 5000));
$("btn-lock-retry").onclick = async () => {
  $("btn-lock-retry").disabled = true;
  await loadEdition(true);
  $("btn-lock-retry").disabled = false;
  if (!$("license-lock").classList.contains("hidden")) toast("Licença ainda não encontrada.", 4000);
};
$("btn-report-ai").onclick = () => reportAi();

document.querySelectorAll("[data-doc]").forEach((b) => (b.onclick = async () => {
  const lang = b.dataset.lang || "pt-BR";
  try {
    const r = await fetch(`legal/${lang}/${b.dataset.doc}.html`);
    if (!r.ok) throw new Error(r.status);
    const html = await r.text();
    const doc = new DOMParser().parseFromString(html, "text/html");
    $("doc-title").textContent = b.textContent.trim();
    $("doc-body").replaceChildren(...doc.body.childNodes);
    $("doc-modal").classList.remove("hidden");
  } catch (_) {
    toast("Documento não encontrado nesta versão.", 4000);
  }
}));

(async function init() {
  loadEdition();
  settings = await invoke("get_settings");
  fillOptions(settings.defaults);
  defaultOutput = await invoke("default_output_dir");
  updateOutputUi();
  syncTop();
  updateModePill();
  await refreshTools();
  if (!settings.setup_done) {
    openSetup();
    return;
  }
  maybeFirstTour();
  const cloud = { anthropic: "anthropic_api_key", open_ai: "openai_api_key" }[settings.llm_provider];
  if (cloud && !settings[cloud]) {
    openSettings("tab-ai");
    toast("Configure a chave da API ou volte para a IA local", 4000);
  }
})();
