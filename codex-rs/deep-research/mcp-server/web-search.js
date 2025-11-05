#!/usr/bin/env node
/**
 * Web Search MCP Server for Deep Research
 * Supports: Brave, DuckDuckGo, Google, Bing
 */

const https = require('https');
const http = require('http');
const { URL } = require('url');

const BRAVE_API_KEY = process.env.BRAVE_API_KEY;
const GOOGLE_API_KEY = process.env.GOOGLE_API_KEY;
const GOOGLE_CSE_ID = process.env.GOOGLE_CSE_ID;
const BING_API_KEY = process.env.BING_API_KEY;

class WebSearchMCPServer {
  constructor() {
    this.tools = {
      brave_search: this.braveSearch.bind(this),
      duckduckgo_search: this.duckduckgoSearch.bind(this),
      google_search: this.googleSearch.bind(this),
      bing_search: this.bingSearch.bind(this),
    };
  }

  /**
   * HTTP Request Helper
   */
  async httpRequest(url, options = {}) {
    return new Promise((resolve, reject) => {
      const urlObj = new URL(url);
      const isHttps = urlObj.protocol === 'https:';
      const client = isHttps ? https : http;

      const req = client.request(url, options, (res) => {
        let data = '';

        res.on('data', (chunk) => {
          data += chunk;
        });

        res.on('end', () => {
          try {
            resolve(JSON.parse(data));
          } catch {
            resolve(data);
          }
        });
      });

      req.on('error', reject);
      req.end();
    });
  }

  /**
   * Brave Search
   */
  async braveSearch({ query, count = 10 }) {
    if (!BRAVE_API_KEY) {
      return { error: 'BRAVE_API_KEY not set' };
    }

    const url = `https://api.search.brave.com/res/v1/web/search?q=${encodeURIComponent(query)}&count=${count}`;
    
    try {
      const data = await this.httpRequest(url, {
        headers: {
          'Accept': 'application/json',
          'X-Subscription-Token': BRAVE_API_KEY
        }
      });

      return {
        success: true,
        provider: 'brave',
        results: data.web?.results || []
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * DuckDuckGo Search (HTML parsing)
   */
  async duckduckgoSearch({ query, count = 10 }) {
    const url = `https://html.duckduckgo.com/html/?q=${encodeURIComponent(query)}`;
    
    try {
      const html = await this.httpRequest(url, {
        headers: {
          'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        }
      });

      // Simple regex parsing (production should use proper HTML parser)
      const results = [];
      const regex = /<a rel="nofollow" class="result__a" href="([^"]+)">([^<]+)<\/a>/g;
      let match;
      
      while ((match = regex.exec(html)) !== null && results.length < count) {
        results.push({
          url: match[1],
          title: match[2]
        });
      }

      return {
        success: true,
        provider: 'duckduckgo',
        results
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Google Custom Search
   */
  async googleSearch({ query, count = 10 }) {
    if (!GOOGLE_API_KEY || !GOOGLE_CSE_ID) {
      return { error: 'GOOGLE_API_KEY or GOOGLE_CSE_ID not set' };
    }

    const url = `https://www.googleapis.com/customsearch/v1?key=${GOOGLE_API_KEY}&cx=${GOOGLE_CSE_ID}&q=${encodeURIComponent(query)}&num=${count}`;
    
    try {
      const data = await this.httpRequest(url);

      return {
        success: true,
        provider: 'google',
        results: data.items || []
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Bing Search
   */
  async bingSearch({ query, count = 10 }) {
    if (!BING_API_KEY) {
      return { error: 'BING_API_KEY not set' };
    }

    const url = `https://api.bing.microsoft.com/v7.0/search?q=${encodeURIComponent(query)}&count=${count}`;
    
    try {
      const data = await this.httpRequest(url, {
        headers: {
          'Ocp-Apim-Subscription-Key': BING_API_KEY
        }
      });

      return {
        success: true,
        provider: 'bing',
        results: data.webPages?.value || []
      };
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Handle MCP Request
   */
  async handleRequest(request) {
    const { tool, arguments: args } = request;

    if (!this.tools[tool]) {
      return {
        error: `Unknown tool: ${tool}`,
        availableTools: Object.keys(this.tools)
      };
    }

    try {
      const result = await this.tools[tool](args);
      return result;
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }

  /**
   * Start Server
   */
  start() {
    console.log('[Web Search MCP] Starting...');
    
    const apiKeys = {
      brave: !!BRAVE_API_KEY,
      google: !!(GOOGLE_API_KEY && GOOGLE_CSE_ID),
      bing: !!BING_API_KEY
    };
    
    console.log('[Web Search MCP] API Keys configured:', apiKeys);

    process.stdin.setEncoding('utf-8');
    
    let buffer = '';
    
    process.stdin.on('data', async (chunk) => {
      buffer += chunk;
      
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';
      
      for (const line of lines) {
        if (!line.trim()) continue;
        
        try {
          const request = JSON.parse(line);
          const response = await this.handleRequest(request);
          process.stdout.write(JSON.stringify(response) + '\n');
        } catch (error) {
          process.stdout.write(JSON.stringify({
            error: 'Invalid JSON request',
            details: error.message
          }) + '\n');
        }
      }
    });

    console.log('[Web Search MCP] Ready');
  }
}

if (require.main === module) {
  const server = new WebSearchMCPServer();
  server.start();
}

module.exports = WebSearchMCPServer;

