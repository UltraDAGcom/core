// Shared utilities

export function formatUdag(sats) {
  if (sats === undefined || sats === null) return '0';
  const val = (sats / 100_000_000);
  return val % 1 === 0 ? val.toFixed(0) : val.toFixed(8).replace(/\.?0+$/, '');
}

export function formatSats(sats) {
  if (sats === undefined || sats === null) return '0';
  return Number(sats).toLocaleString();
}

export function shortAddr(addr) {
  if (!addr) return '';
  return addr.slice(0, 8) + '...' + addr.slice(-6);
}

export function shortHash(hash) {
  if (!hash) return '';
  return hash.slice(0, 12) + '...';
}

export function copyText(text) {
  navigator.clipboard.writeText(text);
  toast('Copied', 'ok');
}

let toastEl = null;
let toastTimer = null;

export function toast(msg, type = 'info') {
  if (!toastEl) toastEl = document.getElementById('toast');
  toastEl.textContent = msg;
  toastEl.className = 'toast toast-' + type + ' show';
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove('show'), 3500);
}

export function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (k.startsWith('on')) {
        el.addEventListener(k.slice(2).toLowerCase(), v);
      } else if (k === 'className') {
        el.className = v;
      } else if (k === 'html') {
        el.innerHTML = v;
      } else {
        el.setAttribute(k, v);
      }
    }
  }
  for (const child of children) {
    if (typeof child === 'string') {
      el.appendChild(document.createTextNode(child));
    } else if (child instanceof Node) {
      el.appendChild(child);
    }
  }
  return el;
}
