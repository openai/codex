# AGENTS.md

## Project Overview
This is **The Sovereign Beta Portal** - a high-converting astrology landing page for selling digital products and services. The page features:
- A mystical, dark-themed design with animated cosmic backgrounds
- Three monetized offerings: Guide ($22), Transit Report ($77), Strategy Call ($222)
- Birth chart data collection forms for personalized astrology services
- Countdown timer for limited-time portal access
- Modal-based checkout flows

## Tech Stack
- **Framework**: Pure HTML/CSS/JavaScript (no build step required)
- **Styling**: Tailwind CSS (via CDN)
- **Fonts**: Google Fonts (Inter + Playfair Display)
- **JavaScript**: Vanilla ES6+
- **Target**: Modern browsers with backdrop-filter support

## File Structure
```
project/
├── index.html          # Main landing page (single file)
├── AGENTS.md          # This file
└── README.md          # Human-readable documentation
```

## Design Principles

### Visual Identity
- **Color Palette**: 
  - Primary: Emerald (#10b981)
  - Secondary: Purple (#7c3aed)
  - Accent: Gold/Amber (#fbbf24)
  - Background: Dark stone tones (#1a1614, #0f0d0c)
- **Typography**: Serif headers (Playfair Display), sans-serif body (Inter)
- **Effects**: Glassmorphism, glowing buttons, animated cosmic orbs, rotating zodiac wheel

### Readability Requirements
- All text must be readable with contrast ratio ≥ 4.5:1
- Use `#fafaf9` (bright), `#e7e5e4` (medium), `#d6d3d1` (soft) for text hierarchy
- Input fields must have `rgba(255, 255, 255, 0.1)` backgrounds minimum
- Never use pure black backgrounds - always use dark stone tones

### Animation Guidelines
- Subtle, performance-conscious animations only
- Use `transform` and `opacity` for GPU acceleration
- Floating orbs: 8s ease-in-out infinite
- Zodiac wheel: 60s linear infinite rotation
- Button hover: 0.3s ease transitions

## Code Style

### HTML
- Semantic HTML5 elements
- BEM-like class naming for custom styles
- Tailwind utility classes for layout and spacing
- Include accessibility attributes (aria-labels, proper form labels)

### CSS
- Use CSS custom properties for repeated values
- Prefer `rem` units over `px` for accessibility
- Mobile-first responsive design
- Glassmorphism: `backdrop-filter: blur(20px)` + semi-transparent backgrounds

### JavaScript
- ES6+ syntax (const/let, arrow functions, template literals)
- Event delegation where appropriate
- Form validation before submission
- No external dependencies beyond CDN resources

## Development Commands

### Local Development
```
# Serve locally (any static server)
python -m http.server 8000
# or
npx serve .
```

### Testing
```
# No build step required - open directly in browser
open index.html
```

### Browser Compatibility Testing
- Chrome/Edge: Latest 2 versions
- Firefox: Latest 2 versions
- Safari: Latest 2 versions
- Mobile: iOS Safari 14+, Chrome Mobile

## Form Integration

### Payment Processors
Replace placeholder links with actual payment URLs:
```javascript
// In modals, update these href attributes:
href="[YOUR_GUMROAD_OR_STRIPE_LINK]"

// For forms, integrate with payment processor in submit handlers:
document.getElementById('transitForm').addEventListener('submit', function(e) {
    e.preventDefault();
    const formData = new FormData(this);
    // Send to payment processor with birth data
    window.location.href = `https://buy.stripe.com/...?​client_reference_id=${data.email}`;
});
```

### Required Integrations
1. **Guide ($22)**: Simple payment link (Gumroad/Stripe Payment Link)
2. **Transit Report ($77)**: Payment + form data collection + email delivery
3. **Strategy Call ($222)**: Payment + form data + Calendly/booking integration

### Birth Data Fields
All astrology services require:
- Full name (text)
- Email (email)
- Birth date (date)
- Birth time (time)
- Timezone (select)
- Birth location (text - city, country)
- Optional notes/focus areas (textarea)

## Testing Instructions

### Visual Testing
- [ ] All text is readable on dark backgrounds
- [ ] Buttons show hover states with glow effects
- [ ] Modals animate in smoothly
- [ ] Countdown timer updates every second
- [ ] Zodiac wheel rotates continuously
- [ ] Floating orbs move without jank

### Functional Testing
- [ ] All modal open/close buttons work
- [ ] Click outside modal closes it
- [ ] Form validation prevents empty submissions
- [ ] Countdown calculates correctly to Oct 31, 2025 midnight EST
- [ ] All links point to correct destinations
- [ ] Forms collect all required birth data

### Responsive Testing
- [ ] Layout works on mobile (320px+)
- [ ] Two-column grid becomes single column on mobile
- [ ] All touch targets are ≥44x44px
- [ ] Text remains readable at all breakpoints

### Performance Testing
- [ ] Page loads in <3s on 3G
- [ ] Animations run at 60fps
- [ ] No layout shifts on load
- [ ] Images/SVGs load without blocking

## Content Guidelines

### Copy Tone
- Mystical but authoritative
- Use astrological terminology (houses, aspects, transits)
- Empower the reader ("sovereign", "karmic code", "activation")
- Balance esoteric language with practical benefits

### Astrology Symbols
Use Unicode symbols for planets and signs:
- ☉ Sun
- ☽ Moon
- ☿ Mercury
- ♀ Venus
- ♂ Mars
- ♃ Jupiter
- ♄ Saturn
- ♅ Uranus
- ♆ Neptune
- ♇ Pluto
- ♈-♓ Zodiac signs (Aries through Pisces)

## Security Considerations

### Form Data
- Never store birth data in localStorage (could be sensitive)
- Always use HTTPS for form submissions
- Include GDPR-compliant privacy notice if serving EU users
- Birth time is considered personal data in some jurisdictions

### Payment Security
- Never handle credit card data directly
- Use payment processor's hosted checkout or embedded forms
- Validate all form inputs server-side (even if validated client-side)
- Include clear refund/cancellation policies

## Deployment

### Pre-deployment Checklist
- [ ] Replace all `[YOUR_..._LINK]` placeholders with actual URLs
- [ ] Test countdown timer with correct timezone
- [ ] Verify payment links work and redirect correctly
- [ ] Add Google Analytics/tracking if needed
- [ ] Configure email delivery for digital products
- [ ] Test form submissions end-to-end
- [ ] Optimize images (if replacing SVG placeholders)
- [ ] Add meta tags for SEO and social sharing

### Hosting Recommendations
- **Static Hosts**: Vercel, Netlify, Cloudflare Pages (free tier works)
- **Traditional**: Any web host with HTTPS support
- **CDN**: Not required but recommended for global audience

### Environment Variables
If using serverless functions for form processing:
```
STRIPE_SECRET_KEY=sk_live_...
SENDGRID_API_KEY=SG...
CALENDLY_API_TOKEN=...
```

## Common Tasks

### Update Countdown Date
```javascript
// Line ~520 in the script
const countdownDate = new Date("2025-10-31T00:00:00-04:00").getTime();
```

### Change Color Scheme
Update these CSS custom properties:
```css
/* Primary color (emerald) */
.glow-button { background: linear-gradient(135deg, #059669 0%, #10b981 100%); }

/* Secondary color (purple) */
.glow-button-secondary { background: linear-gradient(135deg, #7c3aed 0%, #8b5cf6 100%); }

/* Accent color (gold) */
.glow-button-gold { background: linear-gradient(135deg, #d97706 0%, #f59e0b 100%); }
```

### Add New Service
1. Create new price card in services section
2. Add corresponding modal with form
3. Wire up modal open/close handlers
4. Add form submit handler
5. Update payment integration

### Adjust Readability
If text isn't readable enough:
```css
/* Increase opacity of card backgrounds */
.card-border { background: rgba(41, 37, 36, 0.95); /* was 0.92 */ }

/* Brighten text colors */
.text-bright { color: #ffffff; /* was #fafaf9 */ }
```

## Troubleshooting

### Countdown Not Working
- Check timezone in Date constructor
- Verify current date is before countdown date
- Inspect console for JavaScript errors

### Forms Not Submitting
- Check preventDefault() is called
- Verify all required fields have values
- Inspect network tab for submission errors

### Modals Not Opening
- Check modal ID matches button onclick
- Verify openModal/closeModal functions exist
- Inspect console for errors

### Animations Choppy
- Reduce blur intensity on backdrop-filter
- Simplify floating orb animations
- Use will-change CSS property sparingly

## AI Agent Instructions

When modifying this project:
1. **Always maintain readability** - dark themes must have sufficient contrast
2. **Preserve the mystical aesthetic** - keep cosmic themes, zodiac symbols, glowing effects
3. **Test forms thoroughly** - birth data collection is core functionality
4. **Update comments** - explain complex calculations (countdown, form validation)
5. **Keep it single-file** - unless explicitly asked to split, maintain the single HTML structure
6. **Responsive-first** - test mobile layouts after any change
7. **Performance matters** - avoid adding heavy libraries or blocking scripts

When asked to add features:
- Check if it enhances conversion or user experience
- Maintain design consistency with existing elements
- Update this AGENTS.md if new patterns are introduced
- Test on mobile devices

When fixing bugs:
- Identify root cause before patching
- Test the fix across all breakpoints
- Verify no regressions in other areas
- Update documentation if behavior changes
