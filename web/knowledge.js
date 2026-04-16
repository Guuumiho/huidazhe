import { state } from './state.js';
import { els, escapeHtml, formatTime, invoke, relationColor, relationLabel, renderView } from './ui.js';

function startOfToday() {
  const now = new Date();
  now.setHours(0, 0, 0, 0);
  return now.getTime();
}

function getVisibleKnowledgeNodes() {
  if (!state.knowledgeTodayOnly) {
    return state.knowledgeNodes;
  }
  const todayStart = startOfToday();
  return state.knowledgeNodes.filter((node) => (node.updatedAt || 0) >= todayStart);
}

export function renderKnowledgeTodayToggle() {
  els.todayFilter.classList.toggle('active', state.knowledgeTodayOnly);
  els.todayFilter.textContent = state.knowledgeTodayOnly ? '今日: 开' : '今日';
}

export function renderKnowledgeStatus() {
  if (!state.knowledgeStatus) {
    els.knowledgeStatus.textContent = '知识库会在后台每小时自动检查一次。';
    return;
  }

  const status = state.knowledgeStatus;
  const lastRun = status.lastRunAt ? `上次整理 ${formatTime(status.lastRunAt)}` : '尚未整理';
  const lastState = status.lastStatus || 'idle';
  const pending = `待整理 ${status.pendingRecords || 0} 条`;
  const error = status.lastError ? `，最近错误：${status.lastError}` : '';
  els.knowledgeStatus.textContent = `${lastRun} · 状态 ${lastState} · ${pending}${error}`;
}

export function renderKnowledgeNodeList() {
  els.knowledgeNodeList.innerHTML = '';
  const visibleNodes = getVisibleKnowledgeNodes();

  visibleNodes.forEach((node) => {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'knowledge-node-item';
    if (node.id === state.selectedKnowledgeNodeId) {
      item.classList.add('active');
    }
    item.addEventListener('click', () => selectKnowledgeNode(node.id));

    const title = document.createElement('div');
    title.className = 'knowledge-node-title';
    title.textContent = node.title;

    const summary = document.createElement('div');
    summary.className = 'knowledge-node-summary';
    summary.textContent = node.summary || '暂无摘要';

    const meta = document.createElement('div');
    meta.className = 'knowledge-neighbor-meta';
    meta.textContent = `${node.sourceCount || 0} 条来源 · ${formatTime(node.updatedAt)}`;

    item.append(title, summary, meta);
    els.knowledgeNodeList.appendChild(item);
  });

  if (!visibleNodes.length) {
    const empty = document.createElement('div');
    empty.className = 'empty-state';
    empty.textContent = state.knowledgeTodayOnly
      ? '今天还没有整理出新的知识节点。'
      : '还没有知识节点。继续提问，或点击“立即整理”生成第一版主题地图。';
    els.knowledgeNodeList.appendChild(empty);
  }
}

export function renderKnowledgeDetail() {
  if (!state.knowledgeDetail) {
    els.knowledgeDetail.textContent = '选中一个节点后，会在这里看到知识摘要、关联问题和邻接节点。';
    return;
  }

  const detail = state.knowledgeDetail;
  const sourceCards = detail.sources.length
    ? detail.sources.map((source) => `
        <div class="knowledge-source-card">
          <div class="knowledge-source-question">${escapeHtml(source.question)}</div>
          <div class="knowledge-source-answer">${escapeHtml(source.answer)}</div>
          <div class="knowledge-source-meta">${formatTime(source.createdAt)} · ${escapeHtml(source.model || '')}</div>
        </div>
      `).join('')
    : '<div class="knowledge-source-card">还没有关联原始问题。</div>';

  const neighborCards = detail.neighbors.length
    ? detail.neighbors.map((neighbor) => `
        <div class="knowledge-neighbor-card">
          <div class="knowledge-neighbor-title">${escapeHtml(neighbor.title)}</div>
          <div class="knowledge-neighbor-summary">${escapeHtml(neighbor.summary || '暂无摘要')}</div>
          <div class="knowledge-neighbor-meta">${escapeHtml(relationLabel(neighbor.relationType))}</div>
        </div>
      `).join('')
    : '<div class="knowledge-neighbor-card">暂无直接关联节点。</div>';

  els.knowledgeDetail.innerHTML = `
    <h3 class="knowledge-detail-title">${escapeHtml(detail.title)}</h3>
    <p class="knowledge-detail-summary">${escapeHtml(detail.summary || '暂无摘要')}</p>
    <div class="knowledge-status">${detail.sourceCount || 0} 条来源 · 最近更新 ${formatTime(detail.updatedAt)}</div>
    <section class="knowledge-section">
      <h3>关联原始问题</h3>
      <div class="knowledge-source-list">${sourceCards}</div>
    </section>
    <section class="knowledge-section">
      <h3>直接关联节点</h3>
      <div class="knowledge-neighbor-list">${neighborCards}</div>
    </section>
  `;
}

export function renderKnowledgeMap() {
  const detail = state.knowledgeDetail;
  const hasMap = detail && detail.neighbors && detail.neighbors.length >= 0;
  els.knowledgeMapStage.classList.toggle('hidden', !hasMap);
  els.knowledgeMapEmpty.classList.toggle('hidden', hasMap);
  if (!hasMap) {
    return;
  }

  const width = Math.max(els.knowledgeMapStage.clientWidth, 480);
  const height = Math.max(els.knowledgeMapStage.clientHeight, 280);
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) * 0.33;
  const neighbors = detail.neighbors || [];

  els.knowledgeMapNodes.innerHTML = '';
  els.knowledgeMapLines.innerHTML = '';
  els.knowledgeMapLines.setAttribute('viewBox', `0 0 ${width} ${height}`);

  const centerNode = document.createElement('button');
  centerNode.type = 'button';
  centerNode.className = 'map-node center';
  centerNode.style.left = `${centerX}px`;
  centerNode.style.top = `${centerY}px`;
  centerNode.innerHTML = `<div class="map-node-title">${escapeHtml(detail.title)}</div>`;
  els.knowledgeMapNodes.appendChild(centerNode);

  neighbors.forEach((neighbor, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(neighbors.length, 1) - Math.PI / 2;
    const x = centerX + radius * Math.cos(angle);
    const y = centerY + radius * Math.sin(angle);

    const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
    line.setAttribute('x1', String(centerX));
    line.setAttribute('y1', String(centerY));
    line.setAttribute('x2', String(x));
    line.setAttribute('y2', String(y));
    line.setAttribute('stroke-width', '2');
    line.setAttribute('stroke', relationColor(neighbor.relationType));
    els.knowledgeMapLines.appendChild(line);

    const node = document.createElement('button');
    node.type = 'button';
    node.className = `map-node ${neighbor.relationType || 'related'}`;
    node.style.left = `${x}px`;
    node.style.top = `${y}px`;
    node.innerHTML = `
      <div class="map-node-title">${escapeHtml(neighbor.title)}</div>
      <div class="map-node-relation">${escapeHtml(relationLabel(neighbor.relationType))}</div>
    `;
    node.addEventListener('click', () => selectKnowledgeNode(neighbor.nodeId));
    els.knowledgeMapNodes.appendChild(node);
  });
}

export async function loadKnowledgeStatus() {
  state.knowledgeStatus = await invoke('get_knowledge_status');
  renderKnowledgeStatus();
}

export async function loadKnowledgeNodes() {
  state.knowledgeNodes = await invoke('list_knowledge_nodes');
  const visibleNodes = getVisibleKnowledgeNodes();
  if (!visibleNodes.some((node) => node.id === state.selectedKnowledgeNodeId)) {
    state.selectedKnowledgeNodeId = visibleNodes.length ? visibleNodes[0].id : null;
  }
  renderKnowledgeNodeList();
}

export async function selectKnowledgeNode(nodeId) {
  state.selectedKnowledgeNodeId = nodeId;
  renderKnowledgeNodeList();
  state.knowledgeDetail = await invoke('get_knowledge_node', { id: nodeId });
  renderKnowledgeDetail();
  renderKnowledgeMap();
}

export async function refreshKnowledgeView() {
  await loadKnowledgeStatus();
  await loadKnowledgeNodes();
  if (state.selectedKnowledgeNodeId) {
    await selectKnowledgeNode(state.selectedKnowledgeNodeId);
  } else {
    state.knowledgeDetail = null;
    renderKnowledgeDetail();
    renderKnowledgeMap();
  }
}

export async function runKnowledgeBuild() {
  els.buildKnowledge.disabled = true;
  els.knowledgeStatus.textContent = '正在整理知识节点...';
  try {
    const result = await invoke('build_knowledge_map');
    els.knowledgeStatus.textContent = result.message;
    await refreshKnowledgeView();
  } catch (error) {
    els.knowledgeStatus.textContent = `整理失败：${String(error)}`;
  } finally {
    els.buildKnowledge.disabled = false;
  }
}

export function bindKnowledgeEvents() {
  els.buildKnowledge.addEventListener('click', () => {
    runKnowledgeBuild();
  });

  els.todayFilter.addEventListener('click', async () => {
    state.knowledgeTodayOnly = !state.knowledgeTodayOnly;
    renderKnowledgeTodayToggle();
    await refreshKnowledgeView();
  });

  els.toggleKnowledge.addEventListener('click', async () => {
    state.view = state.view === 'chat' ? 'knowledge' : 'chat';
    renderView();
    if (state.view === 'knowledge') {
      await refreshKnowledgeView();
    }
  });
}
