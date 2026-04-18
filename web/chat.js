import { state } from './state.js';
import { loadKnowledgeStatus } from './knowledge.js';
import { collectSettings, saveSettings } from './settings.js';
import {
  els,
  formatTime,
  hideConversationModeModal,
  invoke,
  renderConversationDeleteToggle,
  resetQuestionInputHeight,
  resizeQuestionInput,
  setConfigStatus,
  setFormMessage,
  setMemoryMode,
  showConversationModeModal,
} from './ui.js';

function getPendingRecords(conversationId) {
  return state.pendingRecordsByConversation.get(conversationId) || [];
}

function setPendingRecords(conversationId, records) {
  if (!records.length) {
    state.pendingRecordsByConversation.delete(conversationId);
    return;
  }
  state.pendingRecordsByConversation.set(conversationId, records);
}

function addPendingRecord(conversationId, record) {
  setPendingRecords(conversationId, [...getPendingRecords(conversationId), record]);
}

function replacePendingRecord(conversationId, recordId, nextRecord) {
  setPendingRecords(
    conversationId,
    getPendingRecords(conversationId).map((record) =>
      record.id === recordId ? nextRecord : record
    )
  );
}

function removePendingRecord(conversationId, recordId) {
  setPendingRecords(
    conversationId,
    getPendingRecords(conversationId).filter((record) => record.id !== recordId)
  );
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
    } else {
      target.appendChild(document.createTextNode(part));
    }
  });
}

function flushParagraph(lines, bubble) {
  if (!lines.length) {
    return;
  }
  const paragraph = document.createElement('p');
  paragraph.className = 'bubble-paragraph';
  appendInlineFormatted(paragraph, lines.join(' '));
  bubble.appendChild(paragraph);
  lines.length = 0;
}

function flushList(items, bubble) {
  if (!items.length) {
    return;
  }
  const list = document.createElement('ul');
  list.className = 'bubble-list';
  items.forEach((itemText) => {
    const item = document.createElement('li');
    appendInlineFormatted(item, itemText);
    list.appendChild(item);
  });
  bubble.appendChild(list);
  items.length = 0;
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

function createRawResponseSection(record) {
  if (!record.rawResponse) {
    return null;
  }

  const wrapper = document.createElement('div');
  const expanded = state.expandedRawResponseIds.has(record.id);
  const toggle = document.createElement('button');
  toggle.type = 'button';
  toggle.className = 'response-toggle';
  toggle.textContent = expanded ? '隐藏响应字段' : '查看响应字段';
  toggle.addEventListener('click', () => {
    if (expanded) {
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

export function renderRecords() {
  els.chatList.innerHTML = '';
  els.emptyState.hidden = state.records.length > 0;

  state.records.forEach((record) => {
    const userPart = createBubble('User', record.question, 'user');
    els.chatList.appendChild(userPart.row);

    const assistantText = record.status === 'error'
      ? (record.errorMessage || '请求失败')
      : (record.answer || 'Thinking...');

    const assistantPart = createBubble(
      'Assistant',
      assistantText,
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

export function renderConversations() {
  els.conversationList.innerHTML = '';

  state.conversations.forEach((conversation) => {
    const item = document.createElement('div');
    item.className = 'conversation-item';
    if (conversation.id === state.currentConversationId) {
      item.classList.add('active');
    }
    item.addEventListener('click', async () => {
      await switchConversation(conversation.id);
    });

    const header = document.createElement('div');
    header.className = 'conversation-item-header';

    const title = document.createElement('button');
    title.type = 'button';
    title.className = 'conversation-title';
    title.textContent = conversation.title || '未命名对话';
    title.addEventListener('click', async () => {
      await switchConversation(conversation.id);
    });

    const deleteButton = document.createElement('button');
    deleteButton.type = 'button';
    deleteButton.className = 'conversation-delete';
    deleteButton.textContent = '×';
    deleteButton.title = '删除对话';
    deleteButton.hidden = !state.conversationDeleteMode;
    deleteButton.addEventListener('click', async (event) => {
      event.stopPropagation();
      await removeConversation(conversation.id);
    });

    header.append(title, deleteButton);

    const meta = document.createElement('button');
    meta.type = 'button';
    meta.className = 'conversation-meta';
    const modeLabel = conversation.mode === 'memory' ? '记忆' : '单点';
    meta.textContent = `${modeLabel} · ${formatTime(conversation.updatedAt)}`;
    meta.addEventListener('click', async () => {
      await switchConversation(conversation.id);
    });

    item.append(header, meta);
    els.conversationList.appendChild(item);
  });
}

export async function loadConversations() {
  state.conversations = await invoke('list_conversations');
  const preferredConversationId = state.currentConversationId ?? state.lastConversationId;
  if (preferredConversationId && state.conversations.some((item) => item.id === preferredConversationId)) {
    state.currentConversationId = preferredConversationId;
  } else if (!state.currentConversationId || !state.conversations.some((item) => item.id === state.currentConversationId)) {
    state.currentConversationId = state.conversations[0]?.id ?? null;
  }
  state.lastConversationId = state.currentConversationId;

  const currentConversation = state.conversations.find((item) => item.id === state.currentConversationId);
  setMemoryMode(currentConversation?.mode || 'single');
  renderConversations();
}

export async function loadRecords() {
  if (!state.currentConversationId) {
    state.records = [];
    renderRecords();
    return;
  }

  const databaseRecords = await invoke('list_history_records', {
    conversationId: state.currentConversationId,
  });
  state.records = [...databaseRecords, ...getPendingRecords(state.currentConversationId)];
  renderRecords();
}

export async function switchConversation(conversationId) {
  state.currentConversationId = conversationId;
  state.lastConversationId = conversationId;
  const currentConversation = state.conversations.find((item) => item.id === conversationId);
  setMemoryMode(currentConversation?.mode || 'single');
  renderConversations();
  await loadRecords();
  await saveSettings(false);
}

async function createConversation(mode) {
  const conversation = await invoke('create_conversation', { mode });
  state.conversations.unshift(conversation);
  state.currentConversationId = conversation.id;
  state.lastConversationId = conversation.id;
  state.records = [];
  setMemoryMode(conversation.mode);
  renderConversations();
  renderRecords();
  setFormMessage('');
  hideConversationModeModal();
  await saveSettings(false);
}

async function removeConversation(conversationId) {
  const nextConversations = await invoke('delete_conversation', {
    conversationId,
  });

  state.conversations = nextConversations;
  if (state.currentConversationId === conversationId) {
    state.currentConversationId = nextConversations[0]?.id ?? null;
  }
  state.lastConversationId = state.currentConversationId;

  const currentConversation = state.conversations.find((item) => item.id === state.currentConversationId);
  setMemoryMode(currentConversation?.mode || 'single');
  renderConversations();
  await loadRecords();
  await saveSettings(false);
}

function toggleConversationDeleteMode() {
  state.conversationDeleteMode = !state.conversationDeleteMode;
  renderConversationDeleteToggle();
  renderConversations();
}

export async function askQuestion(event) {
  event.preventDefault();
  setFormMessage('');

  const draftQuestion = els.questionInput.value.trim();
  const activeConversationId = state.currentConversationId;
  const settings = collectSettings();

  if (!activeConversationId) {
    setFormMessage('请先创建一个对话窗口。', 'error');
    return;
  }

  if (!settings.apiUrl || !settings.apiKey) {
    setConfigStatus(false, 'API not set');
    setFormMessage('请先保存 API URL 和 API Key。', 'error');
    els.settingsPanel.classList.remove('hidden');
    return;
  }

  if (!draftQuestion) {
    setFormMessage('问题不能为空。', 'error');
    return;
  }

  els.askButton.disabled = true;
  els.saveSettings.disabled = true;
  els.questionInput.value = '';
  resetQuestionInputHeight();

  const tempRecord = {
    id: `pending-${Date.now()}`,
    conversationId: activeConversationId,
    question: draftQuestion,
    answer: 'Thinking...',
    rawResponse: null,
    createdAt: Date.now(),
    model: 'gpt-5.4',
    latencyMs: null,
    status: 'success',
    errorMessage: null,
  };

  addPendingRecord(activeConversationId, tempRecord);
  if (state.currentConversationId === activeConversationId) {
    state.records = [...state.records, tempRecord];
    renderRecords();
  }

  try {
    await saveSettings(false);
    const result = await invoke('ask', {
      conversationId: activeConversationId,
      question: draftQuestion,
      useShortTermMemory: state.memoryMode === 'memory',
    });
    removePendingRecord(activeConversationId, tempRecord.id);
    if (state.currentConversationId === activeConversationId) {
      state.records = state.records.map((record) =>
        record.id === tempRecord.id ? result : record
      );
      renderRecords();
    }
    setFormMessage('');
    await loadConversations();
    loadKnowledgeStatus().catch(() => {});
  } catch (error) {
    const failedRecord = {
      ...tempRecord,
      status: 'error',
      errorMessage: String(error),
      answer: '',
    };
    replacePendingRecord(activeConversationId, tempRecord.id, failedRecord);
    if (state.currentConversationId === activeConversationId) {
      state.records = state.records.map((record) =>
        record.id === tempRecord.id ? failedRecord : record
      );
      renderRecords();
    }
    setFormMessage(String(error), 'error');
  } finally {
    if (state.currentConversationId === activeConversationId) {
      await loadRecords();
    }
    els.askButton.disabled = false;
    els.saveSettings.disabled = false;
  }
}

export function bindChatEvents() {
  els.createConversation.addEventListener('click', () => {
    showConversationModeModal();
  });

  els.toggleConversationDelete?.addEventListener('click', () => {
    toggleConversationDeleteMode();
  });

  els.createSingleConversation?.addEventListener('click', () => {
    createConversation('single').catch((error) => {
      setFormMessage(String(error), 'error');
    });
  });

  els.createMemoryConversation?.addEventListener('click', () => {
    createConversation('memory').catch((error) => {
      setFormMessage(String(error), 'error');
    });
  });

  els.cancelCreateConversation?.addEventListener('click', () => {
    hideConversationModeModal();
  });

  els.questionInput.addEventListener('input', () => {
    resizeQuestionInput();
  });

  els.askForm.addEventListener('submit', askQuestion);
  resizeQuestionInput();
}
