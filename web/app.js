const invoke = window.__TAURI__?.core?.invoke;

const els = {
  apiUrl: document.querySelector("#api-url"),
  apiKey: document.querySelector("#api-key"),
  model: document.querySelector("#model"),
  toggleSettings: document.querySelector("#toggle-settings"),
  settingsPanel: document.querySelector("#settings-panel"),
  saveSettings: document.querySelector("#save-settings"),
  askForm: document.querySelector("#ask-form"),
  askButton: document.querySelector("#ask-button"),
  questionInput: document.querySelector("#question-input"),
  configStatus: document.querySelector("#config-status"),
  formMessage: document.querySelector("#form-message"),
  chatList: document.querySelector("#chat-list"),
  emptyState: document.querySelector("#empty-state"),
  chatMeta: document.querySelector("#chat-meta"),
};

const state = {
  records: [],
};

function ensureTauri() {
  if (!invoke) {
    throw new Error("Tauri bridge is not available. Please run inside the desktop app.");
  }
}

function setFormMessage(message, kind = "") {
  els.formMessage.textContent = message || "";
  els.formMessage.className = `form-message${kind ? ` ${kind}` : ""}`;
}

function setConfigStatus(isReady, message) {
  els.configStatus.textContent = message;
  els.configStatus.className = `status-pill${isReady ? " ready" : " error"}`;
}

function formatTime(timestamp) {
  try {
    return new Intl.DateTimeFormat("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(timestamp));
  } catch (_) {
    return "";
  }
}

function collectSettings() {
  return {
    apiUrl: els.apiUrl.value.trim(),
    apiKey: els.apiKey.value.trim(),
    model: els.model.value.trim(),
  };
}

function applySettings(settings) {
  els.apiUrl.value = settings.apiUrl || "";
  els.apiKey.value = settings.apiKey || "";
  els.model.value = settings.model || "";

  if (settings.apiUrl && settings.apiKey) {
    setConfigStatus(true, "API 已配置");
  } else {
    setConfigStatus(false, "请先在设置中配置 API");
  }
}

function setChatMeta(message) {
  els.chatMeta.textContent = message;
}

function createBubble(roleLabel, text, variant = "assistant") {
  const row = document.createElement("article");
  row.className = "chat-row";

  const role = document.createElement("div");
  role.className = "chat-role";
  role.textContent = roleLabel;

  const bubble = document.createElement("div");
  bubble.className = `chat-bubble ${variant}`;
  bubble.textContent = text;

  row.append(role, bubble);
  return row;
}

function renderRecords() {
  els.chatList.innerHTML = "";
  els.emptyState.hidden = state.records.length > 0;

  state.records.forEach((record) => {
    els.chatList.appendChild(createBubble("User", record.question, "user"));

    const assistantBubble = createBubble(
      "Assistant",
      record.status === "error" ? record.errorMessage || "请求失败" : record.answer,
      record.status === "error" ? "error" : "assistant"
    );

    const meta = document.createElement("div");
    meta.className = "chat-record-meta";
    const parts = [formatTime(record.createdAt)];
    if (record.model) {
      parts.push(record.model);
    }
    if (record.latencyMs !== null && record.latencyMs !== undefined) {
      parts.push(`${record.latencyMs} ms`);
    }
    meta.textContent = parts.filter(Boolean).join(" · ");
    assistantBubble.appendChild(meta);

    els.chatList.appendChild(assistantBubble);
  });

  els.chatList.scrollTop = els.chatList.scrollHeight;
}

async function loadSettings() {
  const settings = await invoke("load_settings");
  applySettings(settings);
}

async function saveSettings(showMessage = true) {
  const settings = collectSettings();
  const saved = await invoke("save_settings", { settings });
  applySettings(saved);
  if (showMessage) {
    setFormMessage("配置已保存", "success");
  }
  return saved;
}

async function loadRecords() {
  state.records = await invoke("list_history_records");
  renderRecords();
}

async function askQuestion(event) {
  event.preventDefault();
  setFormMessage("");

  const draftQuestion = els.questionInput.value.trim();
  const settings = collectSettings();

  if (!settings.apiUrl || !settings.apiKey) {
    setConfigStatus(false, "请先在设置中配置 API");
    setFormMessage("请先在设置中填写并保存 API URL 和 API Key。", "error");
    if (els.settingsPanel.classList.contains("hidden")) {
      els.settingsPanel.classList.remove("hidden");
    }
    return;
  }

  if (!draftQuestion) {
    setFormMessage("问题不能为空。", "error");
    return;
  }

  els.askButton.disabled = true;
  els.saveSettings.disabled = true;
  els.questionInput.value = "";
  setChatMeta("正在请求模型...");

  const tempRecord = {
    id: `pending-${Date.now()}`,
    question: draftQuestion,
    answer: "正在请求模型，请稍候...",
    createdAt: Date.now(),
    model: settings.model || "gpt-4.1-mini",
    latencyMs: null,
    status: "success",
    errorMessage: null,
  };

  state.records.push(tempRecord);
  renderRecords();

  try {
    await saveSettings(false);
    const result = await invoke("ask", { question: draftQuestion });
    state.records[state.records.length - 1] = result;
    renderRecords();
    setChatMeta("回答已更新");
    setFormMessage("提问完成", "success");
  } catch (error) {
    state.records[state.records.length - 1] = {
      ...tempRecord,
      status: "error",
      errorMessage: String(error),
      answer: "",
    };
    renderRecords();
    setChatMeta("请求失败");
    setFormMessage(String(error), "error");
  } finally {
    await loadRecords();
    els.askButton.disabled = false;
    els.saveSettings.disabled = false;
  }
}

async function bootstrap() {
  ensureTauri();
  try {
    await loadSettings();
    await loadRecords();
  } catch (error) {
    setConfigStatus(false, "启动失败");
    setFormMessage(String(error), "error");
  }
}

els.toggleSettings.addEventListener("click", () => {
  els.settingsPanel.classList.toggle("hidden");
});

els.saveSettings.addEventListener("click", async () => {
  setFormMessage("");
  try {
    await saveSettings(true);
  } catch (error) {
    setFormMessage(String(error), "error");
  }
});

els.askForm.addEventListener("submit", askQuestion);

bootstrap();
