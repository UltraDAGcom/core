import * as store from '../store.js';
import * as api from '../api.js';
import { shortAddr, shortHash, formatUdag } from '../utils.js';

export function mount(container) {
  const el = document.createElement('div');
  container.appendChild(el);
  let currentRound = null;

  function render(roundData) {
    const status = store.get('status');
    const connected = store.get('connected');
    const maxRound = status?.dag_round ?? 0;

    if (!connected) {
      el.innerHTML = `
        <div class="page-header">
          <div class="page-title">Explorer</div>
          <div class="page-sub">Browse the DAG by round</div>
        </div>
        <div class="page-content">
          <div class="empty-state">Connect to a node to explore the DAG</div>
        </div>
      `;
      return;
    }

    el.innerHTML = `
      <div class="page-header">
        <div class="page-title">Explorer</div>
        <div class="page-sub">Browse DAG vertices by round</div>
      </div>
      <div class="page-content">
        <div class="card">
          <div class="card-header">
            <span class="card-title">Round Browser</span>
            <div style="display:flex;align-items:center;gap:.5rem">
              <button class="btn btn-sm" id="btn-prev" ${currentRound === null || currentRound <= 0 ? 'disabled' : ''}>&larr;</button>
              <input type="number" id="round-input" style="width:80px;text-align:center" placeholder="#" value="${currentRound ?? ''}" min="0">
              <button class="btn btn-sm" id="btn-go">Go</button>
              <button class="btn btn-sm" id="btn-next" ${currentRound === null || currentRound >= maxRound ? 'disabled' : ''}>&rarr;</button>
              <button class="btn btn-sm" id="btn-latest">Latest</button>
            </div>
          </div>
          <div class="card-body" id="round-content">
            ${roundData ? renderVertices(roundData) : '<div class="empty-state">Enter a round number to browse vertices</div>'}
          </div>
        </div>

        ${status ? renderDagSummary(status) : ''}
      </div>
    `;

    bindEvents();
  }

  function renderVertices(vertices) {
    if (!vertices || vertices.length === 0) {
      return '<div class="empty-state">No vertices in this round</div>';
    }

    return `
      <div style="margin-bottom:.5rem;font-size:.75rem;color:var(--fg3)">
        ${vertices.length} vertex${vertices.length !== 1 ? 'es' : ''} in round ${currentRound}
      </div>
      ${vertices.map(v => `
        <div class="card" style="margin-bottom:.5rem;border-color:rgba(255,255,255,.04)">
          <div class="card-body" style="padding:.75rem">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:.5rem">
              <span class="badge badge-blue">Validator: ${shortAddr(v.validator)}</span>
              <span class="badge">${v.tx_count} tx${v.tx_count !== 1 ? 's' : ''}</span>
            </div>
            <div class="kv-row">
              <span class="kv-label">Hash</span>
              <span class="kv-value" style="font-size:.68rem">${shortHash(v.hash)}</span>
            </div>
            <div class="kv-row">
              <span class="kv-label">Reward</span>
              <span class="kv-value">${formatUdag(v.reward)} UDAG</span>
            </div>
            <div class="kv-row">
              <span class="kv-label">Parents</span>
              <span class="kv-value">${v.parent_count}</span>
            </div>
          </div>
        </div>
      `).join('')}
    `;
  }

  function renderDagSummary(s) {
    return `
      <div class="card" style="margin-top:1rem">
        <div class="card-header"><span class="card-title">DAG Summary</span></div>
        <div class="card-body">
          <div class="grid-3">
            <div class="stat-card">
              <div class="stat-label">Total Vertices</div>
              <div class="stat-value" style="font-size:1.3rem">${s.dag_vertices}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Current Round</div>
              <div class="stat-value" style="font-size:1.3rem">${s.dag_round}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Active Tips</div>
              <div class="stat-value" style="font-size:1.3rem">${s.dag_tips}</div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  async function loadRound(round) {
    if (round === null || round === undefined || round < 0) return;
    currentRound = round;
    try {
      const data = await api.getRound(round);
      render(Array.isArray(data) ? data : [data]);
    } catch (e) {
      render(null);
      const content = el.querySelector('#round-content');
      if (content) content.innerHTML = `<div class="empty-state">No vertices in round ${round}</div>`;
    }
  }

  function bindEvents() {
    el.querySelector('#btn-prev')?.addEventListener('click', () => {
      if (currentRound !== null && currentRound > 0) loadRound(currentRound - 1);
    });
    el.querySelector('#btn-next')?.addEventListener('click', () => {
      if (currentRound !== null) loadRound(currentRound + 1);
    });
    el.querySelector('#btn-go')?.addEventListener('click', () => {
      const v = parseInt(el.querySelector('#round-input').value);
      if (!isNaN(v)) loadRound(v);
    });
    el.querySelector('#btn-latest')?.addEventListener('click', () => {
      const s = store.get('status');
      if (s) loadRound(s.dag_round);
    });
    el.querySelector('#round-input')?.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        const v = parseInt(e.target.value);
        if (!isNaN(v)) loadRound(v);
      }
    });
  }

  render(null);
  const unsub = store.on('connected', () => render(null));

  return {
    cleanup() { unsub(); }
  };
}
