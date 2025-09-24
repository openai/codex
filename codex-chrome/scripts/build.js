#!/usr/bin/env node

/**
 * Build script for Chrome extension
 * Copies manifest and builds with Vite
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
};

function log(message, color = colors.reset) {
  console.log(`${color}${message}${colors.reset}`);
}

function build() {
  try {
    log('\n🔨 Building Codex Chrome Extension...', colors.yellow);
    
    // Clean dist directory
    const distPath = path.join(__dirname, '..', 'dist');
    if (fs.existsSync(distPath)) {
      fs.rmSync(distPath, { recursive: true });
    }
    fs.mkdirSync(distPath, { recursive: true });
    
    // Run Vite build
    log('\n📦 Building with Vite...', colors.yellow);
    execSync('npm run vite:build', { stdio: 'inherit' });
    
    // Copy manifest
    log('\n📄 Copying manifest...', colors.yellow);
    const manifestSrc = path.join(__dirname, '..', 'manifest.json');
    const manifestDest = path.join(distPath, 'manifest.json');
    fs.copyFileSync(manifestSrc, manifestDest);

    // Copy and fix HTML files
    log('\n📄 Copying and fixing HTML files...', colors.yellow);
    const htmlFiles = [
      { src: 'src/sidepanel/sidepanel.html', dest: 'sidepanel.html' },
      { src: 'src/welcome/welcome.html', dest: 'welcome.html' }
    ];

    htmlFiles.forEach(file => {
      const srcPath = path.join(distPath, file.src);
      const destPath = path.join(distPath, file.dest);
      if (fs.existsSync(srcPath)) {
        // Read the HTML file
        let htmlContent = fs.readFileSync(srcPath, 'utf8');

        // Fix paths - remove leading slashes for Chrome extension
        htmlContent = htmlContent
          .replace(/src="\/([^"]+)"/g, 'src="$1"')
          .replace(/href="\/([^"]+)"/g, 'href="$1"');

        // Write the fixed HTML
        fs.writeFileSync(destPath, htmlContent);
        log(`  ✓ Copied and fixed ${file.dest}`, colors.green);
      } else {
        log(`  ⚠ Missing ${file.src}`, colors.yellow);
      }
    });
    
    // Copy static assets if they exist
    const iconsPath = path.join(__dirname, '..', 'icons');
    if (fs.existsSync(iconsPath)) {
      log('\n🎨 Copying icons...', colors.yellow);
      const iconsDest = path.join(distPath, 'icons');
      fs.mkdirSync(iconsDest, { recursive: true });
      
      fs.readdirSync(iconsPath).forEach(file => {
        fs.copyFileSync(
          path.join(iconsPath, file),
          path.join(iconsDest, file)
        );
      });
    }
    
    // Create placeholder icons if they don't exist
    const iconsDest = path.join(distPath, 'icons');
    if (!fs.existsSync(iconsDest)) {
      fs.mkdirSync(iconsDest, { recursive: true });
      
      // Create simple SVG icons as placeholders
      const sizes = [16, 48, 128];
      sizes.forEach(size => {
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
          <rect width="${size}" height="${size}" fill="#4f46e5"/>
          <text x="50%" y="50%" font-family="Arial" font-size="${size * 0.4}" font-weight="bold" fill="white" text-anchor="middle" dominant-baseline="middle">C</text>
        </svg>`;
        
        fs.writeFileSync(
          path.join(iconsDest, `icon${size}.svg`),
          svg
        );
      });
    }
    
    // Check build output
    const requiredFiles = [
      'manifest.json',
      'background.js',
      'content.js',
      'sidepanel.html',
      'welcome.html'
    ];
    
    const missingFiles = requiredFiles.filter(
      file => !fs.existsSync(path.join(distPath, file))
    );
    
    if (missingFiles.length > 0) {
      log(`\n⚠️  Warning: Missing files in build output:`, colors.yellow);
      missingFiles.forEach(file => log(`  - ${file}`, colors.yellow));
    }
    
    log('\n✅ Build complete!', colors.green);
    log(`\n📁 Extension built to: ${distPath}`, colors.green);
    log('\nTo load the extension:', colors.reset);
    log('1. Open Chrome and navigate to chrome://extensions/', colors.reset);
    log('2. Enable "Developer mode"', colors.reset);
    log('3. Click "Load unpacked"', colors.reset);
    log(`4. Select the ${distPath} directory`, colors.reset);
    
  } catch (error) {
    log(`\n❌ Build failed: ${error.message}`, colors.red);
    process.exit(1);
  }
}

// Run build
build();
