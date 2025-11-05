'use client';

import React, { useState } from 'react';
import {
  Grid,
  Typography,
  Box,
  Paper,
  Chip,
  Avatar,
  LinearProgress,
} from '@mui/material';
import { motion } from 'framer-motion';
import {
  DashboardLayout,
} from '@/components/templates/DashboardLayout';
import { Card } from '@/components/atoms/Card';
import { Button } from '@/components/atoms/Button';
import { LoadingSpinner } from '@/components/molecules/LoadingSpinner';
import {
  Brain,
  Code,
  Shield,
  Zap,
  TrendingUp,
  Users,
  Activity,
  Settings,
} from 'lucide-react';

interface StatCardProps {
  title: string;
  value: string;
  change: string;
  icon: React.ComponentType<any>;
  color: string;
}

const StatCard: React.FC<StatCardProps> = ({ title, value, change, icon: Icon, color }) => (
  <motion.div
    initial={{ opacity: 0, y: 20 }}
    animate={{ opacity: 1, y: 0 }}
    transition={{ duration: 0.5 }}
  >
    <Card hover>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
        <Avatar sx={{ bgcolor: `${color}.main`, width: 48, height: 48 }}>
          <Icon size={24} />
        </Avatar>
        <Box sx={{ flex: 1 }}>
          <Typography variant="h4" sx={{ fontWeight: 700, mb: 0.5 }}>
            {value}
          </Typography>
          <Typography variant="body2" sx={{ color: 'text.secondary', mb: 1 }}>
            {title}
          </Typography>
          <Chip
            label={change}
            size="small"
            sx={{
              bgcolor: change.startsWith('+') ? 'success.main' : 'warning.main',
              color: 'white',
              fontWeight: 600,
            }}
          />
        </Box>
      </Box>
    </Card>
  </motion.div>
);

interface RecentActivityProps {
  activities: Array<{
    id: string;
    type: string;
    description: string;
    timestamp: string;
    status: 'success' | 'warning' | 'error';
  }>;
}

const RecentActivity: React.FC<RecentActivityProps> = ({ activities }) => (
  <Card header="最近のアクティビティ">
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      {activities.map((activity, index) => (
        <motion.div
          key={activity.id}
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: index * 0.1 }}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 2,
            padding: '12px',
            borderRadius: '8px',
            backgroundColor: 'background.paper',
            border: '1px solid',
            borderColor: 'outline.variant',
          }}
        >
          <Box
            sx={{
              width: 8,
              height: 8,
              borderRadius: '50%',
              bgcolor: activity.status === 'success'
                ? 'success.main'
                : activity.status === 'warning'
                ? 'warning.main'
                : 'error.main',
            }}
          />
          <Box sx={{ flex: 1 }}>
            <Typography variant="body2" sx={{ fontWeight: 500 }}>
              {activity.description}
            </Typography>
            <Typography variant="caption" sx={{ color: 'text.secondary' }}>
              {activity.timestamp}
            </Typography>
          </Box>
        </motion.div>
      ))}
    </Box>
  </Card>
);

export default function Dashboard() {
  const { state, clearError } = useCodex();
  const [isLoading, setIsLoading] = useState(false);

  const stats = [
    {
      title: '実行中のタスク',
      value: state.agents.filter(a => a.status === 'working').length.toString(),
      change: '+2',
      icon: Activity,
      color: 'primary',
    },
    {
      title: 'アクティブエージェント',
      value: state.agents.length.toString(),
      change: `+${state.agents.filter(a => a.status === 'working').length}`,
      icon: Users,
      color: 'secondary',
    },
    {
      title: 'セキュリティスコア',
      value: '98%',
      change: '+5%',
      icon: Shield,
      color: 'success',
    },
    {
      title: '接続状態',
      value: state.isConnected ? 'オンライン' : 'オフライン',
      change: state.isConnected ? '接続済み' : '再接続中',
      icon: Zap,
      color: state.isConnected ? 'success' : 'warning',
    },
  ];

  // Convert notifications to activities format
  const recentActivities = state.notifications.slice(0, 4).map(notification => ({
    id: notification.id,
    type: notification.type,
    description: notification.message,
    timestamp: new Date(notification.timestamp).toLocaleString('ja-JP', {
      hour: '2-digit',
      minute: '2-digit'
    }),
    status: notification.type as 'success' | 'warning' | 'error',
  }));

  const handleQuickAction = (action: string) => {
    setIsLoading(true);
    // Simulate async operation
    setTimeout(() => {
      setIsLoading(false);
      console.log(`Quick action: ${action}`);
    }, 2000);
  };

  return (
    <DashboardLayout title="Codex Dashboard">
      {isLoading && <LoadingSpinner overlay message="処理中..." />}

      <Box sx={{ mb: 4 }}>
        <motion.div
          initial={{ opacity: 0, y: -20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
        >
          <Typography variant="h4" sx={{ fontWeight: 700, mb: 2 }}>
            ようこそ、Codexへ
          </Typography>
          <Typography variant="body1" sx={{ color: 'text.secondary', mb: 4 }}>
            AIアシスタントプラットフォームで、効率的な開発と自動化を実現しましょう。
          </Typography>
        </motion.div>
      </Box>

      {/* Stats Grid */}
      <Grid container spacing={3} sx={{ mb: 4 }}>
        {stats.map((stat, index) => (
          <Grid item xs={12} sm={6} lg={3} key={stat.title}>
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: index * 0.1 }}
            >
              <StatCard {...stat} />
            </motion.div>
          </Grid>
        ))}
      </Grid>

      {/* Main Content */}
      <Grid container spacing={3}>
        {/* Quick Actions */}
        <Grid item xs={12} md={6}>
          <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 0.3 }}
          >
            <Card header="クイックアクション">
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Button
                  fullWidth
                  startIcon={<Code size={20} />}
                  onClick={() => handleQuickAction('code-review')}
                  variant="contained"
                >
                  コードレビュー開始
                </Button>
                <Button
                  fullWidth
                  startIcon={<Brain size={20} />}
                  onClick={() => handleQuickAction('deep-research')}
                  variant="outlined"
                >
                  Deep Research実行
                </Button>
                <Button
                  fullWidth
                  startIcon={<Shield size={20} />}
                  onClick={() => handleQuickAction('security-scan')}
                  variant="outlined"
                >
                  セキュリティスキャン
                </Button>
                <Button
                  fullWidth
                  startIcon={<Settings size={20} />}
                  onClick={() => handleQuickAction('configure')}
                  variant="text"
                >
                  エージェント設定
                </Button>
              </Box>
            </Card>
          </motion.div>
        </Grid>

        {/* Recent Activity */}
        <Grid item xs={12} md={6}>
          <motion.div
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: 0.4 }}
          >
            <RecentActivity activities={recentActivities} />
          </motion.div>
        </Grid>

        {/* Performance Overview */}
        <Grid item xs={12}>
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.5 }}
          >
            <Card header="パフォーマンス概要">
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                <Box>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                    <Typography variant="body2" sx={{ fontWeight: 500 }}>
                      CPU使用率
                    </Typography>
                    <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                      45%
                    </Typography>
                  </Box>
                  <LinearProgress
                    variant="determinate"
                    value={45}
                    sx={{
                      height: 8,
                      borderRadius: 4,
                      bgcolor: 'background.paper',
                      '& .MuiLinearProgress-bar': {
                        borderRadius: 4,
                        background: 'linear-gradient(45deg, #0061a4, #565f71)',
                      },
                    }}
                  />
                </Box>

                <Box>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                    <Typography variant="body2" sx={{ fontWeight: 500 }}>
                      メモリ使用率
                    </Typography>
                    <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                      67%
                    </Typography>
                  </Box>
                  <LinearProgress
                    variant="determinate"
                    value={67}
                    sx={{
                      height: 8,
                      borderRadius: 4,
                      bgcolor: 'background.paper',
                      '& .MuiLinearProgress-bar': {
                        borderRadius: 4,
                        background: 'linear-gradient(45deg, #146c2e, #7d5800)',
                      },
                    }}
                  />
                </Box>

                <Box>
                  <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                    <Typography variant="body2" sx={{ fontWeight: 500 }}>
                      ディスク使用率
                    </Typography>
                    <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                      23%
                    </Typography>
                  </Box>
                  <LinearProgress
                    variant="determinate"
                    value={23}
                    sx={{
                      height: 8,
                      borderRadius: 4,
                      bgcolor: 'background.paper',
                      '& .MuiLinearProgress-bar': {
                        borderRadius: 4,
                        background: 'linear-gradient(45deg, #7d5800, #ba1a1a)',
                      },
                    }}
                  />
                </Box>
              </Box>
            </Card>
          </motion.div>
        </Grid>
      </Grid>
    </DashboardLayout>
  );
}
