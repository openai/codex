#!/usr/bin/env node

/**
 * CSV Headers Management Script
 * Validates and manages CSV headers for DTG data files
 */

import fs from 'fs/promises';
import path from 'path';

// Expected headers for each CSV file
const EXPECTED_HEADERS = {
  'DecryptTheGirl_Analytics.csv': [
    'ts_iso',
    'event_id', 
    'reader_id',
    'surface',
    'action',
    'node_id',
    'version',
    'session_id',
    'artifact_href',
    'notes',
    'sentient_cents_earned'
  ],
  'DecryptTheGirl_Deploys.csv': [
    'ts_iso',
    'version',
    'source_repo',
    'artifact_href',
    'node_id',
    'event_id'
  ],
  'AVC_IP_MasterLedger_v1.csv': [
    'ts_iso',
    'event_id',
    'node_id',
    'action',
    'hash_sha256',
    'reader_id',
    'notes'
  ]
};

/**
 * Read and parse CSV headers from a file
 */
async function readCsvHeaders(filePath) {
  try {
    const content = await fs.readFile(filePath, 'utf-8');
    const lines = content.trim().split('\n');
    if (lines.length === 0) return [];
    
    return lines[0].split(',').map(header => header.trim());
  } catch (error) {
    console.error(`Error reading ${filePath}:`, error.message);
    return null;
  }
}

/**
 * Write CSV headers to a file
 */
async function writeCsvHeaders(filePath, headers) {
  try {
    await fs.writeFile(filePath, headers.join(',') + '\n');
    console.log(`✓ Updated headers for ${path.basename(filePath)}`);
    return true;
  } catch (error) {
    console.error(`Error writing ${filePath}:`, error.message);
    return false;
  }
}

/**
 * Validate CSV headers against expected schema
 */
function validateHeaders(fileName, actualHeaders, expectedHeaders) {
  const issues = [];
  
  // Check for missing headers
  for (const expected of expectedHeaders) {
    if (!actualHeaders.includes(expected)) {
      issues.push(`Missing header: ${expected}`);
    }
  }
  
  // Check for extra headers
  for (const actual of actualHeaders) {
    if (!expectedHeaders.includes(actual)) {
      issues.push(`Unexpected header: ${actual}`);
    }
  }
  
  // Check header order
  const correctOrder = expectedHeaders.every((header, index) => 
    actualHeaders[index] === header
  );
  
  if (!correctOrder && issues.length === 0) {
    issues.push('Headers are present but in wrong order');
  }
  
  return issues;
}

/**
 * Fix CSV headers by rewriting them in the correct order
 */
async function fixCsvHeaders(filePath, expectedHeaders) {
  const fileName = path.basename(filePath);
  console.log(`🔧 Fixing headers for ${fileName}...`);
  
  const success = await writeCsvHeaders(filePath, expectedHeaders);
  return success;
}

/**
 * Main validation and fixing logic
 */
async function processFile(filePath) {
  const fileName = path.basename(filePath);
  const expectedHeaders = EXPECTED_HEADERS[fileName];
  
  if (!expectedHeaders) {
    console.log(`⚠️  No header schema defined for ${fileName}`);
    return { valid: true, issues: [] };
  }
  
  console.log(`\n📋 Processing ${fileName}...`);
  
  const actualHeaders = await readCsvHeaders(filePath);
  if (actualHeaders === null) {
    return { valid: false, issues: ['Failed to read file'] };
  }
  
  const issues = validateHeaders(fileName, actualHeaders, expectedHeaders);
  
  if (issues.length === 0) {
    console.log(`✅ Headers are valid for ${fileName}`);
    return { valid: true, issues: [] };
  } else {
    console.log(`❌ Header issues found in ${fileName}:`);
    issues.forEach(issue => console.log(`   - ${issue}`));
    return { valid: false, issues };
  }
}

/**
 * CLI interface
 */
async function main() {
  const args = process.argv.slice(2);
  const command = args[0];
  
  // Get data directory path
  const dataDir = path.join(process.cwd(), 'data');
  
  try {
    await fs.access(dataDir);
  } catch {
    console.error('❌ Data directory not found. Run this script from the project root.');
    process.exit(1);
  }
  
  if (command === 'validate') {
    console.log('🔍 Validating CSV headers...');
    
    let allValid = true;
    for (const fileName of Object.keys(EXPECTED_HEADERS)) {
      const filePath = path.join(dataDir, fileName);
      const result = await processFile(filePath);
      if (!result.valid) {
        allValid = false;
      }
    }
    
    if (allValid) {
      console.log('\n🎉 All CSV headers are valid!');
      process.exit(0);
    } else {
      console.log('\n💥 Some CSV headers need fixing. Run with "fix" command.');
      process.exit(1);
    }
    
  } else if (command === 'fix') {
    console.log('🔧 Fixing CSV headers...');
    
    for (const fileName of Object.keys(EXPECTED_HEADERS)) {
      const filePath = path.join(dataDir, fileName);
      const expectedHeaders = EXPECTED_HEADERS[fileName];
      await fixCsvHeaders(filePath, expectedHeaders);
    }
    
    console.log('\n🎉 CSV headers have been fixed!');
    
  } else if (command === 'list') {
    console.log('📄 Expected CSV headers:\n');
    
    for (const [fileName, headers] of Object.entries(EXPECTED_HEADERS)) {
      console.log(`${fileName}:`);
      headers.forEach((header, index) => {
        console.log(`  ${index + 1}. ${header}`);
      });
      console.log('');
    }
    
  } else {
    console.log(`
📋 CSV Headers Management Tool

Usage:
  node csv_headers.mjs <command>

Commands:
  validate    Check if CSV headers match expected schema
  fix         Fix CSV headers by rewriting them correctly  
  list        Show expected headers for all CSV files

Examples:
  node csv_headers.mjs validate
  node csv_headers.mjs fix
  node csv_headers.mjs list
`);
    process.exit(1);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error('💥 Script failed:', error.message);
    process.exit(1);
  });
}