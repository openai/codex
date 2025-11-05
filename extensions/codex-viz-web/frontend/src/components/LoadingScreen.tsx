import { Html } from '@react-three/drei'

export default function LoadingScreen() {
  return (
    <Html center>
      <div style={{
        padding: '20px',
        background: 'rgba(0, 0, 0, 0.8)',
        borderRadius: '8px',
        color: 'white',
        fontSize: '18px',
      }}>
        Loading repository data...
      </div>
    </Html>
  )
}

