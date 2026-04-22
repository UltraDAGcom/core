// UltraDAG Explorer Script

document.addEventListener('DOMContentLoaded', function() {

// Mainnet is paused — array kept for future re-enablement.
var MAINNET_NODES = [];
var TESTNET_NODES = [
  'https://ultradag-node-1.fly.dev','https://ultradag-node-2.fly.dev'
];

let NODES = TESTNET_NODES;
const SATS = 100_000_000;
const MAX_SUPPLY = 21_000_000;
let lastStatus = null;

// Network switch
function switchNet(net) {
  NODES = net === 'mainnet' ? MAINNET_NODES : TESTNET_NODES;
  document.querySelectorAll('.net-btn').forEach(function(b) {
    b.classList.toggle('active', b.id === 'btn-' + net);
  });
  refreshStats();
}

var btnTestnet = document.getElementById('btn-testnet');
var btnMainnet = document.getElementById('btn-mainnet');
if (btnTestnet) btnTestnet.onclick = function() { switchNet('testnet'); };
if (btnMainnet) btnMainnet.onclick = function() { switchNet('mainnet'); };

// API fetch with fallback
function fetchApi(path) {
  return Promise.any(NODES.map(function(n) {
    return fetch(n + path, { signal: AbortSignal.timeout(4000) }).then(function(r) {
      if (!r.ok) throw new Error(r.status);
      return r.json();
    });
  }));
}

// Utilities
function short(hex) {
  if (!hex) return '--';
  var s = typeof hex === 'string' ? hex : Array.from(hex).map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
  return s.length > 16 ? s.slice(0, 8) + '…' + s.slice(-6) : s;
}
function fmt(n) { return n == null ? '--' : Number(n).toLocaleString(); }
function fmtUdag(sats) { return sats == null ? '--' : (Number(sats) / SATS).toLocaleString(undefined, { maximumFractionDigits: 2 }); }

// Stats refresh
function refreshStats() {
  fetchApi('/status').then(function(s) {
    lastStatus = s;

    document.getElementById('s-round').textContent = fmt(s.dag_round);
    document.getElementById('s-finalized').textContent = fmt(s.last_finalized_round);

    var lag = (s.dag_round || 0) - (s.last_finalized_round || 0);
    var lagEl = document.getElementById('s-lag');
    lagEl.textContent = 'lag: ' + lag;
    lagEl.className = 'stat-extra ' + (lag <= 3 ? 'lag-good' : lag <= 10 ? 'lag-warn' : 'lag-bad');

    var supplyUdag = s.total_supply_udag || (s.total_supply ? s.total_supply / SATS : 0);
    document.getElementById('s-supply').textContent = Math.floor(supplyUdag).toLocaleString() + ' UDAG';
    document.getElementById('s-supply-pct').textContent = (supplyUdag / MAX_SUPPLY * 100).toFixed(4) + '% of 21M';

    document.getElementById('s-validators').textContent = fmt(s.active_stakers || s.validators);
    document.getElementById('s-peers').textContent = fmt(s.connected_peers || s.peer_count);
    document.getElementById('s-mempool').textContent = fmt(s.mempool_size);
    document.getElementById('s-accounts').textContent = fmt(s.account_count || s.total_accounts);

    var stakedUdag = s.total_staked_udag || (s.total_staked ? s.total_staked / SATS : 0);
    document.getElementById('s-staked').textContent = Math.floor(stakedUdag).toLocaleString() + ' UDAG';

    loadRounds(s.last_finalized_round || s.dag_round || 0);
  }).catch(function(e) {
    console.warn('Stats fetch failed:', e);
    var currentNet = document.querySelector('.net-btn.active');
    if (currentNet && currentNet.id === 'btn-mainnet') {
      document.getElementById('s-round').textContent = 'Not live';
      document.getElementById('s-finalized').textContent = '--';
      document.getElementById('s-supply').textContent = '--';
      document.getElementById('s-validators').textContent = '--';
      document.getElementById('s-peers').textContent = '--';
      document.getElementById('s-mempool').textContent = '--';
      document.getElementById('s-accounts').textContent = '--';
      document.getElementById('s-staked').textContent = '--';
      var tbody = document.getElementById('rounds-body');
      if (tbody) tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--muted)">Mainnet is paused</td></tr>';
    }
  });
}

// Load recent rounds
function loadRounds(latest) {
  var tbody = document.getElementById('rounds-body');
  if (!latest || latest < 1) {
    tbody.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--muted)">No finalized rounds yet</td></tr>';
    return;
  }

  var start = Math.max(1, latest - 9);
  var promises = [];
  for (var r = latest; r >= start; r--) {
    (function(round) {
      promises.push(
        fetchApi('/round/' + round)
          .then(function(data) { return { round: round, data: data, ok: true }; })
          .catch(function() { return { round: round, ok: false }; })
      );
    })(r);
  }

  Promise.all(promises).then(function(results) {
    var html = '';
    for (var i = 0; i < results.length; i++) {
      var res = results[i];
      if (!res.ok) {
        html += '<tr><td class="mono">' + fmt(res.round) + '</td><td colspan="3" style="color:var(--muted)">unavailable</td></tr>';
        continue;
      }

      var verts = Array.isArray(res.data) ? res.data : (res.data.vertices || []);
      var validators = verts.map(function(v) { return short(v.validator || v.address); }).join(', ');
      var finalized = lastStatus && res.round <= (lastStatus.last_finalized_round || 0);

      html += '<tr>'
        + '<td><span class="mono link" onclick="window.showRound(' + res.round + ')">' + fmt(res.round) + '</span></td>'
        + '<td class="mono">' + verts.length + '</td>'
        + '<td class="mono" style="font-size:12px;color:var(--subtle)">' + (validators || '--') + '</td>'
        + '<td>' + (finalized ? '<span class="badge badge-green">Finalized</span>' : '<span class="badge badge-yellow">Pending</span>') + '</td>'
        + '</tr>';
    }
    tbody.innerHTML = html;
  });
}

// Detail panel
var detailPanel = document.getElementById('detail-panel');
var detailTitle = document.getElementById('detail-title');
var detailBody = document.getElementById('detail-body');

document.getElementById('detail-close').onclick = function() { detailPanel.classList.remove('active'); };

function showDetail(title, html) {
  detailTitle.textContent = title;
  detailBody.innerHTML = html;
  detailPanel.classList.add('active');
  detailPanel.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
}

function field(label, value) {
  return '<div class="detail-field"><div class="label">' + label + '</div><div class="value">' + value + '</div></div>';
}

// Show round
window.showRound = function(round) {
  showDetail('Round ' + fmt(round), '<div style="color:var(--muted)">Loading...</div>');

  fetchApi('/round/' + round).then(function(data) {
    var verts = Array.isArray(data) ? data : (data.vertices || []);

    if (!verts.length) {
      showDetail('Round ' + fmt(round), '<div style="color:var(--muted)">No vertices in this round</div>');
      return;
    }

    var finalized = lastStatus && round <= (lastStatus.last_finalized_round || 0);

    var html = '<div class="detail-grid" style="margin-bottom:16px">'
      + field('Round', fmt(round))
      + field('Vertices', verts.length)
      + field('Status', finalized ? '<span class="badge badge-green">Finalized</span>' : '<span class="badge badge-yellow">Pending</span>')
      + '</div><div class="section-title" style="margin-top:16px">Vertices</div><div class="vertex-list">';

    for (var i = 0; i < verts.length; i++) {
      var v = verts[i];
      var hash = v.hash || v.vertex_hash || '';
      html += '<div class="vertex-item" onclick="window.showVertex(\'' + hash + '\')">'
        + '<div><div class="vertex-label">Hash</div><div class="vertex-val">' + short(hash) + '</div></div>'
        + '<div><div class="vertex-label">Validator</div><div class="vertex-val">' + short(v.validator || v.address) + '</div></div>'
        + '<div><div class="vertex-label">Txs</div><div class="vertex-val">' + (v.tx_count != null ? v.tx_count : (v.transactions ? v.transactions.length : 0)) + '</div></div>'
        + '<div><div class="vertex-label">Parents</div><div class="vertex-val">' + (v.parent_count != null ? v.parent_count : (v.parents ? v.parents.length : 0)) + '</div></div>'
        + '</div>';
    }

    html += '</div>';
    showDetail('Round ' + fmt(round), html);
  }).catch(function() {
    showDetail('Round ' + fmt(round), '<div style="color:var(--danger)">Failed to load round data</div>');
  });
};

// Show vertex
window.showVertex = function(hash) {
  showDetail('Vertex ' + short(hash), '<div style="color:var(--muted)">Loading...</div>');

  fetchApi('/vertex/' + hash).then(function(v) {
    var html = '<div class="detail-grid">'
      + field('Hash', hash)
      + field('Round', fmt(v.round))
      + field('Validator', v.validator || '--')
      + field('Parent Count', v.parent_count != null ? v.parent_count : (v.parents ? v.parents.length : '--'))
      + field('Coinbase', v.coinbase_reward != null ? fmtUdag(v.coinbase_reward) + ' UDAG' : '--')
      + '</div>';

    var txs = v.transactions || [];
    if (txs.length) {
      html += '<div class="section-title" style="margin-top:20px">Transactions</div><div class="table-wrap"><table>'
        + '<thead><tr><th>Type</th><th>Hash</th><th>From</th><th>Fee</th></tr></thead><tbody>';

      for (var i = 0; i < txs.length; i++) {
        var tx = txs[i];
        html += '<tr>'
          + '<td><span class="badge badge-blue">' + (tx.type || tx.tx_type || '--') + '</span></td>'
          + '<td class="mono link" onclick="window.showTx(\'' + (tx.hash || '') + '\')">' + short(tx.hash) + '</td>'
          + '<td class="mono" style="font-size:12px">' + short(tx.from) + '</td>'
          + '<td class="mono">' + (tx.fee != null ? fmtUdag(tx.fee) : '--') + '</td>'
          + '</tr>';
      }

      html += '</tbody></table></div>';
    }

    showDetail('Vertex ' + short(hash), html);
  }).catch(function() {
    showDetail('Vertex ' + short(hash), '<div style="color:var(--danger)">Vertex not found</div>');
  });
};

// Show transaction
window.showTx = function(hash) {
  showDetail('Transaction ' + short(hash), '<div style="color:var(--muted)">Loading...</div>');

  fetchApi('/tx/' + hash).then(function(tx) {
    var status = tx.status === 'finalized' ? '<span class="badge badge-green">Finalized</span>' : '<span class="badge badge-yellow">Pending</span>';

    var html = '<div class="detail-grid">'
      + field('Hash', hash)
      + field('Status', status)
      + field('Type', tx.type || tx.tx_type || '--');

    if (tx.from) html += field('From', '<span class="link" onclick="window.showAddress(\'' + tx.from + '\')">' + tx.from + '</span>');
    if (tx.to) html += field('To', '<span class="link" onclick="window.showAddress(\'' + tx.to + '\')">' + tx.to + '</span>');
    if (tx.amount != null) html += field('Amount', fmtUdag(tx.amount) + ' UDAG');
    if (tx.fee != null) html += field('Fee', fmtUdag(tx.fee) + ' UDAG');
    if (tx.round != null) html += field('Round', '<span class="link" onclick="window.showRound(' + tx.round + ')">' + fmt(tx.round) + '</span>');
    if (tx.vertex_hash) html += field('Vertex', '<span class="link" onclick="window.showVertex(\'' + tx.vertex_hash + '\')">' + short(tx.vertex_hash) + '</span>');
    if (tx.validator) html += field('Validator', short(tx.validator));

    html += '</div>';
    showDetail('Transaction ' + short(hash), html);
  }).catch(function() {
    showDetail('Transaction ' + short(hash), '<div style="color:var(--danger)">Transaction not found</div>');
  });
};

// Show address
window.showAddress = function(addr) {
  showDetail('Address ' + short(addr), '<div style="color:var(--muted)">Loading...</div>');

  fetchApi('/balance/' + addr).then(function(bal) {
    var html = '<div class="detail-grid">'
      + field('Address', addr)
      + field('Balance', (bal.balance_udag != null ? bal.balance_udag : fmtUdag(bal.balance)) + ' UDAG')
      + field('Balance (sats)', fmt(bal.balance))
      + field('Nonce', fmt(bal.nonce))
      + '</div>';

    // Try to fetch stake info
    fetchApi('/stake/' + addr).then(function(st) {
      if (st && (st.staked > 0 || st.staked_udag)) {
        html += '<div class="section-title" style="margin-top:20px">Stake</div><div class="detail-grid">'
          + field('Staked', (st.staked_udag != null ? st.staked_udag : fmtUdag(st.staked)) + ' UDAG')
          + field('Active Validator', st.is_active_validator ? '<span class="badge badge-green">Yes</span>' : '<span class="badge badge-yellow">No</span>');
        if (st.commission_percent != null) html += field('Commission', st.commission_percent + '%');
        if (st.effective_stake) html += field('Effective Stake', fmtUdag(st.effective_stake) + ' UDAG');
        html += '</div>';
      }
      showDetail('Address ' + short(addr), html);
    }).catch(function() {
      showDetail('Address ' + short(addr), html);
    });
  }).catch(function() {
    showDetail('Address ' + short(addr), '<div style="color:var(--danger)">Address not found or has no balance</div>');
  });
};

// Search
var searchInput = document.getElementById('search-input');
var searchError = document.getElementById('search-error');

function doSearch() {
  var q = searchInput.value.trim();
  searchError.textContent = '';

  if (!q) return;

  // Round number
  if (/^\d+$/.test(q)) {
    window.showRound(parseInt(q));
    return;
  }

  // Hex hash
  var hex = q.startsWith('0x') ? q.slice(2) : q;
  if (/^[0-9a-fA-F]{64}$/.test(hex)) {
    showDetail('Looking up...', '<div style="color:var(--muted)">Searching...</div>');

    fetchApi('/tx/' + hex)
      .then(function() { window.showTx(hex); })
      .catch(function() {
        return fetchApi('/vertex/' + hex)
          .then(function() { window.showVertex(hex); })
          .catch(function() {
            return fetchApi('/balance/' + hex)
              .then(function() { window.showAddress(hex); })
              .catch(function() {
                searchError.textContent = 'Nothing found for: ' + short(hex);
                detailPanel.classList.remove('active');
              });
          });
      });
    return;
  }

  // Address
  if (/^[0-9a-fA-F]+$/.test(hex) && hex.length >= 20) {
    window.showAddress(hex);
    return;
  }

  searchError.textContent = 'Enter a round number, 64-char hex hash, or address.';
}

searchInput.addEventListener('keydown', function(e) { if (e.key === 'Enter') doSearch(); });
document.getElementById('search-btn').onclick = doSearch;

// Initialize — default to testnet
switchNet('testnet');
setInterval(refreshStats, 10000);

}); // end DOMContentLoaded
