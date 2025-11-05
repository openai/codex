import { createTheme, ThemeOptions } from '@mui/material/styles';
import { Inter, Roboto_Mono } from 'next/font/google';

// Google Fonts
const inter = Inter({
  subsets: ['latin'],
  display: 'swap',
  variable: '--font-inter',
});

const robotoMono = Roboto_Mono({
  subsets: ['latin'],
  display: 'swap',
  variable: '--font-roboto-mono',
});

// Material Design 3 Color Tokens
const lightPalette = {
  primary: {
    main: '#0061a4',
    light: '#4d7bb3',
    dark: '#003f77',
    contrastText: '#ffffff',
  },
  secondary: {
    main: '#565f71',
    light: '#7e899c',
    dark: '#2f3749',
    contrastText: '#ffffff',
  },
  error: {
    main: '#ba1a1a',
    light: '#d23838',
    dark: '#8c0009',
    contrastText: '#ffffff',
  },
  warning: {
    main: '#7d5800',
    light: '#a67c00',
    dark: '#4c3d00',
    contrastText: '#ffffff',
  },
  info: {
    main: '#0061a6',
    light: '#4d7bb4',
    dark: '#003f78',
    contrastText: '#ffffff',
  },
  success: {
    main: '#146c2e',
    light: '#3b8055',
    dark: '#004e1f',
    contrastText: '#ffffff',
  },
  background: {
    default: '#fdfbff',
    paper: '#fdfbff',
  },
  surface: {
    main: '#fdfbff',
    variant: '#e7e0ec',
  },
  outline: {
    main: '#79747e',
    variant: '#c9c5ca',
  },
  text: {
    primary: '#1d1b20',
    secondary: '#49454f',
    disabled: '#938f99',
  },
};

const darkPalette = {
  primary: {
    main: '#9ecaed',
    light: '#c8e6ff',
    dark: '#7b95bb',
    contrastText: '#0f1419',
  },
  secondary: {
    main: '#bfc6dc',
    light: '#d1d9e6',
    dark: '#9ca3b3',
    contrastText: '#1d1b20',
  },
  error: {
    main: '#ffb4ab',
    light: '#ffdad6',
    dark: '#93000a',
    contrastText: '#410e0b',
  },
  warning: {
    main: '#ffb945',
    light: '#ffe088',
    dark: '#9c4200',
    contrastText: '#4e2500',
  },
  info: {
    main: '#9ecaed',
    light: '#c8e6ff',
    dark: '#7b95bb',
    contrastText: '#0f1419',
  },
  success: {
    main: '#6fdd8b',
    light: '#92f7a2',
    dark: '#4ae070',
    contrastText: '#003912',
  },
  background: {
    default: '#0f1419',
    paper: '#0f1419',
  },
  surface: {
    main: '#0f1419',
    variant: '#1d1b20',
  },
  outline: {
    main: '#938f99',
    variant: '#49454f',
  },
  text: {
    primary: '#e6e1e5',
    secondary: '#c9c5ca',
    disabled: '#938f99',
  },
};

// Common theme options
const getCommonThemeOptions = (isDark: boolean): ThemeOptions => ({
  typography: {
    fontFamily: inter.style.fontFamily,
    h1: {
      fontSize: '2.25rem',
      fontWeight: 400,
      lineHeight: 1.2,
      letterSpacing: '-0.025em',
    },
    h2: {
      fontSize: '1.875rem',
      fontWeight: 400,
      lineHeight: 1.3,
      letterSpacing: '-0.025em',
    },
    h3: {
      fontSize: '1.5rem',
      fontWeight: 400,
      lineHeight: 1.4,
      letterSpacing: '-0.025em',
    },
    h4: {
      fontSize: '1.25rem',
      fontWeight: 400,
      lineHeight: 1.4,
      letterSpacing: '-0.025em',
    },
    h5: {
      fontSize: '1.125rem',
      fontWeight: 400,
      lineHeight: 1.4,
      letterSpacing: '-0.025em',
    },
    h6: {
      fontSize: '1rem',
      fontWeight: 500,
      lineHeight: 1.4,
      letterSpacing: '-0.025em',
    },
    body1: {
      fontSize: '1rem',
      fontWeight: 400,
      lineHeight: 1.5,
    },
    body2: {
      fontSize: '0.875rem',
      fontWeight: 400,
      lineHeight: 1.5,
    },
    button: {
      fontSize: '0.875rem',
      fontWeight: 500,
      lineHeight: 1.5,
      textTransform: 'none' as const,
    },
    caption: {
      fontSize: '0.75rem',
      fontWeight: 400,
      lineHeight: 1.4,
    },
    overline: {
      fontSize: '0.75rem',
      fontWeight: 500,
      lineHeight: 1.4,
      textTransform: 'uppercase' as const,
      letterSpacing: '0.08em',
    },
  },
  shape: {
    borderRadius: 12, // Material Design 3 の境界半径
  },
  components: {
    MuiButton: {
      styleOverrides: {
        root: {
          borderRadius: 100, // 完全な円形の角丸
          padding: '8px 24px',
          fontWeight: 500,
          boxShadow: 'none',
          '&:hover': {
            boxShadow: '0 1px 3px 0 rgba(0, 0, 0, 0.12), 0 1px 1px 0 rgba(0, 0, 0, 0.08)',
          },
        },
        contained: {
          boxShadow: '0 1px 3px 0 rgba(0, 0, 0, 0.12), 0 1px 1px 0 rgba(0, 0, 0, 0.08)',
        },
      },
    },
    MuiCard: {
      styleOverrides: {
        root: {
          borderRadius: 16,
          boxShadow: isDark
            ? '0 1px 3px 0 rgba(0, 0, 0, 0.3), 0 4px 8px 3px rgba(0, 0, 0, 0.15)'
            : '0 1px 3px 0 rgba(0, 0, 0, 0.12), 0 4px 8px 3px rgba(0, 0, 0, 0.08)',
        },
      },
    },
    MuiTextField: {
      styleOverrides: {
        root: {
          '& .MuiOutlinedInput-root': {
            borderRadius: 12,
          },
        },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: {
          borderRadius: 8,
        },
      },
    },
  },
});

// Light theme
export const lightTheme = createTheme({
  ...getCommonThemeOptions(false),
  palette: {
    mode: 'light',
    ...lightPalette,
  },
});

// Dark theme
export const darkTheme = createTheme({
  ...getCommonThemeOptions(true),
  palette: {
    mode: 'dark',
    ...darkPalette,
  },
});

// Theme utilities
export const getTheme = (mode: 'light' | 'dark') =>
  mode === 'dark' ? darkTheme : lightTheme;

export type AppTheme = typeof lightTheme;
