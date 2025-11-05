# Codex Landing Page

Modern, responsive landing page for Codex - World's First AI-Native Operating System

## Features

- 🎨 **Modern Design**: Tech-focused, glassmorphism effects, gradient accents
- 📱 **Fully Responsive**: Works on desktop, tablet, and mobile
- ⚡ **Performance Optimized**: Lazy loading, smooth animations, minimal JS
- 🎯 **Conversion Focused**: Clear CTAs, pricing comparison, social proof
- ♿ **Accessible**: ARIA labels, semantic HTML, keyboard navigation

## Structure

```
website/
├── index.html          # Main HTML file
├── styles.css          # CSS styles
├── script.js           # JavaScript functionality
├── favicon.svg         # Site favicon
└── README.md           # This file
```

## Sections

1. **Hero** - Eye-catching header with key value props and demo video
2. **Features** - 4 main features with detailed descriptions
3. **Comparison** - Table comparing Codex vs competitors
4. **Pricing** - 4 tiers (Free, Pro, Team, Enterprise) + Desktop option
5. **Testimonials** - Social proof from users
6. **CTA** - Final call-to-action with stats
7. **Footer** - Links, legal, social media

## Setup

### Local Development

1. Clone the repository
2. Open `index.html` in a browser
3. Or use a local server:

```bash
# Python
python -m http.server 8000

# Node.js
npx serve

# PHP
php -S localhost:8000
```

4. Visit `http://localhost:8000`

### Domain Setup

**Recommended domain**: `codex.ai` or `codex.dev`

1. Register domain at Namecheap, Google Domains, or Cloudflare
2. Point DNS to hosting provider
3. Set up SSL certificate (Let's Encrypt via Certbot)

### Hosting Options

#### Option 1: Static Hosting (Recommended)

- **Vercel** (Recommended)
  ```bash
  npm install -g vercel
  vercel deploy
  ```
  
- **Netlify**
  ```bash
  npm install -g netlify-cli
  netlify deploy
  ```

- **Cloudflare Pages**
  - Connect GitHub repo
  - Auto-deploy on push

#### Option 2: Traditional Hosting

- **AWS S3 + CloudFront**
  - High availability
  - Global CDN
  - ~$5-10/month

- **DigitalOcean App Platform**
  - Simple deployment
  - Auto-scaling
  - $12/month starter

## Customization

### Colors

Edit CSS variables in `styles.css`:

```css
:root {
    --primary: #667eea;
    --secondary: #764ba2;
    --accent: #f093fb;
    /* ... */
}
```

### Content

1. **Hero Title**: Edit `index.html` line 40-44
2. **Pricing**: Update prices in `index.html` line 300-400
3. **Features**: Modify feature cards line 150-250
4. **Testimonials**: Replace testimonials line 450-500

### Video

Replace YouTube embed URL in `index.html` line 70:

```html
<iframe src="https://www.youtube.com/embed/YOUR_VIDEO_ID"></iframe>
```

## Integration Tasks

### 1. Domain Purchase

- [ ] Purchase `codex.ai` or alternative domain
- [ ] Configure DNS settings
- [ ] Set up SSL certificate

### 2. Analytics

Add Google Analytics or Plausible:

```html
<!-- Add before </head> -->
<script async src="https://www.googletagmanager.com/gtag/js?id=GA_TRACKING_ID"></script>
<script>
  window.dataLayer = window.dataLayer || [];
  function gtag(){dataLayer.push(arguments);}
  gtag('js', new Date());
  gtag('config', 'GA_TRACKING_ID');
</script>
```

### 3. Stripe Integration

For signup buttons:

```html
<a href="https://buy.stripe.com/your_payment_link" class="btn btn-primary">
  Start Free Trial
</a>
```

### 4. Email Capture

Add Mailchimp or ConvertKit form:

```html
<form action="https://mailchimp.com/subscribe" method="POST">
  <input type="email" name="email" placeholder="Enter your email">
  <button type="submit">Subscribe</button>
</form>
```

### 5. Live Chat

Add Intercom or Crisp:

```html
<!-- Add before </body> -->
<script>
  window.intercomSettings = { app_id: "YOUR_APP_ID" };
</script>
<script>(function(){...Intercom code...})();</script>
```

## Performance

### Optimization Checklist

- [x] Minify CSS/JS (use build tool)
- [x] Optimize images (use WebP format)
- [x] Lazy load images/videos
- [x] Enable gzip compression
- [x] Set cache headers
- [ ] Use CDN for assets

### Expected Metrics

- **Lighthouse Score**: 95+
- **First Contentful Paint**: < 1.5s
- **Time to Interactive**: < 3s
- **Total Bundle Size**: < 200KB

## SEO

### Meta Tags

Already included in `index.html`:

```html
<meta name="description" content="...">
<meta property="og:title" content="...">
<meta property="og:description" content="...">
<meta name="twitter:card" content="...">
```

### Sitemap

Create `sitemap.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://codex.ai/</loc>
    <changefreq>weekly</changefreq>
    <priority>1.0</priority>
  </url>
</urlset>
```

### robots.txt

Create `robots.txt`:

```
User-agent: *
Allow: /
Sitemap: https://codex.ai/sitemap.xml
```

## Deployment Checklist

- [ ] Test on all major browsers (Chrome, Firefox, Safari, Edge)
- [ ] Test on mobile devices (iOS, Android)
- [ ] Verify all links work
- [ ] Replace placeholder images
- [ ] Add actual YouTube video
- [ ] Set up domain and SSL
- [ ] Configure analytics
- [ ] Set up Stripe payment links
- [ ] Add email capture
- [ ] Submit sitemap to Google Search Console
- [ ] Test page speed (Lighthouse)

## Maintenance

### Regular Updates

- Update testimonials monthly
- Refresh pricing if changed
- Add new features as released
- Update GitHub stars count
- Refresh screenshots/videos

### A/B Testing

Test variations of:
- Hero headline
- CTA button text
- Pricing display
- Feature descriptions

## Support

For questions or issues:

- GitHub Issues: https://github.com/zapabob/codex/issues
- Discord: https://discord.gg/codex
- Email: support@codex.ai

## License

Same as parent project (Apache 2.0)

