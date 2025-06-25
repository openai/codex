#!/usr/bin/env node
function checkNodeVersion(version = process.versions.node, requiredMajor = 22) {
  const major = parseInt(version.split('.')[0], 10);
  return major >= requiredMajor;
}

if (require.main === module) {
  if (!checkNodeVersion()) {
    console.error(
      `Detected Node.js ${process.versions.node} but ${22}+ is required. Please upgrade from https://nodejs.org and re-run this script.`
    );
    process.exit(1);
  }
}

module.exports = { checkNodeVersion };
