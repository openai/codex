// Codex Landing Page JavaScript

// Smooth scroll for anchor links
document.querySelectorAll('a[href^="#"]').forEach(anchor => {
    anchor.addEventListener('click', function (e) {
        e.preventDefault();
        const target = document.querySelector(this.getAttribute('href'));
        if (target) {
            target.scrollIntoView({
                behavior: 'smooth',
                block: 'start'
            });
        }
    });
});

// Pricing toggle (Monthly/Annual)
const pricingToggle = document.querySelectorAll('.toggle-btn');
pricingToggle.forEach(btn => {
    btn.addEventListener('click', function() {
        pricingToggle.forEach(b => b.classList.remove('active'));
        this.classList.add('active');
        
        const billing = this.dataset.billing;
        updatePricing(billing);
    });
});

function updatePricing(billing) {
    const prices = {
        monthly: {
            pro: '$15',
            team: '$50',
            desktop: '$99'
        },
        annual: {
            pro: '$12',  // 20% off
            team: '$40', // 20% off
            desktop: '$99'  // No change for one-time
        }
    };
    
    // Update prices if billing type changes
    // This is a placeholder - actual implementation would update DOM
    console.log(`Switched to ${billing} billing`);
}

// Intersection Observer for fade-in animations
const observerOptions = {
    threshold: 0.1,
    rootMargin: '0px 0px -50px 0px'
};

const observer = new IntersectionObserver(entries => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.style.opacity = '1';
            entry.target.style.transform = 'translateY(0)';
        }
    });
}, observerOptions);

// Observe all feature cards and pricing cards
document.querySelectorAll('.feature-card, .pricing-card, .testimonial-card').forEach(card => {
    card.style.opacity = '0';
    card.style.transform = 'translateY(30px)';
    card.style.transition = 'opacity 0.6s ease, transform 0.6s ease';
    observer.observe(card);
});

// Stats counter animation
function animateCounter(element, target, duration = 2000) {
    const start = 0;
    const increment = target / (duration / 16);
    let current = start;
    
    const timer = setInterval(() => {
        current += increment;
        if (current >= target) {
            element.textContent = target;
            clearInterval(timer);
        } else {
            element.textContent = Math.floor(current);
        }
    }, 16);
}

// Trigger counter animation when stats are in view
const statsObserver = new IntersectionObserver(entries => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            const statValue = entry.target.querySelector('.stat-value');
            if (statValue && !statValue.dataset.animated) {
                const text = statValue.textContent;
                const number = parseInt(text.match(/\d+/)?.[0]);
                if (number) {
                    statValue.dataset.animated = 'true';
                    animateCounter(statValue, number);
                }
            }
        }
    });
}, { threshold: 0.5 });

document.querySelectorAll('.stat').forEach(stat => {
    statsObserver.observe(stat);
});

// Mobile menu toggle (if needed in future)
function toggleMobileMenu() {
    const navLinks = document.querySelector('.nav-links');
    navLinks.classList.toggle('mobile-active');
}

// Form handling (placeholder for future signup forms)
function handleSignup(event) {
    event.preventDefault();
    const formData = new FormData(event.target);
    console.log('Signup form submitted:', Object.fromEntries(formData));
    
    // TODO: Integrate with Stripe/Auth0
    alert('Thank you for your interest! We\'ll contact you soon.');
}

// Video player enhancements
const videoIframe = document.querySelector('.video-container iframe');
if (videoIframe) {
    // Lazy load video
    const videoObserver = new IntersectionObserver(entries => {
        entries.forEach(entry => {
            if (entry.isIntersecting && !videoIframe.src) {
                videoIframe.src = videoIframe.dataset.src;
            }
        });
    });
    
    if (videoIframe.dataset.src) {
        videoObserver.observe(videoIframe);
    }
}

// Track analytics (placeholder)
function trackEvent(category, action, label) {
    console.log('Analytics Event:', { category, action, label });
    
    // TODO: Integrate with Google Analytics / Plausible
    // gtag('event', action, { category, label });
}

// Track CTA clicks
document.querySelectorAll('.btn-primary, .btn-secondary').forEach(btn => {
    btn.addEventListener('click', function() {
        const label = this.textContent.trim();
        trackEvent('CTA', 'click', label);
    });
});

// Feature card tracking
document.querySelectorAll('.feature-card').forEach(card => {
    card.addEventListener('click', function() {
        const feature = this.querySelector('h3').textContent;
        trackEvent('Feature', 'view', feature);
    });
});

// Pricing card tracking
document.querySelectorAll('.pricing-card').forEach(card => {
    card.addEventListener('mouseenter', function() {
        const tier = this.querySelector('h3').textContent;
        trackEvent('Pricing', 'hover', tier);
    });
});

// Keyboard shortcuts
document.addEventListener('keydown', function(e) {
    // Cmd/Ctrl + K for search (future feature)
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        console.log('Open search modal');
        // TODO: Implement search modal
    }
});

// Newsletter signup (if added)
function handleNewsletterSignup(email) {
    console.log('Newsletter signup:', email);
    
    // TODO: Integrate with email service (Mailchimp, SendGrid, etc.)
    return fetch('/api/newsletter', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email })
    });
}

// Pricing calculator (for Team tier)
function calculateTeamPrice(seats) {
    const basePrice = 50;
    const baseSeats = 5;
    const additionalSeatPrice = 10;
    
    if (seats <= baseSeats) {
        return basePrice;
    }
    
    const additionalSeats = seats - baseSeats;
    return basePrice + (additionalSeats * additionalSeatPrice);
}

// Add team pricing calculator UI enhancement
const teamCard = document.querySelector('.pricing-card:nth-child(3)');
if (teamCard) {
    const seatsCalculator = document.createElement('div');
    seatsCalculator.className = 'seats-calculator';
    seatsCalculator.innerHTML = `
        <label>Number of seats: <input type="number" min="5" value="5" id="seats-input"></label>
        <div class="calculated-price"></div>
    `;
    
    const seatsInput = seatsCalculator.querySelector('#seats-input');
    const calculatedPrice = seatsCalculator.querySelector('.calculated-price');
    
    seatsInput.addEventListener('input', function() {
        const seats = parseInt(this.value) || 5;
        const price = calculateTeamPrice(seats);
        calculatedPrice.textContent = `$${price}/month`;
    });
    
    // Initialize
    calculatedPrice.textContent = `$50/month`;
}

// Detect user's preferred theme (light/dark)
function detectPreferredTheme() {
    const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    return isDark ? 'dark' : 'light';
}

// Initialize
document.addEventListener('DOMContentLoaded', function() {
    console.log('Codex landing page loaded');
    console.log('Preferred theme:', detectPreferredTheme());
    
    // Add any initialization code here
});

// Export functions for testing
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        calculateTeamPrice,
        trackEvent,
        handleNewsletterSignup
    };
}

