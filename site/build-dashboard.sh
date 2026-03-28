#!/bin/bash
# Build dashboard and copy to site root for Netlify deployment

set -e

echo "🔨 Building dashboard..."
cd site/dashboard

# Clean old build artifacts first
echo "🧹 Cleaning old build..."
rm -rf dist
rm -f assets/*.js assets/*.css

npm run build

echo "📦 Copying built assets..."
rm -rf assets/*.js assets/*.css
# Copy built assets (JS/CSS) but keep the dev index.html intact
cp -r dist/assets/* assets/
# The production index.html goes to index.html (Netlify serves this)
# but we keep a dev copy for local development
cp index.html index.dev.html
cp dist/index.html index.html

echo "✅ Dashboard built and ready for deployment!"
echo ""
echo "Next steps:"
echo "1. git add -A"
echo "2. git commit -m 'Update dashboard build'"
echo "3. git push"
echo ""
echo "Netlify will auto-deploy after push."
