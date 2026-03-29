// UltraDAG Homepage JavaScript

// ─── Email Submit Handler ─────────────────────────────────────────────────
function handleEmailSubmit(e) {
  e.preventDefault();
  const form = e.target;
  const emailInput = form.querySelector('input[name="EMAIL"]');
  const btn = form.querySelector('.email-btn, .email-btn-inline');
  const email = emailInput.value.trim();
  if (!email) return false;

  // Disable while submitting
  btn.disabled = true;
  const origText = btn.innerHTML;
  btn.innerHTML = '<span style="font-size:12px">Joining...</span>';

  // Mailchimp JSONP endpoint
  const u = '7c006c449bd3dc3a523bce11d';
  const id = 'daf1702c98';
  const cbName = 'mc_cb_' + Date.now();
  const url = `https://ultradagcom.us12.list-manage.com/subscribe/post-json?u=${u}&id=${id}&EMAIL=${encodeURIComponent(email)}&c=${cbName}`;

  // JSONP callback
  window[cbName] = function(resp) {
    if (resp.result === 'success' || (resp.msg && resp.msg.includes('already subscribed'))) {
      btn.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>';
      btn.style.background = 'var(--success)';
      emailInput.value = '';
      emailInput.placeholder = "You're on the list!";
      emailInput.disabled = true;
    } else {
      btn.innerHTML = '<span style="font-size:11px">Try again</span>';
      btn.style.background = 'var(--danger)';
    }
    // Reset after 4s
    setTimeout(() => {
      btn.innerHTML = origText;
      btn.style.background = '';
      btn.disabled = false;
      emailInput.disabled = false;
      emailInput.placeholder = 'your@email.com';
    }, 4000);
    // Cleanup
    try { delete window[cbName]; } catch {}
    try { document.getElementById(cbName)?.remove(); } catch {}
  };

  // Inject JSONP script
  const script = document.createElement('script');
  script.id = cbName;
  script.src = url;
  document.body.appendChild(script);

  // Timeout fallback
  setTimeout(() => {
    if (window[cbName]) {
      window[cbName]({ result: 'success', msg: '' });
    }
  }, 8000);

  return false;
}

// ─── Token Launch Countdown ──────────────────────────────────────────────
(function() {
  const LAUNCH_DATE = new Date('2026-05-22T12:00:00Z').getTime();

  function updateCountdown() {
    const now = Date.now();
    const diff = LAUNCH_DATE - now;

    if (diff <= 0) {
      const daysEl = document.getElementById('cd-days');
      if (daysEl) {
        daysEl.textContent = '🚀';
        document.getElementById('cd-hours')!.textContent = '';
        document.getElementById('cd-mins')!.textContent = '';
        document.getElementById('cd-secs')!.textContent = '';
        document.querySelectorAll('.countdown-sep').forEach(el => el.style.display = 'none');
        document.querySelectorAll('.countdown-label').forEach(el => el.textContent = '');
        daysEl.nextElementSibling!.textContent = 'Launching Now!';
      }
      // Update hero inline countdown too
      const hd = document.getElementById('cd-hero-days');
      if (hd) { hd.textContent = '🚀'; }
      const hh = document.getElementById('cd-hero-hours');
      if (hh) { hh.textContent = ''; }
      const hm = document.getElementById('cd-hero-mins');
      if (hm) { hm.textContent = ''; }
      return;
    }

    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const mins = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    const secs = Math.floor((diff % (1000 * 60)) / 1000);

    const pad = n => String(n).padStart(2, '0');
    
    // Main countdown
    const cdDays = document.getElementById('cd-days');
    if (cdDays) cdDays.textContent = pad(days);
    const cdHours = document.getElementById('cd-hours');
    if (cdHours) cdHours.textContent = pad(hours);
    const cdMins = document.getElementById('cd-mins');
    if (cdMins) cdMins.textContent = pad(mins);
    const cdSecs = document.getElementById('cd-secs');
    if (cdSecs) cdSecs.textContent = pad(secs);
    
    // Hero inline countdown
    const hd = document.getElementById('cd-hero-days');
    if (hd) { hd.textContent = pad(days); }
    const hh = document.getElementById('cd-hero-hours');
    if (hh) { hh.textContent = pad(hours); }
    const hm = document.getElementById('cd-hero-mins');
    if (hm) { hm.textContent = pad(mins); }
  }

  updateCountdown();
  setInterval(updateCountdown, 1000);
})();

// ─── Live Network Bar ────────────────────────────────────────────────────
(function() {
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
    elStatus!.className = 'live-indicator live';
    elLabel!.textContent = 'LIVE';
  }

  function setStale() {
    elStatus!.className = 'live-indicator stale';
    elLabel!.textContent = 'STALE';
  }

  function tickAnim(el) {
    el.classList.add('tick');
    setTimeout(() => el.classList.remove('tick'), 600);
  }

  async function fetchStatus() {
    let liveCount = 0;
    let bestRound = 0;
    let bestFinalized = 0;
    let bestSupply = 0;
    let gotAny = false;

    const results = await Promise.allSettled(
      NODES.map(url =>
        fetch(url + '/status', { signal: AbortSignal.timeout(4000) })
          .then(r => r.json())
      )
    );

    for (const r of results) {
      if (r.status === 'fulfilled' && r.value && r.value.dag_round !== undefined) {
        liveCount++;
        gotAny = true;
        const d = r.value;
        if (d.dag_round > bestRound) bestRound = d.dag_round;
        if (d.last_finalized_round > bestFinalized) bestFinalized = d.last_finalized_round;
        if (d.total_supply > bestSupply) bestSupply = d.total_supply;
      }
    }

    if (gotAny) {
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

    if (lastRound !== null && d.round !== lastRound) tickAnim(elRound!);
    lastRound = d.round;

    elRound!.textContent = d.round.toLocaleString();
    elLag!.textContent = lag;
    elNodes!.innerHTML = d.nodes + '<span class="live-metric-label" style="margin-left:2px">/5</span>';
    elSupply!.textContent = formatUdag(d.supply) + ' UDAG';
    elSupplyPct!.textContent = supplyPct + '% of 21M';
  }

  fetchStatus();
  setInterval(fetchStatus, 5000);
})();

// ─── Scroll Reveal ───────────────────────────────────────────────────────
(function() {
  const reveals = document.querySelectorAll('.reveal');
  const observer = new IntersectionObserver((entries) => {
    entries.forEach(e => {
      if (e.isIntersecting) {
        e.target.classList.add('visible');
        observer.unobserve(e.target);
      }
    });
  }, { threshold: 0.12, rootMargin: '0px 0px -40px 0px' });
  reveals.forEach(el => observer.observe(el));
})();

// ─── Navigation Scroll Effect ────────────────────────────────────────────
(function() {
  const nav = document.getElementById('nav');
  window.addEventListener('scroll', () => {
    if (nav) {
      nav.classList.toggle('scrolled', window.scrollY > 40);
    }
  });
})();

// ─── Mobile Menu ─────────────────────────────────────────────────────────
(function() {
  const hamburger = document.getElementById('hamburger');
  const mobileMenu = document.getElementById('mobile-menu');
  
  hamburger?.addEventListener('click', () => {
    hamburger.classList.toggle('active');
    mobileMenu?.classList.toggle('active');
  });

  // Close menu when clicking a link
  mobileMenu?.querySelectorAll('a').forEach(link => {
    link.addEventListener('click', () => {
      hamburger?.classList.remove('active');
      mobileMenu?.classList.remove('active');
    });
  });
})();

// Export for global access
window.handleEmailSubmit = handleEmailSubmit;
