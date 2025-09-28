/**
 * Cloudflare Worker for DTG Event Logging and Sentient Cents Minting
 * Handles event logging, hashing, KV storage, and batch fetching
 */

// Sentient Cents earning rules
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

// Helper function to generate UUID v4
function generateUUID() {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

// Helper function to calculate SHA256 hash
async function calculateSHA256(text) {
  const encoder = new TextEncoder();
  const data = encoder.encode(text);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}

// Calculate Sentient Cents based on action and context
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

// Validate event data against basic schema
function validateEvent(event) {
  const required = ['action'];
  const validActions = ['view', 'click', 'scroll', 'keystroke', 'submit', 'deploy', 'mint', 'validate'];
  
  for (const field of required) {
    if (!event[field]) {
      throw new Error(`Missing required field: ${field}`);
    }
  }
  
  if (!validActions.includes(event.action)) {
    throw new Error(`Invalid action: ${event.action}`);
  }
  
  return true;
}

// Process and enrich event data
async function processEvent(eventData, request) {
  const now = new Date().toISOString();
  const event_id = generateUUID();
  
  // Extract additional context from request
  const userAgent = request.headers.get('User-Agent') || '';
  const surface = userAgent.includes('Mobile') ? 'mobile' : 'web';
  
  // Build complete event object
  const event = {
    ts_iso: now,
    event_id,
    reader_id: eventData.reader_id || 'anonymous',
    surface: eventData.surface || surface,
    action: eventData.action,
    node_id: eventData.node_id || '',
    version: eventData.version || 'v1.0.0',
    session_id: eventData.session_id || generateUUID(),
    artifact_href: eventData.artifact_href || '',
    notes: eventData.notes || '',
    sentient_cents_earned: calculateSentientCents(eventData.action, eventData.context || {})
  };
  
  // Calculate hash of the event data
  const eventJson = JSON.stringify(event, Object.keys(event).sort());
  event.hash_sha256 = await calculateSHA256(eventJson);
  
  return event;
}

// Store event in KV storage
async function storeEvent(event, env) {
  const key = `event:${event.event_id}`;
  const batchKey = `batch:${new Date().toISOString().slice(0, 10)}`; // Daily batches
  
  // Store individual event
  await env.DTG_EVENTS.put(key, JSON.stringify(event));
  
  // Add to daily batch for processing
  let batch = [];
  try {
    const existingBatch = await env.DTG_EVENTS.get(batchKey);
    if (existingBatch) {
      batch = JSON.parse(existingBatch);
    }
  } catch (e) {
    console.warn('Failed to load existing batch:', e);
  }
  
  batch.push(event.event_id);
  await env.DTG_EVENTS.put(batchKey, JSON.stringify(batch));
  
  return event.event_id;
}

// Main request handler
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const path = url.pathname;
    
    // CORS headers
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };
    
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }
    
    try {
      // Log single event
      if (path === '/log' && request.method === 'POST') {
        const eventData = await request.json();
        validateEvent(eventData);
        
        const event = await processEvent(eventData, request);
        const eventId = await storeEvent(event, env);
        
        return new Response(JSON.stringify({
          success: true,
          event_id: eventId,
          sentient_cents_earned: event.sentient_cents_earned,
          hash: event.hash_sha256
        }), {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' }
        });
      }
      
      // Batch log multiple events
      if (path === '/log/batch' && request.method === 'POST') {
        const { events } = await request.json();
        if (!Array.isArray(events)) {
          throw new Error('Events must be an array');
        }
        
        const results = [];
        for (const eventData of events) {
          try {
            validateEvent(eventData);
            const event = await processEvent(eventData, request);
            const eventId = await storeEvent(event, env);
            results.push({
              success: true,
              event_id: eventId,
              sentient_cents_earned: event.sentient_cents_earned
            });
          } catch (error) {
            results.push({
              success: false,
              error: error.message
            });
          }
        }
        
        return new Response(JSON.stringify({ results }), {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' }
        });
      }
      
      // Get events for a specific date batch
      if (path.startsWith('/batch/') && request.method === 'GET') {
        const date = path.split('/')[2];
        if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) {
          throw new Error('Invalid date format. Use YYYY-MM-DD');
        }
        
        const batchKey = `batch:${date}`;
        const batch = await env.DTG_EVENTS.get(batchKey);
        
        if (!batch) {
          return new Response(JSON.stringify({ events: [] }), {
            headers: { ...corsHeaders, 'Content-Type': 'application/json' }
          });
        }
        
        const eventIds = JSON.parse(batch);
        const events = [];
        
        for (const eventId of eventIds) {
          const eventData = await env.DTG_EVENTS.get(`event:${eventId}`);
          if (eventData) {
            events.push(JSON.parse(eventData));
          }
        }
        
        return new Response(JSON.stringify({ events }), {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' }
        });
      }
      
      // Get Sentient Cents balance for a reader
      if (path.startsWith('/balance/') && request.method === 'GET') {
        const readerId = path.split('/')[2];
        
        // This would typically aggregate from all events for the reader
        // For now, return a placeholder implementation
        return new Response(JSON.stringify({
          reader_id: readerId,
          total_balance: 0,
          last_updated: new Date().toISOString(),
          note: 'Balance calculation not yet implemented - requires aggregation job'
        }), {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' }
        });
      }
      
      // Health check endpoint
      if (path === '/health' && request.method === 'GET') {
        return new Response(JSON.stringify({
          status: 'healthy',
          timestamp: new Date().toISOString(),
          version: '1.0.0'
        }), {
          headers: { ...corsHeaders, 'Content-Type': 'application/json' }
        });
      }
      
      // Default 404 response
      return new Response('Not Found', { 
        status: 404,
        headers: corsHeaders
      });
      
    } catch (error) {
      console.error('Worker error:', error);
      return new Response(JSON.stringify({
        success: false,
        error: error.message
      }), {
        status: 400,
        headers: { ...corsHeaders, 'Content-Type': 'application/json' }
      });
    }
  }
};