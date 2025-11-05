// WebXRProvider.tsx - WebXR基盤実装
// Quest, PSVR2, Vive等のVRヘッドセット対応

import { ReactNode } from 'react'
import { createXRStore, XR } from '@react-three/xr'

// XR Store作成（グローバル）
export const xrStore = createXRStore({
  // VR Controllers
  controller: true,
  
  // Hand Tracking（Quest Pro, Quest 3）
  hand: true,
  
  // AR Anchors（空間アンカー）
  anchors: true,
  
  // Composition Layers（高品質UI）
  layers: true,
  
  // Foveated Rendering（PSVR2 Eye Tracking）
  foveation: 'dynamic',
  
  // Frame Rate（Quest 2: 72/90/120Hz）
  frameRate: 90,
})

interface WebXRProviderProps {
  children: ReactNode
}

export const WebXRProvider = ({ children }: WebXRProviderProps) => {
  return (
    <XR store={xrStore}>
      {children}
    </XR>
  )
}

// VR Session管理フック
export const useVRSession = () => {
  const isVR = xrStore.useState((state) => state.session?.mode === 'immersive-vr')
  const isAR = xrStore.useState((state) => state.session?.mode === 'immersive-ar')
  
  const enterVR = async () => {
    try {
      await xrStore.enterVR()
      console.log('✓ Entered VR mode')
    } catch (error) {
      console.error('Failed to enter VR:', error)
      throw error
    }
  }
  
  const enterAR = async () => {
    try {
      await xrStore.enterAR()
      console.log('✓ Entered AR mode')
    } catch (error) {
      console.error('Failed to enter AR:', error)
      throw error
    }
  }
  
  const exitXR = () => {
    xrStore.getState().session?.end()
    console.log('✓ Exited XR mode')
  }
  
  return {
    isVR,
    isAR,
    isXR: isVR || isAR,
    enterVR,
    enterAR,
    exitXR,
  }
}

// デバイス検出
export const detectXRCapabilities = async () => {
  if (!('xr' in navigator)) {
    return {
      vrSupported: false,
      arSupported: false,
      deviceName: 'Unknown',
    }
  }
  
  const vrSupported = await navigator.xr?.isSessionSupported('immersive-vr') ?? false
  const arSupported = await navigator.xr?.isSessionSupported('immersive-ar') ?? false
  
  // デバイス名推定（UserAgent解析）
  const ua = navigator.userAgent
  let deviceName = 'Unknown'
  
  if (ua.includes('Quest')) {
    deviceName = 'Meta Quest'
  } else if (ua.includes('PSVR')) {
    deviceName = 'PlayStation VR2'
  } else if (ua.includes('Vive')) {
    deviceName = 'HTC Vive'
  } else if (ua.includes('Oculus')) {
    deviceName = 'Oculus'
  }
  
  return {
    vrSupported,
    arSupported,
    deviceName,
  }
}

