#!/bin/bash

# UltraDAG MkDocs Deployment Script for Netlify
set -e

echo "🚀 Building UltraDAG documentation for Netlify deployment..."

# Check if MkDocs is installed
if ! command -v mkdocs &> /dev/null; then
    echo "❌ MkDocs not found. Installing..."
    pip install mkdocs mkdocs-material pymdown-extensions
fi

# Clean previous build
echo "🧹 Cleaning previous build..."
rm -rf site/docs

# Build documentation
echo "📚 Building documentation..."
mkdocs build --clean

# Verify build
if [ ! -f "site/docs/index.html" ]; then
    echo "❌ Build failed - index.html not found"
    exit 1
fi

echo "✅ Documentation built successfully!"
echo "📁 Built in: site/docs/"
echo "🌐 Ready for Netlify deployment"
echo ""
echo "📋 Next steps:"
echo "1. Push to GitHub repository"
echo "2. Connect repository to Netlify"
echo "3. Set build command: mkdocs build"
echo "4. Set publish directory: site/docs"
echo "5. Deploy!"
echo ""
echo "🔗 Documentation will be available at: https://docs.ultradag.com"
