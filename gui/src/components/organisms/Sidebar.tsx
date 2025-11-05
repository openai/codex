import React from 'react';
import {
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Divider,
  Box,
  Typography,
  SxProps,
  Theme,
} from '@mui/material';
import { motion } from 'framer-motion';
import {
  Home,
  Code,
  Settings,
  FileText,
  BarChart3,
  Users,
  Shield,
  Zap,
  Search,
  Server,
} from 'lucide-react';

export interface NavigationItem {
  id: string;
  label: string;
  icon: React.ComponentType<any>;
  path?: string;
  badge?: string | number;
}

export interface SidebarProps {
  open: boolean;
  onClose: () => void;
  onNavigate: (item: NavigationItem) => void;
  activeItem?: string;
  width?: number;
  sx?: SxProps<Theme>;
}

const navigationItems: NavigationItem[] = [
  { id: 'dashboard', label: 'ダッシュボード', icon: Home },
  { id: 'code', label: 'コード実行', icon: Code },
  { id: 'agents', label: 'エージェント', icon: Users },
  { id: 'research', label: 'Deep Research', icon: Search },
  { id: 'security', label: 'セキュリティ', icon: Shield },
  { id: 'mcp', label: 'MCPサーバー', icon: Server },
  { id: 'analytics', label: '分析', icon: BarChart3 },
  { id: 'docs', label: 'ドキュメント', icon: FileText },
  { id: 'performance', label: 'パフォーマンス', icon: Zap },
];

const settingsItems: NavigationItem[] = [
  { id: 'settings', label: '設定', icon: Settings },
];

export const Sidebar: React.FC<SidebarProps> = ({
  open,
  onClose,
  onNavigate,
  activeItem,
  width = 280,
  sx,
}) => {
  const handleItemClick = (item: NavigationItem) => {
    onNavigate(item);
    // Mobileではクリック後に閉じる
    if (window.innerWidth < 768) {
      onClose();
    }
  };

  const sidebarContent = (
    <Box
      sx={{
        width,
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        background: (theme) =>
          theme.palette.mode === 'dark'
            ? 'linear-gradient(180deg, #0f1419 0%, #1d1b20 100%)'
            : 'linear-gradient(180deg, #fdfbff 0%, #e7e0ec 100%)',
        ...sx,
      }}
    >
      {/* Header */}
      <Box sx={{ p: 3, borderBottom: '1px solid', borderColor: 'outline.variant' }}>
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3 }}
        >
          <Typography
            variant="h6"
            sx={{
              fontWeight: 700,
              background: 'linear-gradient(45deg, #0061a4, #565f71)',
              backgroundClip: 'text',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
            }}
          >
            Codex Control
          </Typography>
          <Typography variant="caption" sx={{ color: 'text.secondary', mt: 0.5 }}>
            AI Assistant Platform
          </Typography>
        </motion.div>
      </Box>

      {/* Navigation */}
      <Box sx={{ flex: 1, overflow: 'auto', py: 2 }}>
        <List sx={{ px: 1 }}>
          {navigationItems.map((item, index) => {
            const Icon = item.icon;
            const isActive = activeItem === item.id;

            return (
              <motion.div
                key={item.id}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.05 }}
              >
                <ListItem disablePadding sx={{ mb: 0.5 }}>
                  <ListItemButton
                    onClick={() => handleItemClick(item)}
                    sx={{
                      borderRadius: 2,
                      mx: 1,
                      py: 1.5,
                      position: 'relative',
                      backgroundColor: isActive
                        ? 'primary.main'
                        : 'transparent',
                      color: isActive
                        ? 'primary.contrastText'
                        : 'text.primary',
                      '&:hover': {
                        backgroundColor: isActive
                          ? 'primary.dark'
                          : 'action.hover',
                      },
                      transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
                    }}
                  >
                    <ListItemIcon
                      sx={{
                        minWidth: 40,
                        color: 'inherit',
                      }}
                    >
                      <Icon size={20} />
                    </ListItemIcon>
                    <ListItemText
                      primary={item.label}
                      primaryTypographyProps={{
                        fontSize: '0.875rem',
                        fontWeight: isActive ? 600 : 500,
                      }}
                    />
                    {item.badge && (
                      <Box
                        sx={{
                          ml: 'auto',
                          px: 1,
                          py: 0.25,
                          borderRadius: 1,
                          fontSize: '0.75rem',
                          fontWeight: 600,
                          backgroundColor: isActive
                            ? 'rgba(255, 255, 255, 0.2)'
                            : 'primary.main',
                          color: isActive
                            ? 'inherit'
                            : 'primary.contrastText',
                        }}
                      >
                        {item.badge}
                      </Box>
                    )}
                  </ListItemButton>
                </ListItem>
              </motion.div>
            );
          })}
        </List>

        <Divider sx={{ my: 2, mx: 2 }} />

        <List sx={{ px: 1 }}>
          {settingsItems.map((item, index) => {
            const Icon = item.icon;
            const isActive = activeItem === item.id;

            return (
              <motion.div
                key={item.id}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: (navigationItems.length + index) * 0.05 }}
              >
                <ListItem disablePadding sx={{ mb: 0.5 }}>
                  <ListItemButton
                    onClick={() => handleItemClick(item)}
                    sx={{
                      borderRadius: 2,
                      mx: 1,
                      py: 1.5,
                      backgroundColor: isActive
                        ? 'primary.main'
                        : 'transparent',
                      color: isActive
                        ? 'primary.contrastText'
                        : 'text.secondary',
                      '&:hover': {
                        backgroundColor: isActive
                          ? 'primary.dark'
                          : 'action.hover',
                      },
                    }}
                  >
                    <ListItemIcon
                      sx={{
                        minWidth: 40,
                        color: 'inherit',
                      }}
                    >
                      <Icon size={20} />
                    </ListItemIcon>
                    <ListItemText
                      primary={item.label}
                      primaryTypographyProps={{
                        fontSize: '0.875rem',
                        fontWeight: isActive ? 600 : 500,
                      }}
                    />
                  </ListItemButton>
                </ListItem>
              </motion.div>
            );
          })}
        </List>
      </Box>
    </Box>
  );

  return (
    <>
      {/* Mobile Drawer */}
      <Drawer
        variant="temporary"
        open={open}
        onClose={onClose}
        ModalProps={{
          keepMounted: true, // Better mobile performance
        }}
        sx={{
          display: { xs: 'block', md: 'none' },
          '& .MuiDrawer-paper': {
            width,
            boxSizing: 'border-box',
          },
        }}
      >
        {sidebarContent}
      </Drawer>

      {/* Desktop Drawer */}
      <Drawer
        variant="permanent"
        sx={{
          display: { xs: 'none', md: 'block' },
          '& .MuiDrawer-paper': {
            width,
            boxSizing: 'border-box',
            borderRight: '1px solid',
            borderColor: 'outline.variant',
          },
        }}
        open
      >
        {sidebarContent}
      </Drawer>
    </>
  );
};

export default Sidebar;
