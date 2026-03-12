// UltraDAG Theme Switcher
// Handles dark/light mode toggle with localStorage persistence

(function() {
  'use strict';
  
  // Get theme from localStorage or default to dark
  const getTheme = () => localStorage.getItem('ultradag-theme') || 'dark';
  
  // Set theme
  const setTheme = (theme) => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('ultradag-theme', theme);
    updateToggleButton(theme);
  };
  
  // Update toggle button appearance
  const updateToggleButton = (theme) => {
    const toggleBtn = document.getElementById('theme-toggle');
    if (toggleBtn) {
      toggleBtn.innerHTML = theme === 'dark' ? '☀️' : '🌙';
      toggleBtn.setAttribute('aria-label', `Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`);
      toggleBtn.setAttribute('title', `Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`);
    }
  };
  
  // Toggle theme
  window.toggleTheme = function() {
    const currentTheme = getTheme();
    const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
    setTheme(newTheme);
  };
  
  // Initialize theme on page load
  const initTheme = () => {
    const theme = getTheme();
    setTheme(theme);
  };
  
  // Run on DOM ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initTheme);
  } else {
    initTheme();
  }
})();
