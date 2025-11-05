import React, { useState } from 'react';
import {
  Box,
  Container,
  useMediaQuery,
  useTheme as useMuiTheme,
  SxProps,
  Theme,
} from '@mui/material';
import { Header } from '@/components/organisms/Header';
import { Sidebar } from '@/components/organisms/Sidebar';
import { NavigationItem } from '@/components/organisms/Sidebar';
import { AppThemeProvider } from './ThemeProvider';

export interface DashboardLayoutProps {
  children: React.ReactNode;
  title?: string;
  activeNavItem?: string;
  onNavigate?: (item: NavigationItem) => void;
  onSettingsClick?: () => void;
  onProfileClick?: () => void;
  sx?: SxProps<Theme>;
}

export const DashboardLayout: React.FC<DashboardLayoutProps> = ({
  children,
  title,
  activeNavItem = 'dashboard',
  onNavigate,
  onSettingsClick,
  onProfileClick,
  sx,
}) => {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const muiTheme = useMuiTheme();
  const isMobile = useMediaQuery(muiTheme.breakpoints.down('md'));

  const handleMenuClick = () => {
    setSidebarOpen(!sidebarOpen);
  };

  const handleSidebarClose = () => {
    setSidebarOpen(false);
  };

  const handleNavigate = (item: NavigationItem) => {
    onNavigate?.(item);
  };

  const handleSettingsClick = () => {
    onSettingsClick?.();
  };

  const handleProfileClick = () => {
    onProfileClick?.();
  };

  return (
    <AppThemeProvider>
      <Box
        sx={{
          display: 'flex',
          minHeight: '100vh',
          backgroundColor: 'background.default',
          ...sx,
        }}
      >
        <Sidebar
          open={sidebarOpen}
          onClose={handleSidebarClose}
          onNavigate={handleNavigate}
          activeItem={activeNavItem}
        />

        <Box
          component="main"
          sx={{
            flexGrow: 1,
            display: 'flex',
            flexDirection: 'column',
            minWidth: 0, // Prevents flex item from overflowing
          }}
        >
          <Header
            title={title}
            onMenuClick={handleMenuClick}
            onSettingsClick={handleSettingsClick}
            onProfileClick={handleProfileClick}
            showMenuButton={isMobile}
          />

          <Container
            maxWidth="xl"
            sx={{
              flex: 1,
              py: 3,
              px: { xs: 2, sm: 3, md: 4 },
              display: 'flex',
              flexDirection: 'column',
            }}
          >
            {children}
          </Container>
        </Box>
      </Box>
    </AppThemeProvider>
  );
};

export default DashboardLayout;
