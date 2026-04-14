const invoke = window.__TAURI__?.core?.invoke;

const els = {
  apiUrl: document.querySelector('#api-url'),
  apiKey: document.querySelector('#api-key'),
  model: document.querySelector('#model'),
  toggleSettings: document.querySelector('#toggle-settings'),
  settingsPanel: document.querySelector('#settings-panel'),
  saveSettings: document.querySelector('#save-settings'),
  askForm: document.querySelector('#ask-form'),
  askButton: document.querySelector('#ask-button'),
  questionInput: document.querySelector('#question-input'),
  configStatus: document.querySelector('#config-status'),
  formMessage: document.querySelector('#form-message'),
  chatList: document.querySelector('#chat-list'),
  emptyState: document.querySelector('#empty-state'),
};

const state = {
  records: [],
  expandedRawResponseIds: new Set(),
};

function ensureTauri() {
  if (!invoke) {
    throw new Error('Tauri bridge is not available. Please run inside the desktop app.');
  }
}

function setFormMessage(message, kind = '') {
  els.formMessage.textContent = message || '';
  els.formMessage.className = `form-message${kind ? ` ${kind}` : ''}`;
}

function setConfigStatus(isReady, message) {
  els.configStatus.textContent = message;
  els.configStatus.className = `status-pill${isReady ? ' ready' : ' error'}`;
}

function formatTime(timestamp) {
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(timestamp));
  } catch (_) {
    return '';
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
  els.apiUrl.value = settings.apiUrl || '';
  els.apiKey.value = settings.apiKey || '';
  els.model.value = settings.model || '';

  if (settings.apiUrl && settings.apiKey) {
    setConfigStatus(true, 'API ready');
  } else {
    setConfigStatus(false, 'API not set');
  }
}

function createBubble(roleLabel, text, variant = 'assistant') {
  const row = document.createElement('article');
  row.className = 'chat-row';

  const role = document.createElement('div');
  role.className = 'chat-role';
  role.textContent = roleLabel;

  const bubble = document.createElement('div');
  bubble.className = `chat-bubble ${variant}`;
  bubble.textContent = text;

  row.append(role, bubble);
  return { row, bubble };
}

function createRawResponseSection(record) {
  if (!record.rawResponse) {
    return null;
  }

  const wrapper = document.createElement('div');
  const expanded = state.expandedRawResponseIds.has(record.id);

  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'response-toggle';
  toggle.textContent = expanded ? 'Hide response fields' : 'View response fields';
  toggle.addEventListener('click', () => {
    if (state.expandedRawResponseIds.has(record.id)) {
      state.expandedRawResponseIds.delete(record.id);
    } else {
      state.expandedRawResponseIds.add(record.id);
    }
    renderRecords();
  });

  wrapper.appendChild(toggle);

  if (expanded) {
    const raw = document.createElement('pre');
    raw.className = 'response-raw';
    raw.textContent = record.rawResponse;
    wrapper.appendChild(raw);
  }

  return wrapper;
}

function renderRecords() {
  els.chatList.innerHTML = '';
  els.emptyState.hidden = state.records.length > 0;

  state.records.forEach((record) => {
    const userPart = createBubble('User', record.question, 'user');
    els.chatList.appendChild(userPart.row);

    const text = record.status === 'error'
      ? (record.errorMessage || 'Request failed')
      : (record.answer || 'Thinking...');

    const assistantPart = createBubble(
      'Assistant',
      text,
      record.status === 'error' ? 'error' : 'assistant'
    );

    const meta = document.createElement('div');
    meta.className = 'chat-record-meta';
    const parts = [formatTime(record.createdAt)];
    if (record.model) {
      parts.push(record.model);
    }
    if (record.latencyMs !== null && record.latencyMs !== undefined) {
      parts.push(`${record.latencyMs} ms`);
    }
    meta.textContent = parts.filter(Boolean).join(' · ');
    assistantPart.bubble.appendChild(meta);

    const rawSection = createRawResponseSection(record);
    if (rawSection) {
      assistantPart.bubble.appendChild(rawSection);
    }

    els.chatList.appendChild(assistantPart.row);
  });

  els.chatList.scrollTop = els.chatList.scrollHeight;
}

async function loadSettings() {
  const settings = await invoke('load_settings');
  applySettings(settings);
}

async function saveSettings(showMessage = true) {
  const settings = collectSettings();
  const saved = await invoke('save_settings', { settings });
  applySettings(saved);
  if (showMessage) {
    setFormMessage('Settings saved', 'success');
  }
  return saved;
}

async function loadRecords() {
  state.records = await invoke('list_history_records');
  renderRecords();
}

async function askQuestion(event) {
  event.preventDefault();
  setFormMessage('');

  const draftQuestion = els.questionInput.value.trim();
  const settings = collectSettings();

  if (!settings.apiUrl || !settings.apiKey) {
    setConfigStatus(false, 'API not set');
    setFormMessage('Please save API URL and API key first.', 'error');
    if (els.settingsPanel.classList.contains('hidden')) {
      els.settingsPanel.classList.remove('hidden');
    }
    return;
  }

  if (!draftQuestion) {
    setFormMessage('Question cannot be empty.', 'error');
    return;
  }

  els.askButton.disabled = true;
  els.saveSettings.disabled = true;
  els.questionInput.value = '';

  const tempRecord = {
    id: `pending-${Date.now()}`,
    question: draftQuestion,
    answer: 'Thinking...',
    rawResponse: null,
    createdAt: Date.now(),
    model: settings.model || 'gpt-4.1-mini',
    latencyMs: null,
    status: 'success',
    errorMessage: null,
  };

  state.records.push(tempRecord);
  renderRecords();

  try {
    await saveSettings(false);
    const result = await invoke('ask', { question: draftQuestion });
    state.records[state.records.length - 1] = result;
    renderRecords();
    setFormMessage('Done', 'success');
  } catch (error) {
    state.records[state.records.length - 1] = {
      ...tempRecord,
      status: 'error',
      errorMessage: String(error),
      answer: '',
    };
    renderRecords();
    setFormMessage(String(error), 'error');
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
    setConfigStatus(false, 'Startup failed');
    setFormMessage(String(error), 'error');
  }
}

els.toggleSettings.addEventListener('click', () => {
  els.settingsPanel.classList.toggle('hidden');
});

els.saveSettings.addEventListener('click', async () => {
  setFormMessage('');
  try {
    await saveSettings(true);
  } catch (error) {
    setFormMessage(String(error), 'error');
  }
});

els.askForm.addEventListener('submit', askQuestion);

bootstrap();
