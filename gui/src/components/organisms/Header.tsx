import React from 'react';
import {
  AppBar,
  Toolbar,
  Typography,
  Box,
  Chip,
  Tooltip,
  SxProps,
  Theme,
} from '@mui/material';
import { motion } from 'framer-motion';
import { IconButton } from '@/components/atoms/IconButton';
import { useAppTheme } from '@/components/templates/ThemeProvider';
import { useCodex } from '@/lib/context/CodexContext';
import { Sun, Moon, Monitor, Menu, Settings, User, Wifi, WifiOff, AlertCircle } from 'lucide-react';

export interface HeaderProps {
  title?: string;
  onMenuClick?: () => void;
  onSettingsClick?: () => void;
  onProfileClick?: () => void;
  showMenuButton?: boolean;
  sx?: SxProps<Theme>;
}

export const Header: React.FC<HeaderProps> = ({
  title = 'Codex GUI',
  onMenuClick,
  onSettingsClick,
  onProfileClick,
  showMenuButton = true,
  sx,
}) => {
  const { theme, toggleTheme } = useAppTheme();
  const { state } = useCodex();

  const getThemeIcon = () => {
    switch (theme) {
      case 'light':
        return Sun;
      case 'dark':
        return Moon;
      default:
        return Monitor;
    }
  };

  const ThemeIcon = getThemeIcon();

  return (
    <AppBar
      position="sticky"
      elevation={0}
      sx={{
        backgroundColor: 'background.paper',
        borderBottom: '1px solid',
        borderColor: 'outline.variant',
        backdropFilter: 'blur(12px)',
        ...sx,
      }}
    >
      <Toolbar sx={{ minHeight: 64 }}>
        {showMenuButton && (
          <motion.div
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <IconButton
              icon={Menu}
              tooltip="メニューを開く"
              onClick={onMenuClick}
              sx={{ mr: 2 }}
            />
          </motion.div>
        )}

        <motion.div
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.5 }}
        >
          <Typography
            variant="h6"
            component="div"
            sx={{
              flexGrow: 1,
              fontWeight: 600,
              background: 'linear-gradient(45deg, #0061a4, #565f71)',
              backgroundClip: 'text',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
            }}
          >
            {title}
          </Typography>
        </motion.div>

        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          {/* Connection Status */}
          <motion.div
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ duration: 0.3 }}
          >
            <Tooltip title={`Codexサーバー: ${state.isConnected ? '接続済み' : '未接続'}`}>
              <Chip
                icon={state.isConnected ? <Wifi size={16} /> : <WifiOff size={16} />}
                label={state.isConnected ? 'オンライン' : 'オフライン'}
                size="small"
                color={state.isConnected ? 'success' : 'error'}
                variant="outlined"
                sx={{
                  '& .MuiChip-icon': {
                    color: state.isConnected ? 'success.main' : 'error.main',
                  },
                }}
              />
            </Tooltip>
          </motion.div>

          {/* Error Notification */}
          {state.error && (
            <motion.div
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 0.3 }}
            >
              <Tooltip title={state.error}>
                <IconButton
                  icon={AlertCircle}
                  tooltip="エラー詳細"
                  variant="outlined"
                  size="small"
                  sx={{
                    color: 'error.main',
                    borderColor: 'error.main',
                    '&:hover': {
                      backgroundColor: 'error.light',
                      borderColor: 'error.dark',
                    },
                  }}
                />
              </Tooltip>
            </motion.div>
          )}

          <motion.div
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <IconButton
              icon={ThemeIcon}
              tooltip={`テーマ切り替え (${theme})`}
              onClick={toggleTheme}
              variant="outlined"
              size="small"
            />
          </motion.div>

          <motion.div
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <IconButton
              icon={Settings}
              tooltip="設定"
              onClick={onSettingsClick}
              variant="outlined"
              size="small"
            />
          </motion.div>

          <motion.div
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <IconButton
              icon={User}
              tooltip="プロフィール"
              onClick={onProfileClick}
              variant="outlined"
              size="small"
            />
          </motion.div>
        </Box>
      </Toolbar>
    </AppBar>
  );
};

export default Header;
