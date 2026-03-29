// UltraDAG Network Page Script
const NODES = ['https://ultradag-node-1.fly.dev','https://ultradag-node-2.fly.dev','https://ultradag-node-3.fly.dev','https://ultradag-node-4.fly.dev','https://ultradag-node-5.fly.dev'];
const SATS = 100000000, MAX_SUPPLY = 21000000;
let countdown = 5;

function fmt(n) { return n >= 1e6 ? (n/1e6).toFixed(2)+'M' : n >= 1e3 ? (n/1e3).toFixed(1)+'K' : n.toFixed(0); }
function hash(h) { return h && h.length > 12 ? h.slice(0,6)+'...'+h.slice(-4) : h||'--'; }

async function update() {
  const dot = document.getElementById('live-dot');
  const lbl = document.getElementById('live-label');
  const arc = document.getElementById('refresh-arc');
  
  if (!dot || !lbl) return;
  
  const results = await Promise.all(NODES.map(async (url, i) => {
    const t0 = Date.now();
    try {
      const r = await fetch(url+'/status', {signal: AbortSignal.timeout(4000)});
      return {ok: true, data: await r.json(), lat: Date.now()-t0, url};
    } catch { return {ok: false, lat: null, url}; }
  }));

  let live = 0, round = 0, fin = 0, supply = 0, treasury = 0, verts = 0, val = 0, peers = 0, mem = 0, accts = 0;
  
  const nodesDiv = document.getElementById('nodes');
  if (nodesDiv) {
    nodesDiv.innerHTML = results.map((r, i) => r.ok && r.data ? 
      `<div class="node-card"><div class="node-head"><span class="node-name">Node ${i+1}</span><span class="node-status online"></span></div><div class="node-rows"><div class="node-row"><span class="node-row-label">Round</span><span class="node-row-value">${r.data.dag_round||'--'}</span></div><div class="node-row"><span class="node-row-label">Peers</span><span class="node-row-value">${r.data.peer_count||'--'}</span></div><div class="node-row"><span class="node-row-label">Latency</span><span class="node-row-value">${r.lat}ms</span></div></div></div>` :
      `<div class="node-card"><div class="node-head"><span class="node-name">Node ${i+1}</span><span class="node-status offline"></span></div><div class="node-rows"><div class="node-row"><span class="node-row-label">Round</span><span class="node-row-value">--</span></div><div class="node-row"><span class="node-row-label">Peers</span><span class="node-row-value">--</span></div><div class="node-row"><span class="node-row-label">Latency</span><span class="node-row-value">--</span></div></div></div>`
    ).join('');
  }

  results.forEach(r => {
    if (r.ok && r.data) {
      live++;
      if (r.data.dag_round > round) { round = r.data.dag_round; fin = r.data.last_finalized_round||0; supply = r.data.total_supply||0; treasury = r.data.treasury_balance||0; val = r.data.validator_count||0; }
      verts += r.data.dag_vertices||0;
      peers += r.data.peer_count||0;
      mem += r.data.mempool_size||0;
      accts += r.data.account_count||0;
    }
  });

  if (live > 0) {
    dot.className = 'live-dot'; lbl.className = 'live-label'; lbl.textContent = 'LIVE';
    const lag = round - fin;
    const lagDot = document.getElementById('lag-dot');
    const lagText = document.getElementById('lag-text');
    if (lagDot) lagDot.style.background = lag < 5 ? '#34d399' : lag < 10 ? '#fbbf24' : '#f87171';
    if (lagText) lagText.textContent = 'Finality lag: '+lag;
    
    const el = document.getElementById('dag-round');
    if (el) el.textContent = round.toLocaleString();
    
    const v = id => document.getElementById(id);
    if (v('v-round')) { v('v-round').textContent = round.toLocaleString(); v('v-round-sub').textContent = 'testnet'; }
    if (v('v-fin')) { v('v-fin').textContent = (fin||0).toLocaleString(); v('v-fin-sub').textContent = lag + ' behind'; }
    if (v('v-supply')) { v('v-supply').textContent = fmt(supply/SATS) + ' UDAG'; }
    const pct = Math.min((supply/SATS)/MAX_SUPPLY*100, 100);
    if (v('v-supply-sub')) v('v-supply-sub').textContent = pct.toFixed(4) + '% of 21M';
    if (v('supply-fill')) v('supply-fill').style.width = pct + '%';
    if (v('v-treasury')) v('v-treasury').textContent = fmt(treasury/SATS) + ' UDAG';
    if (v('v-verts')) v('v-verts').textContent = verts.toLocaleString();
    if (v('v-val')) v('v-val').textContent = val;
    if (v('v-peers')) v('v-peers').textContent = peers.toLocaleString();
    if (v('v-mem')) v('v-mem').textContent = mem.toLocaleString();
    if (v('v-accts')) v('v-accts').textContent = accts.toLocaleString();
    
    ['h-consensus','h-p2p','h-rpc','h-mempool'].forEach(id => { const e = v(id); if (e) e.className = 'health-dot healthy'; });
    
    // Fetch validators
    fetch(NODES[0]+'/validators').then(r=>r.json()).then(d=>{
      const vb = document.getElementById('val-body');
      const v = d.validators||[];
      if (vb) vb.innerHTML = v.length ? v.map(x=>`<tr><td class="addr">${hash(x.address)}</td><td>${fmt((x.stake||0)/SATS)}</td><td>${x.commission||10}%</td><td><span class="active-dot"></span>Active</td></tr>`).join('') : '<tr><td colspan="4" style="text-align:center;color:var(--muted)">No active validators</td></tr>';
    }).catch(()=>{const vb = document.getElementById('val-body'); if (vb) vb.innerHTML = '<tr><td colspan="4" style="text-align:center;color:var(--muted)">Unable to load</td></tr>';});
    
    // Fetch mempool
    fetch(NODES[0]+'/mempool').then(r=>r.json()).then(d=>{
      const mp = document.getElementById('mempool');
      const m = Array.isArray(d) ? d : [];
      if (mp) mp.innerHTML = '<div class="mp-row hdr"><span>Hash</span><span>Type</span><span>From</span><span>Fee</span></div>' + (m.length ? m.slice(0,100).map(x=>`<div class="mp-row"><span class="mp-hash">${hash(x.hash||x.id)}</span><span class="mp-type">${x.type||'tx'}</span><span class="mp-from">${hash(x.from||x.sender)}</span><span class="mp-fee">${fmt((x.fee||0)/SATS)}</span></div>`).join('') : '<div class="empty-state">No pending transactions</div>');
    }).catch(()=>{const mp = document.getElementById('mempool'); if (mp) mp.innerHTML = '<div class="mp-row hdr"><span>Hash</span><span>Type</span><span>From</span><span>Fee</span></div><div class="empty-state">Unable to load</div>';});
  } else {
    if (dot) dot.className = 'live-dot offline';
    if (lbl) lbl.className = 'live-label offline';
    if (lbl) lbl.textContent = 'OFFLINE';
    ['h-consensus','h-p2p'].forEach(id => { const e = v(id); if (e) e.className = 'health-dot unhealthy'; });
  }

  countdown = 5;
  if (arc) {
    const circ = 2*Math.PI*12;
    const tick = () => {
      countdown--;
      arc.style.strokeDasharray = circ;
      arc.style.strokeDashoffset = circ - (countdown/5)*circ;
      if (countdown > 0) setTimeout(tick, 1000); else update();
    };
    setTimeout(tick, 1000);
  }
}

// Network switch buttons
const btnTest = document.getElementById('btn-testnet');
const btnMain = document.getElementById('btn-mainnet');
if (btnTest) btnTest.onclick = () => { btnTest.classList.add('active'); btnMain.classList.remove('active'); update(); };
if (btnMain) btnMain.onclick = () => { btnMain.classList.add('active'); btnTest.classList.remove('active'); update(); };

// Start when DOM is ready
if (document.readyState === 'loading') { document.addEventListener('DOMContentLoaded', update); } else { update(); }
