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

echo "📦 Copying built files..."
rm -rf assets/*.js assets/*.css
cp -r dist/* .

echo "✅ Dashboard built and ready for deployment!"
echo ""
echo "Next steps:"
echo "1. git add -A"
echo "2. git commit -m 'Update dashboard build'"
echo "3. git push"
echo ""
echo "Netlify will auto-deploy after push."
