import { state } from './state.js';
import { els, formatTime, invoke, relationColor } from './ui.js';

const NODE_SIZE = 86;
const NODE_RADIUS = NODE_SIZE / 2;
const X_PADDING = 18;
const Y_PADDING = 18;

export async function loadConversationMap() {
  if (!state.currentConversationId) {
    state.conversationMap = { nodes: [], edges: [] };
    state.selectedConversationMapNodeId = null;
    renderThoughtMap();
    return;
  }

  const graph = await invoke('get_conversation_map', {
    conversationId: state.currentConversationId,
  });
  state.conversationMap = graph || { nodes: [], edges: [] };

  const selectedExists = state.conversationMap.nodes.some(
    (node) => node.id === state.selectedConversationMapNodeId
  );
  if (!selectedExists) {
    state.selectedConversationMapNodeId = pickDefaultNodeId(state.conversationMap.nodes);
  }

  renderThoughtMap();
}

export function scheduleConversationMapRefresh(delay = 700) {
  window.setTimeout(() => {
    loadConversationMap().catch(() => {});
  }, delay);
}

function pickDefaultNodeId(nodes) {
  if (!nodes.length) {
    return null;
  }

  const latestUserNode = [...nodes]
    .filter((node) => node.nodeType === 'user')
    .sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0))[0];

  return latestUserNode?.id ?? nodes[0].id;
}

function buildNodeLayout(graph) {
  const stageWidth = els.thoughtMapStage.clientWidth || 240;
  const stageHeight = els.thoughtMapStage.clientHeight || 320;
  const nodes = graph.nodes || [];
  const edges = graph.edges || [];

  if (!nodes.length) {
    return { positions: new Map(), stageWidth, stageHeight };
  }

  const inDegree = new Map(nodes.map((node) => [node.id, 0]));
  const children = new Map(nodes.map((node) => [node.id, []]));

  edges.forEach((edge) => {
    inDegree.set(edge.toNodeId, (inDegree.get(edge.toNodeId) || 0) + 1);
    children.get(edge.fromNodeId)?.push(edge.toNodeId);
  });

  const sortedNodes = [...nodes].sort((a, b) => {
    if ((b.updatedAt || 0) !== (a.updatedAt || 0)) {
      return (b.updatedAt || 0) - (a.updatedAt || 0);
    }
    return a.id - b.id;
  });

  const roots = sortedNodes.filter((node) => (inDegree.get(node.id) || 0) === 0);
  const fallbackRoots = roots.length ? roots : sortedNodes;
  const depthById = new Map();
  const queue = fallbackRoots.map((node) => ({ id: node.id, depth: 0 }));
  const visited = new Set();

  while (queue.length) {
    const current = queue.shift();
    if (!current || visited.has(current.id)) {
      continue;
    }
    visited.add(current.id);
    depthById.set(current.id, current.depth);
    (children.get(current.id) || []).forEach((childId) => {
      queue.push({ id: childId, depth: current.depth + 1 });
    });
  }

  sortedNodes.forEach((node) => {
    if (!depthById.has(node.id)) {
      depthById.set(node.id, 0);
    }
  });

  const columns = new Map();
  sortedNodes.forEach((node) => {
    const depth = depthById.get(node.id) || 0;
    if (!columns.has(depth)) {
      columns.set(depth, []);
    }
    columns.get(depth).push(node);
  });

  const maxDepth = Math.max(...columns.keys());
  const usableWidth = Math.max(stageWidth - NODE_SIZE - X_PADDING * 2, NODE_SIZE);
  const xStep = maxDepth === 0 ? 0 : usableWidth / maxDepth;
  const positions = new Map();

  [...columns.entries()].forEach(([depth, columnNodes]) => {
    const usableHeight = Math.max(stageHeight - Y_PADDING * 2 - NODE_SIZE, NODE_SIZE);
    const yStep = columnNodes.length <= 1 ? 0 : usableHeight / (columnNodes.length - 1);
    columnNodes.forEach((node, index) => {
      positions.set(node.id, {
        x: X_PADDING + depth * xStep,
        y: Y_PADDING + index * yStep,
      });
    });
  });

  return { positions, stageWidth, stageHeight };
}

function renderThoughtMapLines(graph, positions) {
  els.thoughtMapLines.innerHTML = '';

  graph.edges.forEach((edge) => {
    const from = positions.get(edge.fromNodeId);
    const to = positions.get(edge.toNodeId);
    if (!from || !to) {
      return;
    }

    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    const startX = from.x + NODE_RADIUS;
    const startY = from.y + NODE_RADIUS;
    const endX = to.x + NODE_RADIUS;
    const endY = to.y + NODE_RADIUS;
    const curveX = (startX + endX) / 2;
    path.setAttribute(
      'd',
      `M ${startX} ${startY} C ${curveX} ${startY}, ${curveX} ${endY}, ${endX} ${endY}`
    );
    path.setAttribute('fill', 'none');
    path.setAttribute('stroke', relationColor(edge.relationType));
    path.setAttribute('stroke-width', '1.6');
    path.setAttribute('stroke-linecap', 'round');
    els.thoughtMapLines.appendChild(path);
  });
}

function renderThoughtMapNodes(graph, positions) {
  els.thoughtMapNodes.innerHTML = '';

  graph.nodes.forEach((node) => {
    const position = positions.get(node.id);
    if (!position) {
      return;
    }

    const button = document.createElement('button');
    button.type = 'button';
    button.className = `thought-map-node ${node.nodeType}`;
    if (node.id === state.selectedConversationMapNodeId) {
      button.classList.add('active');
    }
    button.style.left = `${position.x}px`;
    button.style.top = `${position.y}px`;
    button.title = node.title;
    button.addEventListener('click', () => {
      state.selectedConversationMapNodeId = node.id;
      renderThoughtMap();
    });

    const label = document.createElement('span');
    label.className = 'thought-map-node-label';
    label.textContent = node.title;
    button.appendChild(label);
    els.thoughtMapNodes.appendChild(button);
  });
}

function renderThoughtMapDetail(graph) {
  const node = graph.nodes.find((item) => item.id === state.selectedConversationMapNodeId);
  if (!node) {
    els.thoughtMapDetail.innerHTML = '选中一个节点后，这里会显示它的类型、关系和更新时间。';
    return;
  }

  const relatedEdges = graph.edges.filter(
    (edge) => edge.fromNodeId === node.id || edge.toNodeId === node.id
  );
  const relatedNodes = relatedEdges
    .map((edge) => {
      const otherId = edge.fromNodeId === node.id ? edge.toNodeId : edge.fromNodeId;
      return graph.nodes.find((item) => item.id === otherId);
    })
    .filter(Boolean);

  const relatedList = relatedNodes.length
    ? `<ul class="thought-map-detail-list">${relatedNodes
        .map((item) => `<li>${item.title}</li>`)
        .join('')}</ul>`
    : '<p class="thought-map-detail-meta">当前还没有关联节点。</p>';

  els.thoughtMapDetail.innerHTML = `
    <p class="thought-map-detail-title">${node.title}</p>
    <p class="thought-map-detail-meta">类型：${node.nodeType === 'assistant' ? '助手节点' : '用户节点'}</p>
    <p class="thought-map-detail-meta">更新时间：${formatTime(node.updatedAt || node.createdAt)}</p>
    <p class="thought-map-detail-meta">来源记录：${node.createdFromRecordId ?? '—'}</p>
    ${relatedList}
  `;
}

export function renderThoughtMap() {
  const graph = state.conversationMap || { nodes: [], edges: [] };
  const hasNodes = Array.isArray(graph.nodes) && graph.nodes.length > 0;

  els.thoughtMapStatus.textContent = state.currentConversationId
    ? '当前窗口的思考路径会在这里逐步展开。'
    : '先选择一个对话窗口。';
  els.thoughtMapEmpty.classList.toggle('hidden', hasNodes);
  els.thoughtMapStage.classList.toggle('hidden', !hasNodes);

  if (!hasNodes) {
    els.thoughtMapNodes.innerHTML = '';
    els.thoughtMapLines.innerHTML = '';
    renderThoughtMapDetail({ nodes: [], edges: [] });
    return;
  }

  const { positions } = buildNodeLayout(graph);
  renderThoughtMapLines(graph, positions);
  renderThoughtMapNodes(graph, positions);
  renderThoughtMapDetail(graph);
}
