#!/usr/bin/env node

/**
 * Batch Normalization Script
 * Normalizes and processes batches of DTG event data for ingestion
 */

import fs from 'fs/promises';
import path from 'path';
import crypto from 'crypto';

// Configuration
const DATA_DIR = path.join(process.cwd(), 'data');
const BATCH_SIZE = 100;

// Sentient Cents earning rules (matching worker.js)
const SENTIENT_CENTS_RULES = {
  keystroke: 0.01,    // 1 cent per keystroke
  view: 0.05,         // 5 cents per page view
  click: 0.02,        // 2 cents per click
  scroll: 0.001,      // 0.1 cent per scroll event
  submit: 0.10,       // 10 cents per form submission
  deploy: 1.00,       // 100 cents per deployment
  mint: 0.00,         // No earning for minting itself
  validate: 0.25      // 25 cents per validation
};

/**
 * Calculate Sentient Cents based on action and context
 */
function calculateSentientCents(action, context = {}) {
  const baseRate = SENTIENT_CENTS_RULES[action] || 0;
  
  // Apply multipliers based on context
  let multiplier = 1;
  
  // Bonus for engagement quality
  if (context.engagement_duration && context.engagement_duration > 10) {
    multiplier += 0.1; // 10% bonus for sustained engagement
  }
  
  // Bonus for content creation
  if (context.content_length && context.content_length > 100) {
    multiplier += 0.2; // 20% bonus for substantial content
  }
  
  // Bonus for unique contributions
  if (context.is_unique) {
    multiplier += 0.5; // 50% bonus for unique content
  }
  
  return Math.round(baseRate * multiplier * 100) / 100; // Round to 2 decimal places
}
function generateUUID() {
  return crypto.randomUUID();
}

/**
 * Calculate SHA256 hash of data
 */
function calculateSHA256(data) {
  return crypto.createHash('sha256').update(data).digest('hex');
}

/**
 * Validate ISO timestamp format
 */
function validateISOTimestamp(timestamp) {
  const iso8601Regex = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z?$/;
  return iso8601Regex.test(timestamp);
}

/**
 * Normalize timestamp to ISO format
 */
function normalizeTimestamp(timestamp) {
  if (!timestamp) {
    return new Date().toISOString();
  }
  
  // If already valid ISO, return as-is
  if (validateISOTimestamp(timestamp)) {
    return timestamp;
  }
  
  // Try to parse and convert to ISO
  try {
    const date = new Date(timestamp);
    if (isNaN(date.getTime())) {
      throw new Error('Invalid date');
    }
    return date.toISOString();
  } catch {
    console.warn(`Invalid timestamp "${timestamp}", using current time`);
    return new Date().toISOString();
  }
}

/**
 * Validate and normalize an analytics event
 */
function normalizeAnalyticsEvent(event) {
  const normalized = {
    ts_iso: normalizeTimestamp(event.ts_iso || event.timestamp),
    event_id: event.event_id || generateUUID(),
    reader_id: event.reader_id || event.userId || 'anonymous',
    surface: event.surface || 'web',
    action: event.action || 'view',
    node_id: event.node_id || event.nodeId || '',
    version: event.version || 'v1.0.0',
    session_id: event.session_id || event.sessionId || generateUUID(),
    artifact_href: event.artifact_href || event.artifactHref || '',
    notes: event.notes || '',
    sentient_cents_earned: event.sentient_cents_earned || calculateSentientCents(event.action, event.context || {})
  };
  
  // Validate required fields
  if (!normalized.action) {
    throw new Error('Missing required field: action');
  }
  
  const validActions = ['view', 'click', 'scroll', 'keystroke', 'submit', 'deploy', 'mint', 'validate'];
  if (!validActions.includes(normalized.action)) {
    throw new Error(`Invalid action: ${normalized.action}`);
  }
  
  return normalized;
}

/**
 * Validate and normalize a deploy event
 */
function normalizeDeployEvent(event) {
  const normalized = {
    ts_iso: normalizeTimestamp(event.ts_iso || event.timestamp),
    version: event.version || 'v1.0.0',
    source_repo: event.source_repo || event.sourceRepo || '',
    artifact_href: event.artifact_href || event.artifactHref || '',
    node_id: event.node_id || event.nodeId || '',
    event_id: event.event_id || generateUUID()
  };
  
  // Validate required fields
  if (!normalized.version) {
    throw new Error('Missing required field: version');
  }
  
  return normalized;
}

/**
 * Validate and normalize a master ledger event
 */
function normalizeLedgerEvent(event) {
  const eventData = JSON.stringify(event, Object.keys(event).sort());
  
  const normalized = {
    ts_iso: normalizeTimestamp(event.ts_iso || event.timestamp),
    event_id: event.event_id || generateUUID(),
    node_id: event.node_id || event.nodeId || '',
    action: event.action || 'validate',
    hash_sha256: event.hash_sha256 || calculateSHA256(eventData),
    reader_id: event.reader_id || event.userId || 'system',
    notes: event.notes || ''
  };
  
  // Validate required fields
  if (!normalized.action) {
    throw new Error('Missing required field: action');
  }
  
  return normalized;
}

/**
 * Convert object to CSV row
 */
function objectToCsvRow(obj, headers) {
  return headers.map(header => {
    const value = obj[header] || '';
    // Escape quotes and wrap in quotes if contains comma
    if (typeof value === 'string' && (value.includes(',') || value.includes('"'))) {
      return `"${value.replace(/"/g, '""')}"`;
    }
    return value;
  }).join(',');
}

/**
 * Append data to CSV file
 */
async function appendToCsv(filePath, data, headers) {
  try {
    // Check if file exists and has headers
    let fileExists = false;
    try {
      await fs.access(filePath);
      fileExists = true;
    } catch {
      // File doesn't exist
    }
    
    let csvContent = '';
    
    // Add headers if file doesn't exist
    if (!fileExists) {
      csvContent += headers.join(',') + '\n';
    }
    
    // Add data rows
    for (const item of data) {
      csvContent += objectToCsvRow(item, headers) + '\n';
    }
    
    await fs.appendFile(filePath, csvContent);
    return true;
  } catch (error) {
    console.error(`Failed to append to ${filePath}:`, error.message);
    return false;
  }
}

/**
 * Update proof ledger with new events
 */
async function updateProofLedger(events) {
  const ledgerPath = path.join(DATA_DIR, 'proof_ledger.json');
  
  try {
    let ledger = [];
    
    // Read existing ledger
    try {
      const ledgerContent = await fs.readFile(ledgerPath, 'utf-8');
      ledger = JSON.parse(ledgerContent);
    } catch {
      // File doesn't exist or is empty, start with empty array
    }
    
    // Add new events to ledger
    for (const event of events) {
      const proofEntry = {
        timestamp: event.ts_iso,
        event_id: event.event_id,
        hash: event.hash_sha256 || calculateSHA256(JSON.stringify(event)),
        action: event.action,
        reader_id: event.reader_id || 'system'
      };
      ledger.push(proofEntry);
    }
    
    // Write updated ledger
    await fs.writeFile(ledgerPath, JSON.stringify(ledger, null, 2));
    console.log(`✓ Updated proof ledger with ${events.length} entries`);
    return true;
  } catch (error) {
    console.error('Failed to update proof ledger:', error.message);
    return false;
  }
}

/**
 * Process a batch of events
 */
async function processBatch(events, type = 'analytics') {
  console.log(`📊 Processing batch of ${events.length} ${type} events...`);
  
  const normalized = [];
  const errors = [];
  
  for (const [index, event] of events.entries()) {
    try {
      let normalizedEvent;
      
      switch (type) {
        case 'analytics':
          normalizedEvent = normalizeAnalyticsEvent(event);
          break;
        case 'deploy':
          normalizedEvent = normalizeDeployEvent(event);
          break;
        case 'ledger':
          normalizedEvent = normalizeLedgerEvent(event);
          break;
        default:
          throw new Error(`Unknown event type: ${type}`);
      }
      
      normalized.push(normalizedEvent);
    } catch (error) {
      errors.push({ index, error: error.message, event });
    }
  }
  
  if (errors.length > 0) {
    console.warn(`⚠️  ${errors.length} events failed normalization:`);
    errors.forEach(({ index, error }) => {
      console.warn(`   Event ${index}: ${error}`);
    });
  }
  
  console.log(`✅ Successfully normalized ${normalized.length} events`);
  return { normalized, errors };
}

/**
 * Save normalized events to appropriate CSV files
 */
async function saveNormalizedEvents(events, type) {
  const fileMap = {
    analytics: {
      file: 'DecryptTheGirl_Analytics.csv',
      headers: ['ts_iso', 'event_id', 'reader_id', 'surface', 'action', 'node_id', 'version', 'session_id', 'artifact_href', 'notes', 'sentient_cents_earned']
    },
    deploy: {
      file: 'DecryptTheGirl_Deploys.csv',
      headers: ['ts_iso', 'version', 'source_repo', 'artifact_href', 'node_id', 'event_id']
    },
    ledger: {
      file: 'AVC_IP_MasterLedger_v1.csv',
      headers: ['ts_iso', 'event_id', 'node_id', 'action', 'hash_sha256', 'reader_id', 'notes']
    }
  };
  
  const config = fileMap[type];
  if (!config) {
    throw new Error(`Unknown event type: ${type}`);
  }
  
  const filePath = path.join(DATA_DIR, config.file);
  const success = await appendToCsv(filePath, events, config.headers);
  
  if (success) {
    console.log(`✓ Saved ${events.length} events to ${config.file}`);
    
    // Update proof ledger for all event types
    await updateProofLedger(events);
  }
  
  return success;
}

/**
 * Main CLI interface
 */
async function main() {
  const args = process.argv.slice(2);
  const command = args[0];
  
  if (command === 'process') {
    const inputFile = args[1];
    const eventType = args[2] || 'analytics';
    
    if (!inputFile) {
      console.error('❌ Please provide input file path');
      process.exit(1);
    }
    
    try {
      console.log(`📥 Reading events from ${inputFile}...`);
      const content = await fs.readFile(inputFile, 'utf-8');
      const events = JSON.parse(content);
      
      if (!Array.isArray(events)) {
        throw new Error('Input file must contain an array of events');
      }
      
      console.log(`Found ${events.length} events to process`);
      
      // Process events in batches
      const batches = [];
      for (let i = 0; i < events.length; i += BATCH_SIZE) {
        batches.push(events.slice(i, i + BATCH_SIZE));
      }
      
      let totalProcessed = 0;
      let totalErrors = 0;
      
      for (const [batchIndex, batch] of batches.entries()) {
        console.log(`\n🔄 Processing batch ${batchIndex + 1}/${batches.length}...`);
        
        const { normalized, errors } = await processBatch(batch, eventType);
        totalProcessed += normalized.length;
        totalErrors += errors.length;
        
        if (normalized.length > 0) {
          await saveNormalizedEvents(normalized, eventType);
        }
      }
      
      console.log(`\n🎉 Processing complete!`);
      console.log(`   ✅ Successfully processed: ${totalProcessed} events`);
      console.log(`   ❌ Failed to process: ${totalErrors} events`);
      
    } catch (error) {
      console.error('💥 Processing failed:', error.message);
      process.exit(1);
    }
    
  } else if (command === 'validate') {
    const inputFile = args[1];
    
    if (!inputFile) {
      console.error('❌ Please provide input file path');
      process.exit(1);
    }
    
    try {
      console.log(`🔍 Validating events in ${inputFile}...`);
      const content = await fs.readFile(inputFile, 'utf-8');
      const events = JSON.parse(content);
      
      if (!Array.isArray(events)) {
        throw new Error('Input file must contain an array of events');
      }
      
      console.log(`Found ${events.length} events to validate`);
      
      let validCount = 0;
      let errorCount = 0;
      
      for (const [index, event] of events.entries()) {
        try {
          normalizeAnalyticsEvent(event); // Test normalization
          validCount++;
        } catch (error) {
          console.warn(`❌ Event ${index}: ${error.message}`);
          errorCount++;
        }
      }
      
      console.log(`\n📊 Validation Results:`);
      console.log(`   ✅ Valid events: ${validCount}`);
      console.log(`   ❌ Invalid events: ${errorCount}`);
      
      if (errorCount === 0) {
        console.log(`🎉 All events are valid!`);
      } else {
        console.log(`⚠️  ${errorCount} events need attention`);
        process.exit(1);
      }
      
    } catch (error) {
      console.error('💥 Validation failed:', error.message);
      process.exit(1);
    }
    
  } else {
    console.log(`
🔄 Batch Normalization Tool

Usage:
  node normalize_batch.mjs <command> [options]

Commands:
  process <file> [type]   Process events from JSON file (type: analytics|deploy|ledger)
  validate <file>         Validate events in JSON file without processing

Examples:
  node normalize_batch.mjs process events.json analytics
  node normalize_batch.mjs process deploys.json deploy  
  node normalize_batch.mjs validate events.json

Input file format:
  JSON array of event objects
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