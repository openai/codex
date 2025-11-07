'use client'

import { useState, useEffect } from 'react'
import { createSupabaseClient } from '@/lib/supabase'
import { validateApiKey, maskApiKey } from '@/lib/encryption'

interface ApiKeyData {
  id: string
  provider: 'openai' | 'anthropic'
  keyName: string
  isActive: boolean
  lastUsedAt: string | null
  createdAt: string
}

export default function ApiKeysPage() {
  const supabase = createSupabaseClient()
  
  const [keys, setKeys] = useState<ApiKeyData[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  
  // Form state
  const [provider, setProvider] = useState<'openai' | 'anthropic'>('openai')
  const [apiKey, setApiKey] = useState('')
  const [keyName, setKeyName] = useState('')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  useEffect(() => {
    loadApiKeys()
  }, [])

  async function loadApiKeys() {
    try {
      const { data, error } = await supabase
        .from('user_api_keys')
        .select('id, provider, key_name, is_active, last_used_at, created_at')
        .order('created_at', { ascending: false })

      if (error) throw error
      setKeys(data || [])
    } catch (err: any) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  async function handleSaveKey(e: React.FormEvent) {
    e.preventDefault()
    setSaving(true)
    setError('')
    setSuccess('')

    try {
      // Validate API key format
      if (!validateApiKey(provider, apiKey)) {
        throw new Error(`Invalid ${provider} API key format`)
      }

      // Call Edge Function to encrypt and save
      const { data, error } = await supabase.functions.invoke('save-api-key', {
        body: {
          provider,
          apiKey,
          keyName: keyName || `${provider} API Key`
        }
      })

      if (error) throw error

      setSuccess('API key saved successfully!')
      setApiKey('')
      setKeyName('')
      
      // Reload keys
      await loadApiKeys()
    } catch (err: any) {
      setError(err.message || 'Failed to save API key')
    } finally {
      setSaving(false)
    }
  }

  async function handleDeleteKey(keyId: string) {
    if (!confirm('Are you sure you want to delete this API key?')) return

    try {
      const { error } = await supabase
        .from('user_api_keys')
        .delete()
        .eq('id', keyId)

      if (error) throw error

      setSuccess('API key deleted')
      await loadApiKeys()
    } catch (err: any) {
      setError(err.message)
    }
  }

  async function handleToggleKey(keyId: string, currentStatus: boolean) {
    try {
      const { error } = await supabase
        .from('user_api_keys')
        .update({ is_active: !currentStatus })
        .eq('id', keyId)

      if (error) throw error

      await loadApiKeys()
    } catch (err: any) {
      setError(err.message)
    }
  }

  return (
    <div className="max-w-4xl mx-auto p-8">
      <h1 className="text-3xl font-bold text-white mb-2">API Keys</h1>
      <p className="text-gray-400 mb-8">
        Add your OpenAI and Anthropic API keys. They are encrypted and stored securely.
        Your keys are only used for your own requests.
      </p>

      {error && (
        <div className="mb-6 p-4 bg-red-500/10 border border-red-500/50 rounded-lg text-red-400 text-sm">
          {error}
        </div>
      )}

      {success && (
        <div className="mb-6 p-4 bg-green-500/10 border border-green-500/50 rounded-lg text-green-400 text-sm">
          {success}
        </div>
      )}

      {/* Add New Key Form */}
      <div className="mb-8 p-6 bg-gray-800/50 rounded-xl border border-gray-700">
        <h2 className="text-xl font-semibold text-white mb-4">Add New API Key</h2>
        
        <form onSubmit={handleSaveKey} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Provider
            </label>
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as 'openai' | 'anthropic')}
              className="w-full px-4 py-3 bg-gray-700/50 border border-gray-600 rounded-lg text-white focus:outline-none focus:ring-2 focus:ring-purple-500"
            >
              <option value="openai">OpenAI (GPT-4, GPT-3.5)</option>
              <option value="anthropic">Anthropic (Claude 3)</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Key Name (optional)
            </label>
            <input
              type="text"
              value={keyName}
              onChange={(e) => setKeyName(e.target.value)}
              className="w-full px-4 py-3 bg-gray-700/50 border border-gray-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              placeholder="My API Key"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              API Key
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              required
              className="w-full px-4 py-3 bg-gray-700/50 border border-gray-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-purple-500 font-mono"
              placeholder={provider === 'openai' ? 'sk-...' : 'sk-ant-...'}
            />
            <p className="mt-1 text-xs text-gray-400">
              {provider === 'openai' 
                ? 'Get your API key from https://platform.openai.com/api-keys'
                : 'Get your API key from https://console.anthropic.com/'}
            </p>
          </div>

          <button
            type="submit"
            disabled={saving}
            className="w-full py-3 px-4 bg-gradient-to-r from-purple-500 to-pink-500 text-white font-semibold rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50"
          >
            {saving ? 'Saving...' : 'Save API Key'}
          </button>
        </form>
      </div>

      {/* Existing Keys */}
      <div>
        <h2 className="text-xl font-semibold text-white mb-4">Your API Keys</h2>
        
        {loading ? (
          <p className="text-gray-400">Loading...</p>
        ) : keys.length === 0 ? (
          <div className="p-8 text-center bg-gray-800/30 rounded-xl border border-gray-700 border-dashed">
            <p className="text-gray-400">No API keys yet. Add one above to get started.</p>
          </div>
        ) : (
          <div className="space-y-3">
            {keys.map((key) => (
              <div
                key={key.id}
                className="p-4 bg-gray-800/50 rounded-lg border border-gray-700 flex items-center justify-between"
              >
                <div className="flex-1">
                  <div className="flex items-center gap-3">
                    <div className={`w-2 h-2 rounded-full ${key.isActive ? 'bg-green-400' : 'bg-gray-500'}`} />
                    <div>
                      <h3 className="font-semibold text-white">{key.keyName}</h3>
                      <p className="text-sm text-gray-400">
                        {key.provider === 'openai' ? 'OpenAI' : 'Anthropic'} 窶｢{' '}
                        {key.lastUsedAt 
                          ? `Last used ${new Date(key.lastUsedAt).toLocaleDateString()}`
                          : 'Never used'}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    onClick={() => handleToggleKey(key.id, key.isActive)}
                    className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 text-sm text-white rounded transition"
                  >
                    {key.isActive ? 'Disable' : 'Enable'}
                  </button>
                  <button
                    onClick={() => handleDeleteKey(key.id)}
                    className="px-3 py-1.5 bg-red-500/20 hover:bg-red-500/30 text-sm text-red-400 rounded transition"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Info Box */}
      <div className="mt-8 p-6 bg-blue-500/10 border border-blue-500/30 rounded-xl">
        <h3 className="font-semibold text-blue-400 mb-2">柏 Security & Privacy</h3>
        <ul className="text-sm text-gray-300 space-y-1">
          <li>笨・Keys are encrypted with AES-256 before storage</li>
          <li>笨・Only you can decrypt and use your keys</li>
          <li>笨・Keys never leave your account</li>
          <li>笨・You control your own AI usage and costs</li>
        </ul>
      </div>
    </div>
  )
}

