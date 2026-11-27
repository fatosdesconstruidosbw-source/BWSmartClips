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
}
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
  $("tools-missing").textContent = missing.join(", ");
  $("tools-banner").classList.toggle("hidden", missing.length === 0);

  const rows = [
    ["ffmpeg", t.ffmpeg], ["ffprobe", t.ffprobe], ["yt-dlp", t.ytdlp],
    ["whisper-cli", t.whisper], ["modelo Whisper", t.whisper_model], ["detector de voz", t.vad_model],
    ["llama-server", t.llama], ["modelo de IA", t.llm_model],
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
  setInstalling(st.installing);
  $("btn-update").classList.toggle("hidden", !st.tools_installer_supported);
  updateEstimates();
  return st;
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
  $("install-msg").textContent = $("setup-msg").textContent = "Pronto ✔";
  toast("Instalação concluída ✔");
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

// ------------------------------------------------------------- assistente

function openSetup() {
  $("setup-modal").classList.remove("hidden");
  updateEstimates();
}

async function finishSetup() {
  settings.setup_done = true;
  await invoke("save_settings", { settings });
  $("setup-modal").classList.add("hidden");
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
const S_CHECKS = { "s-vad": "whisper_vad", "s-cache": "use_cache" };
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

$("btn-settings").onclick = () => openSettings();
$("btn-save-settings").onclick = async () => {
  try {
    settings = readSettings();
    await invoke("save_settings", { settings });
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
    $("progress-title").textContent = "Processando…";
    $("btn-cancel").classList.remove("hidden");
    $("bar-fill").style.width = "0%";
    $("progress-msg").textContent = "Iniciando…";
    $("btn-start").disabled = true;
    $("results").classList.add("hidden");
    setStage(isUrl ? "download" : "probe", isUrl);
    currentJob = { id: currentJob, isUrl };
  } catch (e) {
    toast(String(e), 5000);
  }
};

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
    $("progress-title").textContent = "Algo deu errado";
    $("progress-error").textContent = d.message;
    $("progress-error").classList.remove("hidden");
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

// ------------------------------------------------------------- início

(async function init() {
  settings = await invoke("get_settings");
  fillOptions(settings.defaults);
  updateModePill();
  await refreshTools();
  if (!settings.setup_done) {
    openSetup();
    return;
  }
  const cloud = { anthropic: "anthropic_api_key", open_ai: "openai_api_key" }[settings.llm_provider];
  if (cloud && !settings[cloud]) {
    openSettings("tab-ai");
    toast("Configure a chave da API ou volte para a IA local", 4000);
  }
})();
