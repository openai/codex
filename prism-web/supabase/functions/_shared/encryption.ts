// Shared encryption utilities for Edge Functions
// Uses Deno's built-in Web Crypto API

const ENCRYPTION_SECRET = Deno.env.get('ENCRYPTION_SECRET')

if (!ENCRYPTION_SECRET) {
  throw new Error('ENCRYPTION_SECRET environment variable is required')
}

// Convert string to ArrayBuffer
function stringToArrayBuffer(str: string): Uint8Array {
  return new TextEncoder().encode(str)
}

// Convert ArrayBuffer to string
function arrayBufferToString(buffer: ArrayBuffer): string {
  return new TextDecoder().decode(buffer)
}

// Derive encryption key from secret
async function deriveKey(): Promise<CryptoKey> {
  const keyMaterial = await crypto.subtle.importKey(
    'raw',
    stringToArrayBuffer(ENCRYPTION_SECRET!),
    { name: 'PBKDF2' },
    false,
    ['deriveBits', 'deriveKey']
  )

  return await crypto.subtle.deriveKey(
    {
      name: 'PBKDF2',
      salt: stringToArrayBuffer('prism-salt'), // In production, use per-key random salt
      iterations: 100000,
      hash: 'SHA-256',
    },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    true,
    ['encrypt', 'decrypt']
  )
}

export async function encryptApiKey(apiKey: string): Promise<string> {
  const key = await deriveKey()
  const iv = crypto.getRandomValues(new Uint8Array(12))
  
  const encrypted = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    key,
    stringToArrayBuffer(apiKey)
  )

  // Combine IV + encrypted data
  const combined = new Uint8Array(iv.length + encrypted.byteLength)
  combined.set(iv, 0)
  combined.set(new Uint8Array(encrypted), iv.length)

  // Convert to base64
  return btoa(String.fromCharCode(...combined))
}

export async function decryptApiKey(encryptedKey: string): Promise<string> {
  const key = await deriveKey()
  
  // Decode from base64
  const combined = Uint8Array.from(atob(encryptedKey), c => c.charCodeAt(0))
  
  // Extract IV and encrypted data
  const iv = combined.slice(0, 12)
  const encrypted = combined.slice(12)

  const decrypted = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv },
    key,
    encrypted
  )

  return arrayBufferToString(decrypted)
}

// Simpler version using crypto-js (alternative, requires npm package in Deno)
// For now, use Web Crypto API which is native to Deno

