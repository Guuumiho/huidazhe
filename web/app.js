const invoke = window.__TAURI__?.core?.invoke;

const els = {
  apiUrl: document.querySelector('#api-url'),
  apiKey: document.querySelector('#api-key'),
  model: document.querySelector('#model'),
  themeSelect: document.querySelector('#theme-select'),
  toggleSettings: document.querySelector('#toggle-settings'),
  settingsPanel: document.querySelector('#settings-panel'),
  saveSettings: document.querySelector('#save-settings'),
  askForm: document.querySelector('#ask-form'),
  askButton: document.querySelector('#ask-button'),
  conciseToggle: document.querySelector('#concise-toggle'),
  questionInput: document.querySelector('#question-input'),
  configStatus: document.querySelector('#config-status'),
  formMessage: document.querySelector('#form-message'),
  chatList: document.querySelector('#chat-list'),
  emptyState: document.querySelector('#empty-state'),
};

const state = {
  records: [],
  expandedRawResponseIds: new Set(),
  conciseMode: false,
};

const CONCISE_PREFIX = '别讲废话别啰嗦，直指核心回答以下问题：\n';
const THEME_PRESETS = {
  'default-theme': {
    h1Size: '20px',
    h1Weight: '700',
    h1Color: '#5E5668',
    h2Size: '18px',
    h2Weight: '700',
    h2Color: '#6B6276',
    h3Size: '16px',
    h3Weight: '600',
    h3Color: '#7A7185',
    bodySize: '15px',
    bodyWeight: '400',
    bodyColor: '#4F4A57',
    strongSize: '15px',
    strongWeight: '700',
    strongColor: '#3B3641',
    dividerColor: '#D8D1E0',
    assistantBubble: '#F7F1E8',
    userBubble: '#BDB1A2',
  },
  'quiet-blue-purple-gray': {
    h1Size: '20px',
    h1Weight: '700',
    h1Color: '#5A6170',
    h2Size: '18px',
    h2Weight: '700',
    h2Color: '#697181',
    h3Size: '16px',
    h3Weight: '600',
    h3Color: '#7D8595',
    bodySize: '15px',
    bodyWeight: '400',
    bodyColor: '#4F5560',
    strongSize: '15px',
    strongWeight: '700',
    strongColor: '#3A3F48',
    dividerColor: '#D8DEE8',
    assistantBubble: '#F3F5F8',
    userBubble: '#959CA6',
  },
  'cream-macaron': {
    h1Size: '20px',
    h1Weight: '700',
    h1Color: '#8B5E5A',
    h2Size: '18px',
    h2Weight: '700',
    h2Color: '#A06F6A',
    h3Size: '16px',
    h3Weight: '600',
    h3Color: '#B38782',
    bodySize: '15px',
    bodyWeight: '400',
    bodyColor: '#6A5A57',
    strongSize: '15px',
    strongWeight: '700',
    strongColor: '#5A4744',
    dividerColor: '#E8D6D0',
    assistantBubble: '#FCF4F1',
    userBubble: '#BFADA6',
  },
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
    theme: els.themeSelect.value,
  };
}

function applySettings(settings) {
  els.apiUrl.value = settings.apiUrl || '';
  els.apiKey.value = settings.apiKey || '';
  els.model.value = settings.model || '';
  els.themeSelect.value = THEME_PRESETS[settings.theme] ? settings.theme : 'default-theme';
  applyTheme(els.themeSelect.value);

  if (settings.apiUrl && settings.apiKey) {
    setConfigStatus(true, 'API ready');
  } else {
    setConfigStatus(false, 'API not set');
  }
}

function applyTheme(themeKey) {
  const theme = THEME_PRESETS[themeKey] || THEME_PRESETS['default-theme'];
  const root = document.documentElement;

  root.style.setProperty('--assistant-bubble', theme.assistantBubble);
  root.style.setProperty('--user-bubble', theme.userBubble);
  root.style.setProperty('--markdown-h1-size', theme.h1Size);
  root.style.setProperty('--markdown-h1-weight', theme.h1Weight);
  root.style.setProperty('--markdown-h1-color', theme.h1Color);
  root.style.setProperty('--markdown-h2-size', theme.h2Size);
  root.style.setProperty('--markdown-h2-weight', theme.h2Weight);
  root.style.setProperty('--markdown-h2-color', theme.h2Color);
  root.style.setProperty('--markdown-h3-size', theme.h3Size);
  root.style.setProperty('--markdown-h3-weight', theme.h3Weight);
  root.style.setProperty('--markdown-h3-color', theme.h3Color);
  root.style.setProperty('--markdown-body-size', theme.bodySize);
  root.style.setProperty('--markdown-body-weight', theme.bodyWeight);
  root.style.setProperty('--markdown-body-color', theme.bodyColor);
  root.style.setProperty('--markdown-strong-size', theme.strongSize);
  root.style.setProperty('--markdown-strong-weight', theme.strongWeight);
  root.style.setProperty('--markdown-strong-color', theme.strongColor);
  root.style.setProperty('--markdown-divider-color', theme.dividerColor);
}

function renderConciseToggle() {
  els.conciseToggle.classList.toggle('active', state.conciseMode);
  els.conciseToggle.textContent = state.conciseMode ? '简洁回复: 开' : '简洁回复';
}

function createBubble(roleLabel, text, variant = 'assistant') {
  const row = document.createElement('article');
  row.className = 'chat-row';

  const role = document.createElement('div');
  role.className = 'chat-role';
  role.textContent = roleLabel;

  const bubble = document.createElement('div');
  bubble.className = `chat-bubble ${variant}`;
  renderBubbleContent(bubble, text, variant);

  row.append(role, bubble);
  return { row, bubble };
}

function appendInlineFormatted(target, text) {
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  parts.forEach((part) => {
    if (!part) {
      return;
    }
    if (part.startsWith('**') && part.endsWith('**') && part.length > 4) {
      const strong = document.createElement('strong');
      strong.textContent = part.slice(2, -2);
      target.appendChild(strong);
      return;
    }
    target.appendChild(document.createTextNode(part));
  });
}

function flushParagraph(paragraphLines, bubble) {
  if (!paragraphLines.length) {
    return;
  }
  const paragraph = document.createElement('p');
  paragraph.className = 'bubble-paragraph';
  appendInlineFormatted(paragraph, paragraphLines.join(' '));
  bubble.appendChild(paragraph);
  paragraphLines.length = 0;
}

function flushList(listItems, bubble) {
  if (!listItems.length) {
    return;
  }
  const list = document.createElement('ul');
  list.className = 'bubble-list';
  listItems.forEach((itemText) => {
    const item = document.createElement('li');
    appendInlineFormatted(item, itemText);
    list.appendChild(item);
  });
  bubble.appendChild(list);
  listItems.length = 0;
}

function renderBubbleContent(bubble, text, variant) {
  bubble.textContent = '';

  if (variant === 'user' || variant === 'error') {
    bubble.textContent = text;
    return;
  }

  const lines = text.replace(/\r\n/g, '\n').split('\n');
  const paragraphLines = [];
  const listItems = [];
  let inCodeBlock = false;
  let codeLines = [];

  lines.forEach((line) => {
    const trimmed = line.trim();

    if (trimmed.startsWith('```')) {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);

      if (inCodeBlock) {
        const pre = document.createElement('pre');
        pre.className = 'bubble-code';
        pre.textContent = codeLines.join('\n');
        bubble.appendChild(pre);
        codeLines = [];
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      return;
    }

    if (inCodeBlock) {
      codeLines.push(line);
      return;
    }

    if (!trimmed) {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);
      return;
    }

    if (trimmed.startsWith('### ')) {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);
      const title = document.createElement('h4');
      title.className = 'bubble-heading bubble-heading-small';
      appendInlineFormatted(title, trimmed.slice(4));
      bubble.appendChild(title);
      return;
    }

    if (trimmed.startsWith('## ')) {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);
      const title = document.createElement('h3');
      title.className = 'bubble-heading bubble-heading-medium';
      appendInlineFormatted(title, trimmed.slice(3));
      bubble.appendChild(title);
      return;
    }

    if (trimmed.startsWith('# ')) {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);
      const title = document.createElement('h2');
      title.className = 'bubble-heading bubble-heading-large';
      appendInlineFormatted(title, trimmed.slice(2));
      bubble.appendChild(title);
      return;
    }

    if (trimmed === '---') {
      flushParagraph(paragraphLines, bubble);
      flushList(listItems, bubble);
      const divider = document.createElement('hr');
      divider.className = 'bubble-divider';
      bubble.appendChild(divider);
      return;
    }

    if (trimmed.startsWith('- ')) {
      flushParagraph(paragraphLines, bubble);
      listItems.push(trimmed.slice(2));
      return;
    }

    flushList(listItems, bubble);
    paragraphLines.push(trimmed);
  });

  if (inCodeBlock) {
    const pre = document.createElement('pre');
    pre.className = 'bubble-code';
    pre.textContent = codeLines.join('\n');
    bubble.appendChild(pre);
  }

  flushParagraph(paragraphLines, bubble);
  flushList(listItems, bubble);

  if (!bubble.childNodes.length) {
    bubble.textContent = text;
  }
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
    const result = await invoke('ask', {
      question: draftQuestion,
      questionPrefix: state.conciseMode ? CONCISE_PREFIX : null,
    });
    state.records[state.records.length - 1] = result;
    renderRecords();
    setFormMessage('');
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
  renderConciseToggle();
  try {
    await loadSettings();
  } catch (error) {
    setConfigStatus(false, 'Settings load failed');
    setFormMessage(`Settings warning: ${String(error)}`, 'error');
  }

  try {
    await loadRecords();
  } catch (error) {
    setFormMessage(`History load failed: ${String(error)}`, 'error');
  }
}

els.conciseToggle.addEventListener('click', () => {
  state.conciseMode = !state.conciseMode;
  renderConciseToggle();
});

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

els.themeSelect.addEventListener('change', () => {
  applyTheme(els.themeSelect.value);
});

els.askForm.addEventListener('submit', askQuestion);

bootstrap();
