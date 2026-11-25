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

function fillOptions(o) {
  $("opt-count").value = o.clip_count;
  $("opt-min").value = o.min_duration;
  $("opt-max").value = o.max_duration;
  $("opt-lang").value = o.language;
  $("opt-reframe").value = o.reframe;
  $("opt-focus").value = o.focus;
  const c = o.captions;
  $("cap-enabled").checked = c.enabled;
  $("cap-color").value = c.color;
  $("cap-highlight").value = c.highlight_color;
  $("cap-outline-color").value = c.outline_color;
  $("cap-size").value = c.font_size;
  $("cap-words").value = c.max_words;
  $("cap-position").value = c.position;
  $("cap-upper").checked = c.uppercase;
  $("cap-pop").checked = c.pop;
  updateCaptionPreview();
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
    captions: {
      ...base.captions,
      enabled: $("cap-enabled").checked,
      color: $("cap-color").value,
      highlight_color: $("cap-highlight").value,
      outline_color: $("cap-outline-color").value,
      font_size: parseInt($("cap-size").value, 10) || base.captions.font_size,
      max_words: parseInt($("cap-words").value, 10) || base.captions.max_words,
      position: $("cap-position").value,
      uppercase: $("cap-upper").checked,
      pop: $("cap-pop").checked,
    },
  };
}

function updateCaptionPreview() {
  const p = $("cap-preview");
  const upper = $("cap-upper").checked;
  const words = ["isso", "aqui", "é"].map((w) => (upper ? w.toUpperCase() : w));
  p.innerHTML = `<b>${words[0]}</b> <i>${words[1]}</i> <b>${words[2]}</b>`;
  p.querySelectorAll("b").forEach((b) => (b.style.color = $("cap-color").value));
  p.querySelector("i").style.color = $("cap-highlight").value;
  p.style.webkitTextStrokeColor = $("cap-outline-color").value;
  p.style.opacity = $("cap-enabled").checked ? 1 : 0.35;
}

["cap-color", "cap-highlight", "cap-outline-color", "cap-upper", "cap-enabled"].forEach((id) =>
  $(id).addEventListener("input", updateCaptionPreview)
);

// ------------------------------------------------------------- ferramentas

async function refreshTools() {
  const st = await invoke("tool_status");
  const t = st.tools;
  const needWhisper = settings.transcription === "local";
  const missing = [];
  if (!t.ffmpeg) missing.push("ffmpeg");
  if (!t.ffprobe) missing.push("ffprobe");
  if (!t.ytdlp) missing.push("yt-dlp (para links)");
  if (needWhisper && !t.whisper) missing.push("whisper-cli");
  if (needWhisper && !t.whisper_model) missing.push("modelo do Whisper");
  $("tools-missing").textContent = missing.join(", ");
  $("tools-banner").classList.toggle("hidden", missing.length === 0);
  $("btn-banner-install").classList.toggle("hidden", !st.installer_supported);

  const rows = [
    ["ffmpeg", t.ffmpeg], ["ffprobe", t.ffprobe], ["yt-dlp", t.ytdlp],
    ["whisper-cli", t.whisper], ["modelo Whisper", t.whisper_model],
  ];
  $("tools-table").innerHTML = rows
    .map(([n, p]) => `<div>${p ? '<span class="ok">✔</span>' : '<span class="bad">✘</span>'} ${n}</div>
      <div class="path">${p ? esc(p) : "não encontrado"}</div>`)
    .join("") + `<div class="muted">Pasta</div><div class="path">${esc(st.tools_dir)}</div>`;

  const sel = $("s-whisper-model");
  sel.innerHTML = `<option value="">Automático (maior modelo instalado)</option>` +
    st.installed_models.map((m) => `<option value="${esc(m)}">${esc(m.split(/[\\/]/).pop())}</option>`).join("");
  sel.value = settings.whisper_model || "";
  if (sel.value !== (settings.whisper_model || "")) {
    sel.insertAdjacentHTML("beforeend", `<option value="${esc(settings.whisper_model)}">${esc(settings.whisper_model)}</option>`);
    sel.value = settings.whisper_model;
  }

  const im = $("install-model");
  if (!im.options.length) {
    im.innerHTML = st.available_models.map(([n, d]) => `<option value="${n}">${n} — ${esc(d)}</option>`).join("") +
      `<option value="">(não baixar modelo)</option>`;
    im.value = "small";
  }
  ["btn-install", "btn-update"].forEach((id) => ($(id).disabled = !st.installer_supported || st.installing));
  if (!st.installer_supported) $("install-msg").textContent = "Instalação automática disponível apenas no Windows.";
  return st;
}

async function startInstall(force) {
  try {
    await invoke("install_tools", { model: $("install-model").value, force });
    $("install-bar").classList.remove("hidden");
    $("btn-install-cancel").classList.remove("hidden");
    $("btn-install").disabled = $("btn-update").disabled = true;
  } catch (e) {
    toast(String(e));
  }
}

listen("install://progress", (p) => {
  $("install-fill").style.width = `${Math.round(p.progress * 100)}%`;
  $("install-msg").textContent = `${p.message} ${Math.round(p.progress * 100)}%`;
});
listen("install://done", async () => {
  $("install-msg").textContent = "Ferramentas instaladas!";
  $("btn-install-cancel").classList.add("hidden");
  toast("Ferramentas instaladas ✔");
  await refreshTools();
});
listen("install://error", async (msg) => {
  $("install-msg").textContent = "Erro: " + msg;
  $("btn-install-cancel").classList.add("hidden");
  await refreshTools();
});

$("btn-install").onclick = () => startInstall(false);
$("btn-update").onclick = () => startInstall(true);
$("btn-install-cancel").onclick = () => invoke("cancel_install");
$("btn-banner-install").onclick = () => {
  openSettings("tab-tools");
  startInstall(false);
};

// ------------------------------------------------------------- configurações

const S_FIELDS = {
  "s-llm": "llm_provider", "s-anthropic-key": "anthropic_api_key", "s-anthropic-model": "anthropic_model",
  "s-anthropic-url": "anthropic_base_url", "s-openai-key": "openai_api_key", "s-openai-model": "openai_llm_model",
  "s-openai-url": "openai_base_url", "s-engine": "transcription", "s-whisper-threads": "whisper_threads",
  "s-openai-tmodel": "openai_transcription_model", "s-ffmpeg": "ffmpeg_path", "s-ffprobe": "ffprobe_path",
  "s-ytdlp": "ytdlp_path", "s-whisper": "whisper_path", "s-output": "output_dir", "s-height": "output_height",
  "s-preset": "x264_preset", "s-crf": "x264_crf",
};
const NUMERIC = new Set(["whisper_threads", "output_height", "x264_crf"]);

function fillSettings() {
  for (const [id, key] of Object.entries(S_FIELDS)) $(id).value = settings[key];
}

function readSettings() {
  const s = { ...settings };
  for (const [id, key] of Object.entries(S_FIELDS)) {
    const v = $(id).value;
    s[key] = NUMERIC.has(key) ? parseInt(v, 10) || 0 : v.trim();
  }
  s.whisper_model = $("s-whisper-model").value;
  return s;
}

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
  const p = await invoke("pick_file", { title: "Escolha o executável" });
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
    setTimeout(() => ($("settings-status").textContent = ""), 2000);
    await refreshTools();
  } catch (e) {
    $("settings-status").textContent = "Erro: " + e;
  }
};

$("btn-save-defaults").onclick = async () => {
  settings.defaults = readOptions();
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
  $("clips").innerHTML = project.clips
    .map((c, i) => {
      const dur = c.end - c.start;
      const cls = c.score >= 80 ? "hot" : c.score >= 60 ? "mid" : "";
      const tags = (c.hashtags || []).join(" ");
      return `<article class="clip">
        <div class="video-wrap">
          <video src="${fileSrc(c.file)}" ${c.thumbnail ? `poster="${fileSrc(c.thumbnail)}"` : ""} controls preload="metadata"></video>
          ${c.score > 0 ? `<span class="score ${cls}">🔥 ${Math.round(c.score)}</span>` : ""}
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
  await refreshTools();
  const needsKey = settings.llm_provider === "anthropic" ? !settings.anthropic_api_key : !settings.openai_api_key;
  if (needsKey) {
    openSettings("tab-ai");
    toast("Configure a chave da IA para começar", 4000);
  }
})();
