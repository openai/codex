import { contextBridge, ipcRenderer } from 'electron'

// Expose protected methods to renderer process
contextBridge.exposeInMainWorld('electronAPI', {
  // Store operations
  getStore: (key: string) => ipcRenderer.invoke('get-store', key),
  setStore: (key: string, value: any) => ipcRenderer.invoke('set-store', key, value),
  
  // Repository operations
  addRecentRepo: (repo: string) => ipcRenderer.invoke('add-recent-repo', repo),
  
  // Notifications
  showNotification: (options: { title: string; body: string }) =>
    ipcRenderer.invoke('show-notification', options),
  
  // Window controls
  minimizeToTray: () => ipcRenderer.invoke('minimize-to-tray'),
  
  // Auto-updater
  installUpdate: () => ipcRenderer.invoke('install-update'),
  
  // Event listeners
  onOpenRepo: (callback: (repo: string) => void) => {
    ipcRenderer.on('open-repo', (_, repo) => callback(repo))
  },
  
  onUpdateAvailable: (callback: () => void) => {
    ipcRenderer.on('update-available', callback)
  },
  
  onUpdateDownloaded: (callback: () => void) => {
    ipcRenderer.on('update-downloaded', callback)
  },
})

// Type declarations
export interface ElectronAPI {
  getStore: (key: string) => Promise<any>
  setStore: (key: string, value: any) => Promise<void>
  addRecentRepo: (repo: string) => Promise<void>
  showNotification: (options: { title: string; body: string }) => Promise<void>
  minimizeToTray: () => Promise<void>
  installUpdate: () => Promise<void>
  onOpenRepo: (callback: (repo: string) => void) => void
  onUpdateAvailable: (callback: () => void) => void
  onUpdateDownloaded: (callback: () => void) => void
}

declare global {
  interface Window {
    electronAPI: ElectronAPI
  }
}

