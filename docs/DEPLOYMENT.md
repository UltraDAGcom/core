# UltraDAG Documentation Deployment Guide

## 🚀 Deploying to Netlify

### Prerequisites
- Netlify account
- GitHub repository with UltraDAG code
- MkDocs dependencies installed

### Option 1: Automatic Deployment (Recommended)

1. **Connect Repository to Netlify**
   - Go to [Netlify](https://app.netlify.com)
   - Click "Add new site" → "Import an existing project"
   - Connect your GitHub account
   - Select the `UltraDAGcom/core` repository

2. **Configure Build Settings**
   ```
   Build command: mkdocs build --clean
   Publish directory: site/docs
   ```

3. **Set Environment Variables**
   - `PYTHON_VERSION`: 3.11

4. **Deploy**
   - Click "Deploy site"
   - Documentation will be available at: `https://docs.ultradag.com`

### Option 2: Manual Deployment

1. **Build Documentation Locally**
   ```bash
   # Install dependencies
   pip install mkdocs mkdocs-material pymdown-extensions
   
   # Build documentation
   mkdocs build --clean
   
   # Deploy the site/docs folder to your hosting provider
   ```

2. **Use Deployment Script**
   ```bash
   chmod +x deploy-docs.sh
   ./deploy-docs.sh
   ```

### Option 3: GitHub Actions (Advanced)

1. **Set Netlify Secrets**
   - `NETLIFY_AUTH_TOKEN`: Your Netlify personal access token
   - `NETLIFY_SITE_ID`: Your Netlify site ID

2. **Push Changes**
   - Any push to `main` branch will trigger automatic deployment
   - Pull requests will build preview deployments

## 📋 Configuration Files

### `netlify.toml`
- Configures build settings
- Sets up redirects
- Adds security headers
- Defines environment variables

### `.github/workflows/deploy-docs.yml`
- GitHub Actions workflow
- Builds documentation on push
- Deploys to Netlify automatically

## 🔧 Custom Domain Setup

1. **In Netlify Dashboard**
   - Go to Site settings → Domain management
   - Add custom domain: `docs.ultradag.com`

2. **DNS Configuration**
   ```
   Type: CNAME
   Name: docs
   Value: your-site-name.netlify.app
   ```

3. **SSL Certificate**
   - Netlify automatically provisions SSL certificate
   - HTTPS redirects are configured automatically

## 📊 Monitoring

### Netlify Analytics
- Built-in visitor analytics
- Performance metrics
- Error tracking

### Search Functionality
- Full-text search powered by MkDocs
- Automatic indexing
- Fast search results

## 🔄 Updates

### Automatic Updates
- Documentation updates on every push to `main`
- Preview deployments for pull requests
- Rollback capability in Netlify

### Manual Updates
```bash
# Rebuild documentation
mkdocs build --clean

# Redeploy to Netlify
netlify deploy --prod --dir=site/docs
```

## 🐛 Troubleshooting

### Common Issues

1. **Build Fails**
   - Check Python version (requires 3.11)
   - Verify MkDocs dependencies
   - Check for syntax errors in markdown files

2. **Links Broken**
   - Verify all links point to `https://docs.ultradag.com`
   - Check for relative paths in navigation

3. **Search Not Working**
   - Rebuild search index: `mkdocs build`
   - Clear browser cache

### Debug Mode
```bash
# Build with verbose output
mkdocs build --clean --verbose

# Local development server
mkdocs serve
```

## 📱 Mobile Support

The documentation is fully responsive and works on:
- Desktop browsers
- Tablets
- Mobile devices

## 🔒 Security

- HTTPS enforced by default
- Security headers configured
- No user data collection
- GDPR compliant

## 📈 Performance

- Optimized CSS/JS delivery
- Image compression
- CDN distribution via Netlify
- Fast search functionality

---

**Need help?** Check the [full documentation](https://docs.ultradag.com) or open an issue on GitHub.
