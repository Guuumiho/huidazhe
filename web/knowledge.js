import { els, renderView } from './ui.js';
import { state } from './state.js';

const KNOWLEDGE_NOTICE_HTML = `
  <div class="knowledge-placeholder">
    <div class="knowledge-placeholder-card">
      <p class="eyebrow">Knowledge</p>
      <h2>开发迭代中，请期待后续功能：</h2>
      <div class="knowledge-placeholder-list">
        <div class="knowledge-placeholder-item">
          <div class="knowledge-placeholder-title">一天啥也没干怎么就过去了？！</div>
          <div class="knowledge-placeholder-desc">自动分析内容整理成思维线，帮助高效复盘</div>
        </div>
        <div class="knowledge-placeholder-item">
          <div class="knowledge-placeholder-title">今天居然卡在...任务这么久？！</div>
          <div class="knowledge-placeholder-desc">意图识别 + 监控提示：你已在...上耗了 2h，还要继续吗？是否尝试...方法？</div>
        </div>
        <div class="knowledge-placeholder-item">
          <div class="knowledge-placeholder-title">又要写日报</div>
          <div class="knowledge-placeholder-desc">自动汇总今日提问，写一份漂亮的问题发现、排坑踩雷、困难解决的向上管理日报</div>
        </div>
        <div class="knowledge-placeholder-item">
          <div class="knowledge-placeholder-title">知识点沉淀</div>
          <div class="knowledge-placeholder-desc">自动整理知识点，或者你想让我记住什么也 ok</div>
        </div>
      </div>
    </div>
  </div>
`;

function ensureKnowledgePlaceholder() {
  if (els.knowledgeView.dataset.placeholderReady === 'true') {
    return;
  }

  els.knowledgeView.innerHTML = KNOWLEDGE_NOTICE_HTML;
  els.knowledgeView.dataset.placeholderReady = 'true';
}

export function renderKnowledgeTodayToggle() {}

export function renderKnowledgeMap() {
  ensureKnowledgePlaceholder();
}

export async function loadKnowledgeStatus() {
  ensureKnowledgePlaceholder();
}

export async function refreshKnowledgeView() {
  ensureKnowledgePlaceholder();
}

export function bindKnowledgeEvents() {
  els.toggleKnowledge.addEventListener('click', async () => {
    state.view = state.view === 'chat' ? 'knowledge' : 'chat';
    renderView();
    if (state.view === 'knowledge') {
      ensureKnowledgePlaceholder();
    }
  });
}
