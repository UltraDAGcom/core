// UltraDAG Homepage Client Scripts

// ─── Email Submit Handler ─────────────────────────────────────────────────
async function handleEmailSubmit(e) {
  e.preventDefault();
  const form = e.target;
  const emailInput = form.querySelector('input[name="EMAIL"]');
  const btn = form.querySelector('.email-btn, .email-btn-inline');
  const email = emailInput.value.trim();
  if (!email) return false;

  btn.disabled = true;
  const origText = btn.innerHTML;
  btn.innerHTML = '<span style="font-size:12px">Joining...</span>';

  // ── Mailchimp IDs ──
  // Get these from Mailchimp → Audience → Signup forms → Embedded forms.
  // The u value should be 32 hex chars, id is ~10 chars.
  const u = '7c006c449bd3dc3a523bce11d';
  const id = 'daf1702c98';
  const cbName = 'mc_cb_' + Date.now();
  const url = `https://ultradagcom.us12.list-manage.com/subscribe/post-json?u=${u}&id=${id}&EMAIL=${encodeURIComponent(email)}&c=${cbName}`;

  let responded = false;

  window[cbName] = function(resp) {
    responded = true;
    if (resp.result === 'success' || (resp.msg && resp.msg.includes('already subscribed'))) {
      btn.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>';
      btn.style.background = 'var(--success)';
      emailInput.value = '';
      emailInput.placeholder = "You're on the list!";
      emailInput.disabled = true;
    } else {
      console.error('[email] Mailchimp error:', resp.msg);
      btn.innerHTML = '<span style="font-size:11px">Error</span>';
      btn.style.background = 'var(--danger)';
    }
    setTimeout(() => {
      btn.innerHTML = origText;
      btn.style.background = '';
      btn.disabled = false;
      emailInput.disabled = false;
      emailInput.placeholder = 'your@email.com';
    }, 4000);
    try { delete window[cbName]; } catch {}
    try { document.getElementById(cbName)?.remove(); } catch {}
  };

  const script = document.createElement('script');
  script.id = cbName;
  script.src = url;
  // Handle script load failure (404, CORS, network error).
  script.onerror = function() {
    if (!responded) {
      console.error('[email] Mailchimp JSONP failed — check u/id values. Endpoint returned 404 or was blocked.');
      btn.innerHTML = '<span style="font-size:10px">Signup unavailable</span>';
      btn.style.background = 'var(--danger)';
      setTimeout(() => { btn.innerHTML = origText; btn.style.background = ''; btn.disabled = false; }, 4000);
    }
    try { delete window[cbName]; } catch {}
    try { document.getElementById(cbName)?.remove(); } catch {}
  };
  document.body.appendChild(script);

  // Timeout: if no response after 10s, show real error (no fake success).
  setTimeout(() => {
    if (!responded) {
      console.error('[email] Mailchimp timeout — no JSONP callback after 10s.');
      btn.innerHTML = '<span style="font-size:10px">Timed out</span>';
      btn.style.background = 'var(--danger)';
      setTimeout(() => { btn.innerHTML = origText; btn.style.background = ''; btn.disabled = false; }, 4000);
      try { delete window[cbName]; } catch {}
    }
  }, 10000);

  return false;
}

// ─── Token Launch Countdown ──────────────────────────────────────────────
function initCountdown() {
  const LAUNCH_DATE = new Date('2026-05-22T12:00:00Z').getTime();

  function updateCountdown() {
    const now = Date.now();
    const diff = LAUNCH_DATE - now;

    if (diff <= 0) {
      const daysEl = document.getElementById('cd-days');
      if (daysEl) {
        daysEl.textContent = '🚀';
        const h = document.getElementById('cd-hours'); if (h) h.textContent = '';
        const m = document.getElementById('cd-mins'); if (m) m.textContent = '';
        const s = document.getElementById('cd-secs'); if (s) s.textContent = '';
        document.querySelectorAll('.countdown-sep').forEach(el => el.style.display = 'none');
        document.querySelectorAll('.countdown-label').forEach(el => el.textContent = '');
        daysEl.nextElementSibling.textContent = 'Launching Now!';
      }
      const hd = document.getElementById('cd-hero-days'); if (hd) { hd.textContent = '🚀'; }
      const hh = document.getElementById('cd-hero-hours'); if (hh) { hh.textContent = ''; }
      const hm = document.getElementById('cd-hero-mins'); if (hm) { hm.textContent = ''; }
      return;
    }

    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const mins = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    const secs = Math.floor((diff % (1000 * 60)) / 1000);

    const pad = n => String(n).padStart(2, '0');
    
    const cdDays = document.getElementById('cd-days'); if (cdDays) cdDays.textContent = pad(days);
    const cdHours = document.getElementById('cd-hours'); if (cdHours) cdHours.textContent = pad(hours);
    const cdMins = document.getElementById('cd-mins'); if (cdMins) cdMins.textContent = pad(mins);
    const cdSecs = document.getElementById('cd-secs'); if (cdSecs) cdSecs.textContent = pad(secs);
    
    const hd = document.getElementById('cd-hero-days'); if (hd) { hd.textContent = pad(days); }
    const hh = document.getElementById('cd-hero-hours'); if (hh) { hh.textContent = pad(hours); }
    const hm = document.getElementById('cd-hero-mins'); if (hm) { hm.textContent = pad(mins); }
  }

  updateCountdown();
  setInterval(updateCountdown, 1000);
}

// ─── Live Network Bar ────────────────────────────────────────────────────
function initLiveBar() {
  const MAX_SUPPLY = 21_000_000;
  const SATS = 100_000_000;
  const NODES = [
    'https://ultradag-node-1.fly.dev',
    'https://ultradag-node-2.fly.dev',
    'https://ultradag-node-3.fly.dev',
    'https://ultradag-node-4.fly.dev',
    'https://ultradag-node-5.fly.dev'
  ];

  const elRound = document.getElementById('live-round');
  const elLag = document.getElementById('live-lag');
  const elNodes = document.getElementById('live-nodes');
  const elSupply = document.getElementById('live-supply');
  const elSupplyPct = document.getElementById('live-supply-pct');
  const elStatus = document.getElementById('live-status');
  const elLabel = document.getElementById('live-label');

  if (!elRound || !elStatus) return;

  let lastRound = null;
  let lastData = null;
  let staleTimeout = null;

  function formatUdag(sats) {
    const udag = sats / SATS;
    if (udag >= 1_000_000) return (udag / 1_000_000).toFixed(2) + 'M';
    if (udag >= 1_000) return (udag / 1_000).toFixed(1) + 'K';
    return udag.toFixed(0);
  }

  function setLive() {
    elStatus.className = 'live-indicator live';
    elLabel.textContent = 'LIVE';
  }

  function setStale() {
    elStatus.className = 'live-indicator stale';
    elLabel.textContent = 'STALE';
  }

  function tickAnim(el) {
    el.classList.add('tick');
    setTimeout(() => el.classList.remove('tick'), 600);
  }

  async function fetchStatus() {
    let liveCount = 0, bestRound = 0, bestFinalized = 0, bestSupply = 0;

    const results = await Promise.allSettled(
      NODES.map(url =>
        fetch(url + '/status', { signal: AbortSignal.timeout(4000) }).then(r => r.json())
      )
    );

    for (const r of results) {
      if (r.status === 'fulfilled' && r.value && r.value.dag_round !== undefined) {
        liveCount++;
        const d = r.value;
        if (d.dag_round > bestRound) bestRound = d.dag_round;
        if (d.last_finalized_round > bestFinalized) bestFinalized = d.last_finalized_round;
        if (d.total_supply > bestSupply) bestSupply = d.total_supply;
      }
    }

    if (liveCount > 0) {
      lastData = { round: bestRound, finalized: bestFinalized, supply: bestSupply, nodes: liveCount };
      setLive();
      clearTimeout(staleTimeout);
      staleTimeout = setTimeout(setStale, 15000);
    } else if (!lastData) {
      return;
    } else {
      setStale();
    }

    const d = lastData;
    const lag = d.round - d.finalized;
    const supplyPct = ((d.supply / SATS) / MAX_SUPPLY * 100).toFixed(1);

    if (lastRound !== null && d.round !== lastRound) tickAnim(elRound);
    lastRound = d.round;

    elRound.textContent = d.round.toLocaleString();
    elLag.textContent = lag;
    elNodes.innerHTML = d.nodes + '<span class="live-metric-label" style="margin-left:2px">/5</span>';
    elSupply.textContent = formatUdag(d.supply) + ' UDAG';
    elSupplyPct.textContent = supplyPct + '% of 21M';
  }

  fetchStatus();
  setInterval(fetchStatus, 5000);
}

// ─── Scroll Reveal ───────────────────────────────────────────────────────
function initScrollReveal() {
  const reveals = document.querySelectorAll('.reveal');
  if (reveals.length === 0) return;
  
  const observer = new IntersectionObserver((entries) => {
    entries.forEach(e => {
      if (e.isIntersecting) {
        e.target.classList.add('visible');
        observer.unobserve(e.target);
      }
    });
  }, { threshold: 0.12, rootMargin: '0px 0px -40px 0px' });
  reveals.forEach(el => observer.observe(el));
}

// ─── Playground ─────────────────────────────────────────────────────────
function initPlayground() {
  const editor = document.getElementById('code-editor');
  const runBtn = document.getElementById('run-code');
  const output = document.getElementById('output');
  const result = document.getElementById('result');
  const exampleSelect = document.getElementById('example-select');

  if (!editor || !runBtn) return;

  const NODE = 'https://ultradag-node-1.fly.dev';
  const SATS = 100_000_000;

  const examples = [
    {
      name: 'Get Network Status',
      code: '// Fetch network status from testnet\n'
        + 'const res = await fetch("' + NODE + '/status");\n'
        + 'const status = await res.json();\n\n'
        + 'console.log("DAG Round:", status.dag_round);\n'
        + 'console.log("Finalized:", status.last_finalized_round);\n'
        + 'console.log("Lag:", status.dag_round - status.last_finalized_round);\n'
        + 'console.log("Validators:", status.active_stakers || status.validators);\n'
        + 'console.log("Peers:", status.connected_peers || status.peer_count);\n'
        + 'console.log("Mempool:", status.mempool_size, "pending txs");\n\n'
        + 'const supply = (status.total_supply / ' + SATS + ').toFixed(2);\n'
        + 'console.log("Supply:", supply, "UDAG of 21,000,000");'
    },
    {
      name: 'Check Account Balance',
      code: '// Look up an account balance\n'
        + 'const address = "0000000000000000000000000000000000000000000000000000000000000001";\n'
        + 'const res = await fetch("' + NODE + '/balance/" + address);\n'
        + 'const data = await res.json();\n\n'
        + 'console.log("Address:", address.slice(0, 16) + "...");\n'
        + 'console.log("Balance:", data.balance, "sats");\n'
        + 'console.log("Balance:", (data.balance / ' + SATS + ').toFixed(8), "UDAG");\n'
        + 'console.log("Nonce:", data.nonce);'
    },
    {
      name: 'View Recent Round',
      code: '// Get the latest finalized round\'s vertices\n'
        + 'const status = await (await fetch("' + NODE + '/status")).json();\n'
        + 'const round = status.last_finalized_round;\n'
        + 'console.log("Fetching round", round, "...");\n\n'
        + 'const res = await fetch("' + NODE + '/round/" + round);\n'
        + 'const data = await res.json();\n'
        + 'const verts = Array.isArray(data) ? data : (data.vertices || []);\n\n'
        + 'console.log("Round:", round);\n'
        + 'console.log("Vertices:", verts.length);\n\n'
        + 'for (const v of verts) {\n'
        + '  const hash = (v.hash || v.vertex_hash || "").slice(0, 16);\n'
        + '  const validator = (v.validator || v.address || "").slice(0, 16);\n'
        + '  const txCount = v.tx_count != null ? v.tx_count : (v.transactions ? v.transactions.length : 0);\n'
        + '  console.log("  Vertex " + hash + "... by " + validator + "... (" + txCount + " txs)");\n'
        + '}'
    },
    {
      name: 'List Active Validators',
      code: '// List active validators and their stakes\n'
        + 'const res = await fetch("' + NODE + '/validators");\n'
        + 'const data = await res.json();\n'
        + 'const validators = data.validators || data || [];\n\n'
        + 'console.log("Active Validators:", validators.length);\n'
        + 'console.log("---");\n\n'
        + 'for (const v of validators) {\n'
        + '  const addr = (v.address || "").slice(0, 16);\n'
        + '  const stake = v.stake_udag || (v.stake ? (v.stake / ' + SATS + ').toFixed(2) : "?");\n'
        + '  const commission = v.commission_percent != null ? v.commission_percent + "%" : "10%";\n'
        + '  console.log(addr + "...  Stake: " + stake + " UDAG  Commission: " + commission);\n'
        + '}'
    },
    {
      name: 'View Mempool',
      code: '// Check the current mempool\n'
        + 'const res = await fetch("' + NODE + '/mempool");\n'
        + 'const data = await res.json();\n'
        + 'const txs = data.transactions || data || [];\n\n'
        + 'console.log("Mempool size:", txs.length, "transactions");\n\n'
        + 'if (txs.length === 0) {\n'
        + '  console.log("(mempool is empty)");\n'
        + '} else {\n'
        + '  for (const tx of txs.slice(0, 10)) {\n'
        + '    const hash = (tx.hash || "").slice(0, 16);\n'
        + '    const type = tx.type || tx.tx_type || "transfer";\n'
        + '    console.log("  " + type + " " + hash + "...");\n'
        + '  }\n'
        + '}'
    }
  ];

  // Populate examples dropdown
  examples.forEach(function(ex, i) {
    const opt = document.createElement('option');
    opt.value = String(i);
    opt.textContent = ex.name;
    exampleSelect.appendChild(opt);
  });

  // Load first example by default
  editor.value = examples[0].code;

  exampleSelect.addEventListener('change', function() {
    const idx = parseInt(exampleSelect.value);
    if (!isNaN(idx) && examples[idx]) {
      editor.value = examples[idx].code;
      output.textContent = '';
      result.textContent = '';
    }
  });

  // Run code
  runBtn.addEventListener('click', async function() {
    output.textContent = '';
    result.textContent = '';
    runBtn.disabled = true;
    runBtn.textContent = 'Running...';

    var logs = [];
    var fakeConsole = {
      log: function() { logs.push(Array.from(arguments).map(function(a) { return typeof a === 'object' ? JSON.stringify(a, null, 2) : String(a); }).join(' ')); },
      error: function() { logs.push('ERROR: ' + Array.from(arguments).map(String).join(' ')); },
      warn: function() { logs.push('WARN: ' + Array.from(arguments).map(String).join(' ')); },
    };

    try {
      var code = editor.value;
      var fn = new Function('fetch', 'console', 'return (async () => { ' + code + ' })();');
      await fn(fetch.bind(window), fakeConsole);
      output.textContent = logs.join('\n');
      result.textContent = '\n✓ Completed successfully';
      result.style.color = 'var(--success)';
    } catch (e) {
      output.textContent = logs.join('\n');
      result.textContent = '\n✗ Error: ' + e.message;
      result.style.color = 'var(--danger)';
    }

    runBtn.disabled = false;
    runBtn.textContent = 'Run Code';
  });

  // Tab key in editor
  editor.addEventListener('keydown', function(e) {
    if (e.key === 'Tab') {
      e.preventDefault();
      var start = editor.selectionStart;
      var end = editor.selectionEnd;
      editor.value = editor.value.substring(0, start) + '  ' + editor.value.substring(end);
      editor.selectionStart = editor.selectionEnd = start + 2;
    }
  });
}

// ─── Initialize ──────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  initCountdown();
  initLiveBar();
  initScrollReveal();
  initPlayground();
});

// Export for global access
window.handleEmailSubmit = handleEmailSubmit;
