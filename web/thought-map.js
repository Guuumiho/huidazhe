import { state } from './state.js';
import { els } from './ui.js';

export async function loadConversationMap() {
  state.conversationMap = { nodes: [], edges: [] };
  state.selectedConversationMapNodeId = null;
  renderThoughtMap();
}

export function scheduleConversationMapRefresh() {
  renderThoughtMap();
}

export function renderThoughtMap() {
  els.thoughtMapStatus.textContent = '功能迭代中，暂时停用';
  els.thoughtMapEmpty.classList.remove('hidden');
  els.thoughtMapEmpty.textContent =
    '当前先专注稳定问答主流程，后续再恢复思维链整理能力。';
  els.thoughtMapStage.classList.add('hidden');

  if (els.thoughtMapLines) {
    els.thoughtMapLines.innerHTML = '';
  }
  if (els.thoughtMapNodes) {
    els.thoughtMapNodes.innerHTML = '';
  }
  if (els.thoughtMapDetail) {
    els.thoughtMapDetail.innerHTML = `
      <div class="thought-map-placeholder-card">
        <p class="thought-map-placeholder-title">思维地图</p>
        <p class="thought-map-placeholder-text">功能迭代中，暂时停用。</p>
        <p class="thought-map-placeholder-text">当前先专注稳定问答主流程，后续再恢复思维链整理能力。</p>
      </div>
    `;
  }
}
