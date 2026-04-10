const invoke = window.__TAURI__?.core?.invoke;

const els = {
  apiUrl: document.querySelector("#api-url"),
  apiKey: document.querySelector("#api-key"),
  model: document.querySelector("#model"),
  saveSettings: document.querySelector("#save-settings"),
  refreshHistory: document.querySelector("#refresh-history"),
  askForm: document.querySelector("#ask-form"),
  askButton: document.querySelector("#ask-button"),
  questionInput: document.querySelector("#question-input"),
  currentQuestion: document.querySelector("#current-question"),
  currentAnswer: document.querySelector("#current-answer"),
  answerMeta: document.querySelector("#answer-meta"),
  answerTitle: document.querySelector("#answer-title"),
  configStatus: document.querySelector("#config-status"),
  formMessage: document.querySelector("#form-message"),
  historyList: document.querySelector("#history-list"),
  historyEmpty: document.querySelector("#history-empty"),
};

const state = {
  selectedHistoryId: null,
  history: [],
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

function statusLabel(status) {
  return status === "success" ? "成功" : "失败";
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
    setConfigStatus(false, "未配置 API");
  }
}

function renderCurrentRecord(record, title = "当前回答") {
  state.selectedHistoryId = record?.id ?? null;
  els.answerTitle.textContent = title;
  els.currentQuestion.textContent = record?.question || "在下方输入你的问题。";
  els.currentAnswer.textContent =
    record?.status === "error"
      ? record?.errorMessage || "请求失败"
      : record?.answer || "回答会显示在这里。";

  if (!record) {
    els.answerMeta.textContent = "准备就绪";
  } else {
    const metaParts = [formatTime(record.createdAt)];
    if (record.model) {
      metaParts.push(record.model);
    }
    if (record.latencyMs !== null && record.latencyMs !== undefined) {
      metaParts.push(`${record.latencyMs} ms`);
    }
    els.answerMeta.textContent = metaParts.filter(Boolean).join(" · ");
  }

  renderHistory();
}

function renderHistory() {
  els.historyList.innerHTML = "";
  els.historyEmpty.hidden = state.history.length > 0;

  state.history.forEach((item) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `history-item${item.id === state.selectedHistoryId ? " active" : ""}`;
    button.addEventListener("click", () => openHistoryItem(item.id));

    const question = document.createElement("p");
    question.className = "history-question";
    question.textContent = item.questionPreview || "空问题";

    const meta = document.createElement("div");
    meta.className = "history-meta";

    const time = document.createElement("span");
    time.textContent = formatTime(item.createdAt);

    const badge = document.createElement("span");
    badge.className = `history-status${item.status === "error" ? " error" : ""}`;
    badge.textContent = statusLabel(item.status);

    meta.append(time, badge);
    button.append(question, meta);
    els.historyList.appendChild(button);
  });
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

async function loadHistory() {
  state.history = await invoke("list_history");
  renderHistory();
}

async function openHistoryItem(id) {
  const item = await invoke("get_history_item", { id });
  renderCurrentRecord(item, "历史回答");
}

async function askQuestion(event) {
  event.preventDefault();
  setFormMessage("");

  const question = els.questionInput.value.trim();
  const settings = collectSettings();

  if (!settings.apiUrl || !settings.apiKey) {
    setConfigStatus(false, "请先填写 API");
    setFormMessage("请先填写并保存 API URL 和 API Key。", "error");
    return;
  }

  if (!question) {
    setFormMessage("问题不能为空。", "error");
    return;
  }

  els.askButton.disabled = true;
  els.saveSettings.disabled = true;
  els.answerTitle.textContent = "当前回答";
  els.currentQuestion.textContent = question;
  els.currentAnswer.textContent = "正在请求模型，请稍候...";
  els.answerMeta.textContent = "请求中";

  try {
    await saveSettings(false);
    const result = await invoke("ask", { question });
    els.questionInput.value = "";
    renderCurrentRecord(result, "当前回答");
    setFormMessage("提问完成，已保存到历史。", "success");
    await loadHistory();
  } catch (error) {
    els.currentAnswer.textContent = String(error);
    els.answerMeta.textContent = "请求失败";
    setFormMessage(String(error), "error");
    await loadHistory();
  } finally {
    els.askButton.disabled = false;
    els.saveSettings.disabled = false;
  }
}

async function bootstrap() {
  ensureTauri();
  try {
    await loadSettings();
    await loadHistory();
  } catch (error) {
    setConfigStatus(false, "启动失败");
    setFormMessage(String(error), "error");
  }
}

els.saveSettings.addEventListener("click", async () => {
  setFormMessage("");
  try {
    await saveSettings(true);
  } catch (error) {
    setFormMessage(String(error), "error");
  }
});

els.refreshHistory.addEventListener("click", async () => {
  setFormMessage("");
  try {
    await loadHistory();
  } catch (error) {
    setFormMessage(String(error), "error");
  }
});

els.askForm.addEventListener("submit", askQuestion);

bootstrap();
