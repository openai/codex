'use client';

import React, { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { Snackbar, Alert, Button, Dialog, DialogTitle, DialogContent, DialogActions } from '@mui/material';

interface PWAContextType {
  isInstallable: boolean;
  isInstalled: boolean;
  isOffline: boolean;
  updateAvailable: boolean;
  installPrompt: () => void;
  updateApp: () => void;
}

const PWAContext = createContext<PWAContextType | undefined>(undefined);

export function PWAProvider({ children }: { children: ReactNode }) {
  const [isInstallable, setIsInstallable] = useState(false);
  const [isInstalled, setIsInstalled] = useState(false);
  const [isOffline, setIsOffline] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [deferredPrompt, setDeferredPrompt] = useState<any>(null);
  const [showInstallPrompt, setShowInstallPrompt] = useState(false);
  const [showUpdateDialog, setShowUpdateDialog] = useState(false);

  // Check if app is installed
  useEffect(() => {
    const checkInstalled = () => {
      if (window.matchMedia && window.matchMedia('(display-mode: standalone)').matches) {
        setIsInstalled(true);
      }

      // Check for iOS Safari
      if ((window.navigator as any).standalone === true) {
        setIsInstalled(true);
      }
    };

    checkInstalled();
  }, []);

  // Register Service Worker
  useEffect(() => {
    if ('serviceWorker' in navigator) {
      window.addEventListener('load', () => {
        navigator.serviceWorker
          .register('/sw.js')
          .then((registration) => {
            console.log('SW registered: ', registration);

            // Check for updates
            registration.addEventListener('updatefound', () => {
              const newWorker = registration.installing;
              if (newWorker) {
                newWorker.addEventListener('statechange', () => {
                  if (newWorker.state === 'installed' && navigator.serviceWorker.controller) {
                    setUpdateAvailable(true);
                    setShowUpdateDialog(true);
                  }
                });
              }
            });

            // Listen for messages from SW
            navigator.serviceWorker.addEventListener('message', (event) => {
              if (event.data && event.data.type === 'UPDATE_AVAILABLE') {
                setUpdateAvailable(true);
                setShowUpdateDialog(true);
              }
            });
          })
          .catch((registrationError) => {
            console.log('SW registration failed: ', registrationError);
          });
      });
    }
  }, []);

  // Handle install prompt
  useEffect(() => {
    const handleBeforeInstallPrompt = (e: Event) => {
      e.preventDefault();
      setDeferredPrompt(e);
      setIsInstallable(true);
      setShowInstallPrompt(true);
    };

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);

    return () => {
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    };
  }, []);

  // Handle online/offline status
  useEffect(() => {
    const handleOnline = () => setIsOffline(false);
    const handleOffline = () => setIsOffline(true);

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    // Initial check
    setIsOffline(!navigator.onLine);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  const installPrompt = async () => {
    if (!deferredPrompt) return;

    deferredPrompt.prompt();
    const { outcome } = await deferredPrompt.userChoice;

    if (outcome === 'accepted') {
      console.log('User accepted the install prompt');
      setIsInstalled(true);
    }

    setDeferredPrompt(null);
    setShowInstallPrompt(false);
  };

  const updateApp = () => {
    if ('serviceWorker' in navigator && navigator.serviceWorker.controller) {
      navigator.serviceWorker.controller.postMessage({ type: 'SKIP_WAITING' });
      window.location.reload();
    }
  };

  const value: PWAContextType = {
    isInstallable,
    isInstalled,
    isOffline,
    updateAvailable,
    installPrompt,
    updateApp,
  };

  return (
    <PWAContext.Provider value={value}>
      {children}

      {/* Install Prompt Snackbar */}
      <Snackbar
        open={showInstallPrompt && !isInstalled}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
        sx={{ mb: 2 }}
      >
        <Alert
          severity="info"
          action={
            <>
              <Button color="inherit" size="small" onClick={installPrompt}>
                インストール
              </Button>
              <Button color="inherit" size="small" onClick={() => setShowInstallPrompt(false)}>
                後で
              </Button>
            </>
          }
        >
          Codex GUIをホーム画面に追加できます
        </Alert>
      </Snackbar>

      {/* Offline Status */}
      <Snackbar
        open={isOffline}
        anchorOrigin={{ vertical: 'top', horizontal: 'center' }}
      >
        <Alert severity="warning">
          オフラインモード - 一部の機能が制限されます
        </Alert>
      </Snackbar>

      {/* Update Dialog */}
      <Dialog open={showUpdateDialog} onClose={() => setShowUpdateDialog(false)}>
        <DialogTitle>アップデートが利用可能です</DialogTitle>
        <DialogContent>
          <p>新しいバージョンのCodex GUIが利用可能です。今すぐアップデートしますか？</p>
          <p style={{ fontSize: '0.9em', color: '#666', marginTop: '8px' }}>
            アップデートにより最新の機能と改善が利用可能になります。
          </p>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setShowUpdateDialog(false)}>後で</Button>
          <Button onClick={updateApp} variant="contained">
            今すぐアップデート
          </Button>
        </DialogActions>
      </Dialog>
    </PWAContext.Provider>
  );
}

export function usePWA() {
  const context = useContext(PWAContext);
  if (context === undefined) {
    throw new Error('usePWA must be used within a PWAProvider');
  }
  return context;
}
