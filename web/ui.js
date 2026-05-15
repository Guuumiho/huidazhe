import { state } from './state.js';

export const invoke = window.__TAURI__?.core?.invoke;

export const els = {
  createConversation: document.querySelector('#create-conversation'),
  toggleConversationDelete: document.querySelector('#toggle-conversation-delete'),
  conversationList: document.querySelector('#conversation-list'),
  apiUrl: document.querySelector('#api-url'),
  apiKey: document.querySelector('#api-key'),
  themeSelect: document.querySelector('#theme-select'),
  toggleSettings: document.querySelector('#toggle-settings'),
  toggleKnowledge: document.querySelector('#toggle-knowledge'),
  settingsPanel: document.querySelector('#settings-panel'),
  saveSettings: document.querySelector('#save-settings'),
  askForm: document.querySelector('#ask-form'),
  askButton: document.querySelector('#ask-button'),
  questionInput: document.querySelector('#question-input'),
  modelToggle: document.querySelector('#model-toggle'),
  formMessage: document.querySelector('#form-message'),
  chatList: document.querySelector('#chat-list'),
  emptyState: document.querySelector('#empty-state'),
  chatView: document.querySelector('#chat-view'),
  thoughtMapSidebar: document.querySelector('#thought-map-sidebar'),
  thoughtMapStatus: document.querySelector('#thought-map-status'),
  thoughtMapEmpty: document.querySelector('#thought-map-empty'),
  thoughtMapStage: document.querySelector('#thought-map-stage'),
  thoughtMapLines: document.querySelector('#thought-map-lines'),
  thoughtMapNodes: document.querySelector('#thought-map-nodes'),
  thoughtMapDetail: document.querySelector('#thought-map-detail'),
  knowledgeView: document.querySelector('#knowledge-view'),
  todayFilter: document.querySelector('#today-filter'),
  buildKnowledge: document.querySelector('#build-knowledge'),
  knowledgeStatus: document.querySelector('#knowledge-status'),
  knowledgeNodeList: document.querySelector('#knowledge-node-list'),
  knowledgeMapEmpty: document.querySelector('#knowledge-map-empty'),
  knowledgeMapStage: document.querySelector('#knowledge-map-stage'),
  knowledgeMapLines: document.querySelector('#knowledge-map-lines'),
  knowledgeMapNodes: document.querySelector('#knowledge-map-nodes'),
  knowledgeDetail: document.querySelector('#knowledge-detail'),
  composer: document.querySelector('#ask-form'),
  conversationModeModal: document.querySelector('#conversation-mode-modal'),
  createSingleConversation: document.querySelector('#create-single-conversation'),
  createMemoryConversation: document.querySelector('#create-memory-conversation'),
  cancelCreateConversation: document.querySelector('#cancel-create-conversation'),
  noteSearchModal: document.querySelector('#note-search-modal'),
  noteSearchTitle: document.querySelector('#note-search-title'),
  noteSearchBody: document.querySelector('#note-search-body'),
  closeNoteSearch: document.querySelector('#close-note-search'),
};

const QUESTION_INPUT_BASE_HEIGHT = 56;
const QUESTION_INPUT_EXPANDED_HEIGHT = 240;

export function ensureTauri() {
  if (!invoke) {
    throw new Error('Tauri bridge is not available. Please run inside the desktop app.');
  }
}

export function setFormMessage(message, kind = '') {
  els.formMessage.textContent = message || '';
  els.formMessage.className = `form-message${kind ? ` ${kind}` : ''}`;
}

export function renderAskModel() {
  if (!els.modelToggle) {
    return;
  }

  els.modelToggle.textContent = state.askModel === 'gpt-5.5' ? 'gpt-5.5' : 'gpt-5.4';
}

export function formatTime(timestamp) {
  if (!timestamp) {
    return '';
  }

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

export function escapeHtml(text) {
  return String(text || '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

export function relationLabel(relationType) {
  switch (relationType) {
    case 'prerequisite':
      return '前置';
    case 'confusable':
      return '易混淆';
    default:
      return '相关';
  }
}

export function relationColor(relationType) {
  const root = getComputedStyle(document.documentElement);
  switch (relationType) {
    case 'prerequisite':
      return root.getPropertyValue('--prerequisite-line').trim() || '#b8c4d6';
    case 'confusable':
      return root.getPropertyValue('--confusable-line').trim() || '#d6c3bc';
    default:
      return root.getPropertyValue('--related-line').trim() || '#c1b7d0';
  }
}

export function renderConversationDeleteToggle() {
  els.toggleConversationDelete?.classList.toggle('active', state.conversationDeleteMode);
}

export function renderMemoryMode() {
  const isMemory = state.memoryMode === 'memory';
  els.questionInput.placeholder = isMemory
    ? '输入一个需要参考前文聊天内容的问题...'
    : '输入一个不会污染主工作流上下文的小问题...';
}

export function setMemoryMode(mode) {
  state.memoryMode = mode === 'memory' ? 'memory' : 'single';
  renderMemoryMode();
}

export function setAskModel(model) {
  state.askModel = model === 'gpt-5.5' ? 'gpt-5.5' : 'gpt-5.4';
  renderAskModel();
}

export function showConversationModeModal() {
  els.conversationModeModal?.classList.remove('hidden');
}

export function hideConversationModeModal() {
  els.conversationModeModal?.classList.add('hidden');
}

export function renderView() {
  const showingKnowledge = state.view === 'knowledge';
  els.chatView.classList.toggle('hidden', showingKnowledge);
  els.knowledgeView.classList.toggle('hidden', !showingKnowledge);
  els.composer.classList.toggle('hidden', showingKnowledge);
  els.thoughtMapSidebar?.classList.toggle('hidden', showingKnowledge);
  els.toggleKnowledge.classList.toggle('active', showingKnowledge);
  els.toggleKnowledge.textContent = showingKnowledge ? '回到问答' : '知识地图';
}

export function resizeQuestionInput() {
  const input = els.questionInput;
  if (!input) {
    return;
  }

  input.style.height = `${QUESTION_INPUT_BASE_HEIGHT}px`;
  const nextHeight = Math.min(
    Math.max(input.scrollHeight, QUESTION_INPUT_BASE_HEIGHT),
    QUESTION_INPUT_EXPANDED_HEIGHT
  );
  input.style.height = `${nextHeight}px`;
}

export function resetQuestionInputHeight() {
  if (!els.questionInput) {
    return;
  }

  els.questionInput.style.height = `${QUESTION_INPUT_BASE_HEIGHT}px`;
}
