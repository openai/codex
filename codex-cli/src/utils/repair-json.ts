/**
 * Attempts to repair common JSON formatting issues, particularly those
 * produced by Ollama when generating function calls.
 */
export function repairJson(input: string): string | null {
  let json = input.trim();
  
  // Common repairs:
  
  // 1. Fix escaped parentheses that should be double-escaped
  // Ollama often outputs \( instead of \\(
  json = json.replace(/\\([()[\]{}])/g, '\\\\$1');
  
  // 2. Fix trailing commas before closing braces/brackets
  json = json.replace(/,(\s*[}\]])/g, '$1');
  
  // 3. Add missing quotes around property names
  json = json.replace(/(\{|,)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*:/g, '$1"$2":');
  
  // 4. Fix single quotes (should be double quotes in JSON)
  // But be careful not to replace single quotes inside double-quoted strings
  let inString = false;
  let escaped = false;
  let result = '';
  
  for (let i = 0; i < json.length; i++) {
    const char = json[i];
    const prevChar = i > 0 ? json[i - 1] : '';
    
    if (escaped) {
      escaped = false;
      result += char;
      continue;
    }
    
    if (char === '\\') {
      escaped = true;
      result += char;
      continue;
    }
    
    if (char === '"' && prevChar !== '\\') {
      inString = !inString;
      result += char;
      continue;
    }
    
    if (char === "'" && !inString) {
      result += '"';
    } else {
      result += char;
    }
  }
  
  json = result;
  
  // 5. Try to fix incomplete JSON by adding missing closing braces/brackets
  const openBraces = (json.match(/{/g) || []).length;
  const closeBraces = (json.match(/}/g) || []).length;
  const openBrackets = (json.match(/\[/g) || []).length;
  const closeBrackets = (json.match(/]/g) || []).length;
  
  // Add missing closing brackets
  for (let i = 0; i < openBrackets - closeBrackets; i++) {
    json += ']';
  }
  
  // Add missing closing braces
  for (let i = 0; i < openBraces - closeBraces; i++) {
    json += '}';
  }
  
  // 6. Try to validate and return
  try {
    JSON.parse(json);
    return json;
  } catch (e) {
    // If it still fails, try one more thing: remove any text after the last valid closing brace
    const lastBrace = json.lastIndexOf('}');
    if (lastBrace !== -1) {
      const truncated = json.substring(0, lastBrace + 1);
      try {
        JSON.parse(truncated);
        return truncated;
      } catch {
        // Give up
      }
    }
    
    return null;
  }
}

/**
 * Specifically repairs Ollama function call JSON
 */
export function repairOllamaFunctionCall(input: string): { name: string; arguments: any } | null {
  const repaired = repairJson(input);
  if (!repaired) return null;
  
  try {
    const parsed = JSON.parse(repaired);
    
    // Ensure it has the expected structure
    if (parsed.name && (parsed.arguments || parsed.parameters)) {
      return {
        name: parsed.name,
        arguments: parsed.arguments || parsed.parameters
      };
    }
    
    return null;
  } catch {
    return null;
  }
}