import { bindChatEvents, loadConversations, loadRecords, renderConversations } from './chat.js';
import { bindKnowledgeEvents, refreshKnowledgeView, renderKnowledgeMap, renderKnowledgeTodayToggle } from './knowledge.js';
import { bindSettingsEvents, loadSettings } from './settings.js';
import { renderThoughtMap } from './thought-map.js';
import { ensureTauri, els, renderAskModel, renderConversationDeleteToggle, renderMemoryMode, renderView, setFormMessage } from './ui.js';

async function bootstrap() {
  ensureTauri();
  renderAskModel();
  renderConversationDeleteToggle();
  renderMemoryMode();
  renderKnowledgeTodayToggle();
  renderView();

  try {
    await loadSettings();
  } catch (error) {
    setFormMessage(`Settings warning: ${String(error)}`, 'error');
  }

  try {
    await loadConversations();
    renderConversations();
    await loadRecords();
    renderThoughtMap();
  } catch (error) {
    setFormMessage(`History load failed: ${String(error)}`, 'error');
  }

  try {
    await refreshKnowledgeView();
  } catch (error) {
    els.knowledgeStatus.textContent = `知识库加载失败：${String(error)}`;
  }
}

bindSettingsEvents(() => {
  renderKnowledgeMap();
});

bindChatEvents();
bindKnowledgeEvents();

window.addEventListener('resize', () => {
  renderKnowledgeMap();
  renderThoughtMap();
});

bootstrap();
