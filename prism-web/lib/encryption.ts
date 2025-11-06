// API Key Encryption/Decryption
// Note: This runs on Edge Functions (server-side only)
import CryptoJS from 'crypto-js'

const ENCRYPTION_SECRET = process.env.ENCRYPTION_SECRET

if (!ENCRYPTION_SECRET) {
  console.warn('ENCRYPTION_SECRET not set. API key encryption disabled.')
}

export function encryptApiKey(apiKey: string): string {
  if (!ENCRYPTION_SECRET) {
    throw new Error('Encryption secret not configured')
  }
  
  return CryptoJS.AES.encrypt(apiKey, ENCRYPTION_SECRET).toString()
}

export function decryptApiKey(encryptedKey: string): string {
  if (!ENCRYPTION_SECRET) {
    throw new Error('Encryption secret not configured')
  }
  
  const bytes = CryptoJS.AES.decrypt(encryptedKey, ENCRYPTION_SECRET)
  const decrypted = bytes.toString(CryptoJS.enc.Utf8)
  
  if (!decrypted) {
    throw new Error('Failed to decrypt API key')
  }
  
  return decrypted
}

// Validate API key format
export function validateApiKey(provider: 'openai' | 'anthropic', apiKey: string): boolean {
  switch (provider) {
    case 'openai':
      return /^sk-[a-zA-Z0-9]{48}$/.test(apiKey)
    case 'anthropic':
      return /^sk-ant-[a-zA-Z0-9-]{95}$/.test(apiKey)
    default:
      return false
  }
}

// Mask API key for display (show first 8 and last 4 characters)
export function maskApiKey(apiKey: string): string {
  if (apiKey.length < 12) return '***'
  return `${apiKey.slice(0, 8)}...${apiKey.slice(-4)}`
}

