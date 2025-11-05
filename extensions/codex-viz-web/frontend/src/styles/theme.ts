// Tech-inspired Design System
export const theme = {
  colors: {
    // Primary - Cyber Blue
    primary: {
      50: '#e0f2fe',
      100: '#bae6fd',
      200: '#7dd3fc',
      300: '#38bdf8',
      400: '#0ea5e9',
      500: '#0284c7',
      600: '#0369a1',
      700: '#075985',
      800: '#0c4a6e',
      900: '#082f49',
    },
    // Accent - Neon Green
    accent: {
      50: '#f0fdf4',
      100: '#dcfce7',
      200: '#bbf7d0',
      300: '#86efac',
      400: '#4ade80',
      500: '#22c55e',
      600: '#16a34a',
      700: '#15803d',
      800: '#166534',
      900: '#14532d',
    },
    // Warning - Electric Yellow
    warning: {
      400: '#facc15',
      500: '#eab308',
      600: '#ca8a04',
    },
    // Error - Hot Pink
    error: {
      400: '#f472b6',
      500: '#ec4899',
      600: '#db2777',
    },
    // Background - Dark with gradient
    bg: {
      primary: '#0a0a0f',
      secondary: '#111118',
      tertiary: '#1a1a24',
      glass: 'rgba(17, 17, 24, 0.7)',
    },
    // Text
    text: {
      primary: '#ffffff',
      secondary: 'rgba(255, 255, 255, 0.7)',
      tertiary: 'rgba(255, 255, 255, 0.5)',
    },
    // Border
    border: {
      primary: 'rgba(56, 189, 248, 0.3)',
      secondary: 'rgba(255, 255, 255, 0.1)',
      glow: 'rgba(56, 189, 248, 0.6)',
    },
  },
  
  effects: {
    glassmorphism: {
      background: 'rgba(17, 17, 24, 0.7)',
      backdropFilter: 'blur(16px) saturate(180%)',
      border: '1px solid rgba(56, 189, 248, 0.2)',
      boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.37)',
    },
    neonGlow: {
      textShadow: '0 0 10px rgba(56, 189, 248, 0.8), 0 0 20px rgba(56, 189, 248, 0.5)',
      boxShadow: '0 0 15px rgba(56, 189, 248, 0.5), inset 0 0 15px rgba(56, 189, 248, 0.2)',
    },
    gridPattern: {
      backgroundImage: `
        linear-gradient(rgba(56, 189, 248, 0.05) 1px, transparent 1px),
        linear-gradient(90deg, rgba(56, 189, 248, 0.05) 1px, transparent 1px)
      `,
      backgroundSize: '50px 50px',
    },
  },

  transitions: {
    fast: '150ms cubic-bezier(0.4, 0, 0.2, 1)',
    normal: '300ms cubic-bezier(0.4, 0, 0.2, 1)',
    slow: '500ms cubic-bezier(0.4, 0, 0.2, 1)',
  },

  shadows: {
    sm: '0 2px 8px rgba(0, 0, 0, 0.3)',
    md: '0 4px 16px rgba(0, 0, 0, 0.4)',
    lg: '0 8px 32px rgba(0, 0, 0, 0.5)',
    neon: '0 0 20px rgba(56, 189, 248, 0.6)',
  },

  zIndex: {
    modal: 1000,
    overlay: 999,
    dropdown: 900,
    header: 800,
    tooltip: 700,
  },
}

export type Theme = typeof theme

