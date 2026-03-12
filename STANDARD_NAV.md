# Standard Navigation Structure

This is the EXACT navigation that should be on EVERY page:

```html
<nav>
  <a href="index.html" class="nav-logo">
    <div class="nav-logo-mark"></div>
    <span>UltraDAG</span>
  </a>
  <ul class="nav-links">
    <li><a href="index.html">Home</a></li>
    
    <li>
      <a href="whitepaper.html">Whitepaper</a>
      <div class="mega-menu">
        <div class="mega-cols">
          <div class="mega-col">
            <h4>Core Concepts</h4>
            <ul>
              <li><a href="whitepaper.html#consensus">DAG-BFT Consensus</a></li>
              <li><a href="whitepaper.html#finality">Fast Finality</a></li>
              <li><a href="whitepaper.html#architecture">Architecture</a></li>
            </ul>
          </div>
          <div class="mega-col">
            <h4>Advanced Topics</h4>
            <ul>
              <li><a href="whitepaper.html#staking">Staking & Validation</a></li>
              <li><a href="whitepaper.html#governance">Governance</a></li>
              <li><a href="whitepaper.html#checkpoints">Checkpoints</a></li>
              <li><a href="whitepaper.html#security">Security Model</a></li>
            </ul>
          </div>
          <div class="mega-col">
            <h4>Technical Details</h4>
            <ul>
              <li><a href="whitepaper.html#tokenomics">Tokenomics</a></li>
              <li><a href="whitepaper.html#pruning">Pruning</a></li>
              <li><a href="whitepaper.html#formal">Formal Verification</a></li>
            </ul>
          </div>
        </div>
      </div>
    </li>
    
    <li>
      <a href="docs.html">Docs</a>
      <div class="mega-menu">
        <div class="mega-cols">
          <div class="mega-col">
            <h4>Getting Started</h4>
            <ul>
              <li><a href="docs.html#api">HTTP RPC API</a></li>
              <li><a href="docs.html#sdk">SDK Quickstart</a></li>
              <li><a href="docs.html#node">Running a Node</a></li>
            </ul>
          </div>
          <div class="mega-col">
            <h4>Validators</h4>
            <ul>
              <li><a href="docs.html#staking">Staking & Validation</a></li>
              <li><a href="docs.html#governance">Governance</a></li>
              <li><a href="docs.html#monitoring">Metrics & Monitoring</a></li>
            </ul>
          </div>
          <div class="mega-col">
            <h4>Reference</h4>
            <ul>
              <li><a href="docs.html#arch">Architecture</a></li>
              <li><a href="docs.html#transactions">Transaction Format</a></li>
              <li><a href="docs.html#checkpoints">Checkpoints & Fast-Sync</a></li>
              <li><a href="docs.html#troubleshooting">Troubleshooting</a></li>
            </ul>
          </div>
        </div>
      </div>
    </li>
    
    <li><a href="explorer.html">Explorer</a></li>
    <li><a href="blog/index.html">Blog</a></li>
    <li><a href="https://github.com/UltraDAGcom/core" target="_blank">GitHub</a></li>
    <li><a href="network.html" class="nav-cta">Live Network</a></li>
    <li><a href="dashboard.html" class="nav-cta">Dashboard</a></li>
  </ul>
  <button class="hamburger" id="hamburger" aria-label="Menu">
    <span></span>
    <span></span>
    <span></span>
  </button>
</nav>

<!-- MOBILE MENU -->
<div class="mobile-menu" id="mobile-menu">
  <ul>
    <li><a href="index.html">Home</a></li>
    <li><a href="whitepaper.html">Whitepaper</a></li>
    <li><a href="docs.html">Docs</a></li>
    <li><a href="explorer.html">Explorer</a></li>
    <li><a href="blog/index.html">Blog</a></li>
    <li><a href="https://github.com/UltraDAGcom/core" target="_blank">GitHub</a></li>
    <li><a href="network.html" class="nav-cta">Live Network</a></li>
    <li><a href="dashboard.html" class="nav-cta">Dashboard</a></li>
  </ul>
</div>
```

## For Blog Pages (adjust paths with ../)
Same structure but all links should have `../` prefix except for blog links.
