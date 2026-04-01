// UltraDAG Network Page Script
var TESTNET_NODES = ['https://ultradag-node-1.fly.dev','https://ultradag-node-2.fly.dev','https://ultradag-node-3.fly.dev','https://ultradag-node-4.fly.dev','https://ultradag-node-5.fly.dev'];
var MAINNET_NODES = ['https://ultradag-mainnet-1.fly.dev','https://ultradag-mainnet-2.fly.dev','https://ultradag-mainnet-3.fly.dev','https://ultradag-mainnet-4.fly.dev','https://ultradag-mainnet-5.fly.dev'];
var NODES = TESTNET_NODES;
var currentNet = 'testnet';
var SATS = 100000000, MAX_SUPPLY = 21000000;
var countdown = 5;
var updateTimer = null;

function fmt(n) { return n >= 1e6 ? (n/1e6).toFixed(2)+'M' : n >= 1e3 ? (n/1e3).toFixed(1)+'K' : n.toFixed(0); }
function hash(h) { return h && h.length > 12 ? h.slice(0,6)+'...'+h.slice(-4) : h||'--'; }

function switchNet(net) {
  currentNet = net;
  NODES = net === 'mainnet' ? MAINNET_NODES : TESTNET_NODES;
  document.querySelectorAll('.net-btn').forEach(function(b) {
    b.classList.toggle('active', b.id === 'btn-' + net);
  });
  var v = function(id) { return document.getElementById(id); };
  ['v-round','v-fin','v-supply','v-treasury','v-verts','v-val','v-peers','v-mem','v-accts','dag-round'].forEach(function(id) {
    var el = v(id); if (el) el.textContent = '--';
  });
  var vb = document.getElementById('val-body');
  if (vb) vb.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--muted)">Loading...</td></tr>';
  if (updateTimer) clearTimeout(updateTimer);
  countdown = 5;
  update();
}

async function update() {
  var dot = document.getElementById('live-dot');
  var lbl = document.getElementById('live-label');
  var arc = document.getElementById('refresh-arc');

  if (!dot || !lbl) return;

  var results = await Promise.all(NODES.map(async function(url, i) {
    var t0 = Date.now();
    try {
      var r = await fetch(url+'/status', {signal: AbortSignal.timeout(4000)});
      return {ok: true, data: await r.json(), lat: Date.now()-t0, url: url};
    } catch(e) { return {ok: false, lat: null, url: url}; }
  }));

  var live = 0;
  // Use best node (highest round) for all network-wide stats — don't sum across nodes
  var best = null;

  var nodesDiv = document.getElementById('nodes');
  if (nodesDiv) {
    nodesDiv.innerHTML = results.map(function(r, i) {
      return r.ok && r.data ?
        '<div class="node-card"><div class="node-head"><span class="node-name">Node '+(i+1)+'</span><span class="node-status online"></span></div><div class="node-rows"><div class="node-row"><span class="node-row-label">Round</span><span class="node-row-value">'+(r.data.dag_round||'--')+'</span></div><div class="node-row"><span class="node-row-label">Peers</span><span class="node-row-value">'+(r.data.peer_count||'--')+'</span></div><div class="node-row"><span class="node-row-label">Latency</span><span class="node-row-value">'+r.lat+'ms</span></div></div></div>' :
        '<div class="node-card"><div class="node-head"><span class="node-name">Node '+(i+1)+'</span><span class="node-status offline"></span></div><div class="node-rows"><div class="node-row"><span class="node-row-label">Round</span><span class="node-row-value">--</span></div><div class="node-row"><span class="node-row-label">Peers</span><span class="node-row-value">--</span></div><div class="node-row"><span class="node-row-label">Latency</span><span class="node-row-value">--</span></div></div></div>';
    }).join('');
  }

  results.forEach(function(r) {
    if (r.ok && r.data) {
      live++;
      if (!best || (r.data.dag_round||0) > (best.dag_round||0)) {
        best = r.data;
      }
    }
  });

  var v = function(id) { return document.getElementById(id); };

  if (live > 0 && best) {
    dot.className = 'live-dot'; lbl.className = 'live-label'; lbl.textContent = 'LIVE';
    var round = best.dag_round || 0;
    var fin = best.last_finalized_round || 0;
    var lag = round - fin;
    var lagDot = document.getElementById('lag-dot');
    var lagText = document.getElementById('lag-text');
    if (lagDot) lagDot.style.background = lag < 5 ? '#34d399' : lag < 10 ? '#fbbf24' : '#f87171';
    if (lagText) lagText.textContent = 'Finality lag: '+lag;

    var el = document.getElementById('dag-round');
    if (el) el.textContent = round.toLocaleString();

    if (v('v-round')) { v('v-round').textContent = round.toLocaleString(); v('v-round-sub').textContent = currentNet; }
    if (v('v-fin')) { v('v-fin').textContent = fin.toLocaleString(); v('v-fin-sub').textContent = lag + ' behind'; }
    var supply = best.total_supply || 0;
    if (v('v-supply')) { v('v-supply').textContent = fmt(supply/SATS) + ' UDAG'; }
    var pct = Math.min((supply/SATS)/MAX_SUPPLY*100, 100);
    if (v('v-supply-sub')) v('v-supply-sub').textContent = pct.toFixed(4) + '% of 21M';
    if (v('supply-fill')) v('supply-fill').style.width = pct + '%';
    if (v('v-treasury')) v('v-treasury').textContent = fmt((best.treasury_balance||0)/SATS) + ' UDAG';
    if (v('v-verts')) v('v-verts').textContent = (best.dag_vertices||0).toLocaleString();
    if (v('v-val')) v('v-val').textContent = best.validator_count || best.active_stakers || 0;
    if (v('v-peers')) v('v-peers').textContent = (best.peer_count||best.connected_peers||0).toLocaleString();
    if (v('v-mem')) v('v-mem').textContent = (best.mempool_size||0).toLocaleString();
    if (v('v-accts')) v('v-accts').textContent = (best.account_count||best.total_accounts||0).toLocaleString();

    ['h-consensus','h-p2p','h-rpc','h-mempool'].forEach(function(id) { var e = v(id); if (e) e.className = 'health-dot healthy'; });

    // Fetch validators
    fetch(NODES[0]+'/validators', {signal: AbortSignal.timeout(4000)}).then(function(r){return r.json();}).then(function(d){
      var vb = document.getElementById('val-body');
      var vals = d.validators||[];
      if (vb) vb.innerHTML = vals.length ? vals.map(function(x){return '<tr><td class="addr">'+hash(x.address)+'</td><td>'+fmt((x.stake||0)/SATS)+'</td><td>'+(x.commission||10)+'%</td><td><span class="active-dot"></span>Active</td></tr>';}).join('') : '<tr><td colspan="4" style="text-align:center;color:var(--muted)">No active validators</td></tr>';
    }).catch(function(){var vb = document.getElementById('val-body'); if (vb) vb.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--muted)">Unable to load</td></tr>';});

    // Fetch mempool
    fetch(NODES[0]+'/mempool', {signal: AbortSignal.timeout(4000)}).then(function(r){return r.json();}).then(function(d){
      var mp = document.getElementById('mempool');
      var m = Array.isArray(d) ? d : [];
      if (mp) mp.innerHTML = '<div class="mp-row hdr"><span>Hash</span><span>Type</span><span>From</span><span>Fee</span></div>' + (m.length ? m.slice(0,100).map(function(x){return '<div class="mp-row"><span class="mp-hash">'+hash(x.hash||x.id)+'</span><span class="mp-type">'+(x.type||'tx')+'</span><span class="mp-from">'+hash(x.from||x.sender)+'</span><span class="mp-fee">'+fmt((x.fee||0)/SATS)+'</span></div>';}).join('') : '<div class="empty-state">No pending transactions</div>');
    }).catch(function(){var mp = document.getElementById('mempool'); if (mp) mp.innerHTML = '<div class="mp-row hdr"><span>Hash</span><span>Type</span><span>From</span><span>Fee</span></div><div class="empty-state">Unable to load</div>';});
  } else {
    if (dot) dot.className = 'live-dot offline';
    if (lbl) lbl.className = 'live-label offline';
    if (lbl) lbl.textContent = currentNet === 'mainnet' ? 'MAINNET NOT LIVE' : 'OFFLINE';
    ['h-consensus','h-p2p'].forEach(function(id) { var e = v(id); if (e) e.className = 'health-dot unhealthy'; });
    ['h-rpc','h-mempool'].forEach(function(id) { var e = v(id); if (e) e.className = 'health-dot unhealthy'; });
  }

  countdown = 5;
  if (arc) {
    var circ = 2*Math.PI*12;
    var tick = function() {
      countdown--;
      arc.style.strokeDasharray = circ;
      arc.style.strokeDashoffset = circ - (countdown/5)*circ;
      if (countdown > 0) { updateTimer = setTimeout(tick, 1000); } else { update(); }
    };
    updateTimer = setTimeout(tick, 1000);
  }
}

// Network switch buttons
var btnTest = document.getElementById('btn-testnet');
var btnMain = document.getElementById('btn-mainnet');
if (btnTest) btnTest.onclick = function() { switchNet('testnet'); };
if (btnMain) btnMain.onclick = function() { switchNet('mainnet'); };

// Start
if (document.readyState === 'loading') { document.addEventListener('DOMContentLoaded', update); } else { update(); }
