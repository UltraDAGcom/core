#!/bin/bash
# Build dashboard and copy to site root for Netlify deployment

set -e

echo "🔨 Building dashboard..."
cd site/dashboard

# Ensure dev index.html is in place for vite (it may have been overwritten by a previous build)
DEV_HTML='<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/dashboard/favicon.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>UltraDAG Dashboard</title>
    <script type="module" src="/src/main.tsx"></script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>'
echo "$DEV_HTML" > index.html

# Clean old build artifacts
echo "🧹 Cleaning old build..."
rm -rf dist
rm -f assets/*.js assets/*.css

npm run build

echo "📦 Copying built assets..."
rm -f assets/*.js assets/*.css
cp -r dist/assets/* assets/
# Replace index.html with built version for Netlify
cp dist/index.html index.html

echo "✅ Dashboard built and ready for deployment!"
echo ""
echo "Next steps:"
echo "1. git add -A"
echo "2. git commit -m 'Update dashboard build'"
echo "3. git push"
echo ""
echo "Netlify will auto-deploy after push."
