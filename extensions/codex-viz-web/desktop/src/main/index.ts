import { app, BrowserWindow, Tray, Menu, ipcMain, shell, nativeImage } from 'electron'
import { join } from 'path'
import { spawn, ChildProcess } from 'child_process'
import Store from 'electron-store'
import { autoUpdater } from 'electron-updater'

// Persistent store
const store = new Store({
  defaults: {
    windowBounds: { width: 1400, height: 900 },
    recentRepos: [],
    settings: {
      autoStart: true,
      minimizeToTray: true,
      notifications: true,
      theme: 'dark',
    },
  },
})

let mainWindow: BrowserWindow | null = null
let tray: Tray | null = null
let backendProcess: ChildProcess | null = null

// Backend server management
function startBackendServer() {
  const backendPath = join(__dirname, '../../backend/target/release/codex-viz-backend')
  
  backendProcess = spawn(backendPath, [], {
    stdio: 'inherit',
    shell: process.platform === 'win32',
  })

  backendProcess.on('error', (error) => {
    console.error('Backend server error:', error)
  })

  backendProcess.on('exit', (code) => {
    console.log(`Backend server exited with code ${code}`)
  })

  console.log('🦀 Backend server started')
}

function stopBackendServer() {
  if (backendProcess) {
    backendProcess.kill()
    backendProcess = null
    console.log('🛑 Backend server stopped')
  }
}

// Create main window
function createWindow() {
  const bounds = store.get('windowBounds') as { width: number; height: number }

  mainWindow = new BrowserWindow({
    width: bounds.width,
    height: bounds.height,
    minWidth: 1024,
    minHeight: 768,
    backgroundColor: '#0a0a0f',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      nodeIntegration: false,
      contextIsolation: true,
    },
    title: 'Codex Repository Visualizer',
    show: false, // Show after ready-to-show
  })

  // Load frontend
  if (process.env.VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL)
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }

  // Show window when ready
  mainWindow.once('ready-to-show', () => {
    mainWindow?.show()
  })

  // Save window bounds on resize
  mainWindow.on('resize', () => {
    const bounds = mainWindow?.getBounds()
    if (bounds) {
      store.set('windowBounds', { width: bounds.width, height: bounds.height })
    }
  })

  // Handle minimize to tray
  mainWindow.on('close', (event) => {
    const settings = store.get('settings') as { minimizeToTray: boolean }
    
    if (settings.minimizeToTray && !app.isQuitting) {
      event.preventDefault()
      mainWindow?.hide()
    }
  })

  // Open external links in browser
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url)
    return { action: 'deny' }
  })
}

// Create system tray
function createTray() {
  const iconPath = join(__dirname, '../../resources/tray-icon.png')
  const trayIcon = nativeImage.createFromPath(iconPath)
  
  tray = new Tray(trayIcon.resize({ width: 16, height: 16 }))

  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Show Codex Viz',
      click: () => {
        mainWindow?.show()
        mainWindow?.focus()
      },
    },
    { type: 'separator' },
    {
      label: 'Recent Repositories',
      submenu: getRecentReposMenu(),
    },
    { type: 'separator' },
    {
      label: 'Settings',
      submenu: [
        {
          label: 'Auto-start on login',
          type: 'checkbox',
          checked: (store.get('settings') as { autoStart: boolean }).autoStart,
          click: (item) => {
            store.set('settings.autoStart', item.checked)
            app.setLoginItemSettings({
              openAtLogin: item.checked,
            })
          },
        },
        {
          label: 'Minimize to tray',
          type: 'checkbox',
          checked: (store.get('settings') as { minimizeToTray: boolean }).minimizeToTray,
          click: (item) => {
            store.set('settings.minimizeToTray', item.checked)
          },
        },
        {
          label: 'Enable notifications',
          type: 'checkbox',
          checked: (store.get('settings') as { notifications: boolean }).notifications,
          click: (item) => {
            store.set('settings.notifications', item.checked)
          },
        },
      ],
    },
    { type: 'separator' },
    {
      label: 'Quit',
      click: () => {
        app.isQuitting = true
        app.quit()
      },
    },
  ])

  tray.setToolTip('Codex Repository Visualizer')
  tray.setContextMenu(contextMenu)

  tray.on('double-click', () => {
    mainWindow?.show()
    mainWindow?.focus()
  })
}

function getRecentReposMenu(): Electron.MenuItemConstructorOptions[] {
  const recentRepos = store.get('recentRepos') as string[]

  if (recentRepos.length === 0) {
    return [{ label: 'No recent repositories', enabled: false }]
  }

  return recentRepos.slice(0, 5).map((repo) => ({
    label: repo,
    click: () => {
      mainWindow?.webContents.send('open-repo', repo)
      mainWindow?.show()
      mainWindow?.focus()
    },
  }))
}

// IPC Handlers
ipcMain.handle('get-store', (_, key: string) => {
  return store.get(key)
})

ipcMain.handle('set-store', (_, key: string, value: any) => {
  store.set(key, value)
})

ipcMain.handle('add-recent-repo', (_, repo: string) => {
  const recentRepos = store.get('recentRepos') as string[]
  const updated = [repo, ...recentRepos.filter((r) => r !== repo)].slice(0, 10)
  store.set('recentRepos', updated)
  
  // Update tray menu
  if (tray) {
    const contextMenu = tray.getContextMenu()
    if (contextMenu) {
      // Rebuild menu with updated recent repos
      createTray()
    }
  }
})

ipcMain.handle('show-notification', (_, options: { title: string; body: string }) => {
  const settings = store.get('settings') as { notifications: boolean }
  
  if (settings.notifications && mainWindow) {
    new Notification({
      title: options.title,
      body: options.body,
    }).show()
  }
})

ipcMain.handle('minimize-to-tray', () => {
  mainWindow?.hide()
})

// Auto-updater
autoUpdater.on('update-available', () => {
  mainWindow?.webContents.send('update-available')
})

autoUpdater.on('update-downloaded', () => {
  mainWindow?.webContents.send('update-downloaded')
})

ipcMain.handle('install-update', () => {
  autoUpdater.quitAndInstall()
})

// App lifecycle
app.whenReady().then(() => {
  // Start backend server
  startBackendServer()

  // Create window and tray
  createWindow()
  createTray()

  // Check for updates
  autoUpdater.checkForUpdatesAndNotify()

  // Auto-start settings
  const settings = store.get('settings') as { autoStart: boolean }
  app.setLoginItemSettings({
    openAtLogin: settings.autoStart,
  })
})

app.on('window-all-closed', () => {
  const settings = store.get('settings') as { minimizeToTray: boolean }
  
  if (!settings.minimizeToTray || process.platform !== 'darwin') {
    stopBackendServer()
    app.quit()
  }
})

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow()
  } else {
    mainWindow?.show()
  }
})

app.on('before-quit', () => {
  app.isQuitting = true
  stopBackendServer()
})

// Extend app with custom property
declare module 'electron' {
  interface App {
    isQuitting?: boolean
  }
}

