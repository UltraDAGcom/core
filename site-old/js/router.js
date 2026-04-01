// Simple hash-based router

const routes = new Map();
let currentView = null;

export function register(name, mountFn) {
  routes.set(name, mountFn);
}

export function navigate(name) {
  window.location.hash = '#' + name;
}

export function currentRoute() {
  return window.location.hash.slice(1) || 'dashboard';
}

export function start(container) {
  function render() {
    const route = currentRoute();
    const mountFn = routes.get(route);
    if (!mountFn) {
      navigate('dashboard');
      return;
    }
    container.innerHTML = '';
    currentView = mountFn(container);
  }

  window.addEventListener('hashchange', render);
  render();
}

export function getCleanup() {
  return currentView?.cleanup;
}
