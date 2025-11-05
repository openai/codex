'use client';

import React, { useState } from 'react';
import {
  Box,
  Typography,
  Grid,
  Card as MuiCard,
  CardContent,
  CardActions,
  Button,
  Chip,
  Avatar,
  Switch,
  FormControlLabel,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Alert,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Tooltip,
  IconButton,
  LinearProgress,
  Tabs,
  Tab,
} from '@mui/material';
import {
  Server,
  Database,
  Code,
  Globe,
  Monitor,
  Settings,
  CheckCircle,
  XCircle,
  AlertTriangle,
  RefreshCw,
  Plus,
  Edit,
  Trash2,
} from 'lucide-react';
import { DashboardLayout } from '@/components/templates/DashboardLayout';
import { Card } from '@/components/atoms/Card';
import { useCodex } from '@/lib/context/CodexContext';

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel(props: TabPanelProps) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`mcp-tabpanel-${index}`}
      aria-labelledby={`mcp-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

const MCP_SERVER_TYPES = [
  { id: 'filesystem', name: 'File System', icon: Database, description: 'ローカルファイルシステムアクセス' },
  { id: 'github', name: 'GitHub', icon: Code, description: 'GitHub API連携' },
  { id: 'sequential-thinking', name: 'Sequential Thinking', icon: Settings, description: '順次思考プロセス' },
  { id: 'playwright', name: 'Playwright', icon: Monitor, description: 'ブラウザ自動化' },
  { id: 'gemini', name: 'Gemini', icon: Globe, description: 'Google Gemini AI' },
  { id: 'chrome-mcp', name: 'Chrome MCP', icon: Globe, description: 'Chrome拡張連携' },
];

export default function MCPPage() {
  const { state } = useCodex();
  const [activeTab, setActiveTab] = useState(0);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<any>(null);
  const [newServer, setNewServer] = useState({
    name: '',
    type: '',
    url: '',
    enabled: true,
  });

  const getServerIcon = (type: string) => {
    const serverType = MCP_SERVER_TYPES.find(t => t.id === type);
    return serverType ? serverType.icon : Server;
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'connected':
        return <CheckCircle size={16} color="#4caf50" />;
      case 'disconnected':
        return <XCircle size={16} color="#f44336" />;
      case 'error':
        return <AlertTriangle size={16} color="#ff9800" />;
      default:
        return <RefreshCw size={16} />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'connected':
        return 'success';
      case 'disconnected':
        return 'error';
      case 'error':
        return 'warning';
      default:
        return 'default';
    }
  };

  const handleAddServer = () => {
    // In a real implementation, this would add the server to the configuration
    console.log('Adding server:', newServer);
    setAddDialogOpen(false);
    setNewServer({ name: '', type: '', url: '', enabled: true });
  };

  const handleEditServer = (server: any) => {
    setEditingServer(server);
  };

  const handleSaveEdit = () => {
    // In a real implementation, this would update the server configuration
    console.log('Saving server:', editingServer);
    setEditingServer(null);
  };

  const handleDeleteServer = (serverId: string) => {
    // In a real implementation, this would remove the server from configuration
    console.log('Deleting server:', serverId);
  };

  const handleToggleServer = (serverId: string, enabled: boolean) => {
    // In a real implementation, this would enable/disable the server
    console.log('Toggling server:', serverId, enabled);
  };

  // Mock data for demonstration
  const mockServerStats = {
    totalServers: state.mcpConnections.length,
    connectedServers: state.mcpConnections.filter(c => c.status === 'connected').length,
    totalRequests: 1250,
    avgResponseTime: 245, // ms
  };

  return (
    <DashboardLayout title="MCPサーバー管理">
      <Box sx={{ p: 3 }}>
        <Typography variant="h4" sx={{ mb: 2, fontWeight: 700 }}>
          MCPサーバー管理
        </Typography>
        <Typography variant="body1" color="text.secondary" sx={{ mb: 4 }}>
          Model Context Protocolサーバーの接続状態を管理し、外部サービスとの連携を設定します。
        </Typography>

        {/* MCP Stats */}
        <Grid container spacing={3} sx={{ mb: 4 }}>
          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'primary.main' }}>
                  <Server size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700 }}>
                    {mockServerStats.totalServers}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    総サーバー数
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>

          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'success.main' }}>
                  <CheckCircle size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700, color: 'success.main' }}>
                    {mockServerStats.connectedServers}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    接続済み
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>

          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'info.main' }}>
                  <RefreshCw size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700 }}>
                    {mockServerStats.totalRequests.toLocaleString()}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    リクエスト数
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>

          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'warning.main' }}>
                  <AlertTriangle size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700 }}>
                    {mockServerStats.avgResponseTime}ms
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    平均応答時間
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>
        </Grid>

        {/* Server Management */}
        <MuiCard>
          <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
            <Tabs value={activeTab} onChange={(_, newValue) => setActiveTab(newValue)}>
              <Tab label="サーバー一覧" />
              <Tab label="パフォーマンス" />
              <Tab label="設定" />
            </Tabs>
          </Box>

          <TabPanel value={activeTab} index={0}>
            <Box sx={{ mb: 2 }}>
              <Button
                variant="contained"
                startIcon={<Plus />}
                onClick={() => setAddDialogOpen(true)}
                sx={{
                  background: 'linear-gradient(45deg, #1976d2, #2196f3)',
                  '&:hover': {
                    background: 'linear-gradient(45deg, #0d47a1, #1976d2)',
                  },
                }}
              >
                サーバー追加
              </Button>
            </Box>

            <Grid container spacing={3}>
              {state.mcpConnections.map((server) => {
                const IconComponent = getServerIcon(server.type);
                return (
                  <Grid item xs={12} md={6} lg={4} key={server.id}>
                    <MuiCard
                      sx={{
                        height: '100%',
                        display: 'flex',
                        flexDirection: 'column',
                        transition: 'all 0.3s ease',
                        '&:hover': {
                          transform: 'translateY(-2px)',
                          boxShadow: 3,
                        },
                      }}
                    >
                      <CardContent sx={{ flex: 1 }}>
                        <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                          <Avatar sx={{ mr: 2 }}>
                            <IconComponent size={20} />
                          </Avatar>
                          <Box sx={{ flex: 1 }}>
                            <Typography variant="h6" sx={{ fontWeight: 600 }}>
                              {server.name}
                            </Typography>
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                              {getStatusIcon(server.status)}
                              <Chip
                                label={server.status === 'connected' ? '接続済み' :
                                      server.status === 'disconnected' ? '未接続' : 'エラー'}
                                size="small"
                                color={getStatusColor(server.status)}
                                variant="outlined"
                              />
                            </Box>
                          </Box>
                        </Box>

                        <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                          {MCP_SERVER_TYPES.find(t => t.id === server.type)?.description || 'MCPサーバー'}
                        </Typography>

                        {server.lastConnected && (
                          <Typography variant="caption" color="text.secondary">
                            最終接続: {new Date(server.lastConnected).toLocaleString('ja-JP')}
                          </Typography>
                        )}

                        {server.url && (
                          <Typography variant="caption" color="text.secondary" sx={{ display: 'block' }}>
                            URL: {server.url}
                          </Typography>
                        )}
                      </CardContent>

                      <CardActions sx={{ justifyContent: 'space-between', px: 2, pb: 2 }}>
                        <FormControlLabel
                          control={
                            <Switch
                              checked={server.status === 'connected'}
                              onChange={(e) => handleToggleServer(server.id, e.target.checked)}
                              color="primary"
                            />
                          }
                          label="有効"
                        />

                        <Box sx={{ display: 'flex', gap: 1 }}>
                          <Tooltip title="編集">
                            <IconButton size="small" onClick={() => handleEditServer(server)}>
                              <Edit size={16} />
                            </IconButton>
                          </Tooltip>
                          <Tooltip title="削除">
                            <IconButton
                              size="small"
                              onClick={() => handleDeleteServer(server.id)}
                              sx={{ color: 'error.main' }}
                            >
                              <Trash2 size={16} />
                            </IconButton>
                          </Tooltip>
                        </Box>
                      </CardActions>
                    </MuiCard>
                  </Grid>
                );
              })}
            </Grid>
          </TabPanel>

          <TabPanel value={activeTab} index={1}>
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <Alert severity="info">
                MCPサーバーのパフォーマンス監視と最適化を行います。
              </Alert>

              <Grid container spacing={3}>
                <Grid item xs={12} md={6}>
                  <Card header="応答時間分布">
                    <Box sx={{ textAlign: 'center', py: 2 }}>
                      <Typography variant="h3" sx={{ color: 'success.main', fontWeight: 700 }}>
                        245ms
                      </Typography>
                      <Typography variant="body2" color="text.secondary">
                        平均応答時間
                      </Typography>
                      <LinearProgress
                        variant="determinate"
                        value={75}
                        sx={{ mt: 2, height: 8, borderRadius: 4 }}
                      />
                    </Box>
                  </Card>
                </Grid>

                <Grid item xs={12} md={6}>
                  <Card header="エラー率">
                    <Box sx={{ textAlign: 'center', py: 2 }}>
                      <Typography variant="h3" sx={{ color: 'error.main', fontWeight: 700 }}>
                        2.1%
                      </Typography>
                      <Typography variant="body2" color="text.secondary">
                        エラー発生率
                      </Typography>
                      <LinearProgress
                        variant="determinate"
                        value={2.1}
                        color="error"
                        sx={{ mt: 2, height: 8, borderRadius: 4 }}
                      />
                    </Box>
                  </Card>
                </Grid>
              </Grid>

              <Card header="サーバー別パフォーマンス">
                <TableContainer>
                  <Table>
                    <TableHead>
                      <TableRow>
                        <TableCell>サーバー</TableCell>
                        <TableCell>リクエスト数</TableCell>
                        <TableCell>平均応答時間</TableCell>
                        <TableCell>エラー率</TableCell>
                        <TableCell>ステータス</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {state.mcpConnections.map((server) => (
                        <TableRow key={server.id} hover>
                          <TableCell>
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                              {React.createElement(getServerIcon(server.type), { size: 16 })}
                              <Typography variant="body2">{server.name}</Typography>
                            </Box>
                          </TableCell>
                          <TableCell>125</TableCell>
                          <TableCell>245ms</TableCell>
                          <TableCell>0.5%</TableCell>
                          <TableCell>
                            <Chip
                              label={server.status === 'connected' ? '正常' : '異常'}
                              size="small"
                              color={server.status === 'connected' ? 'success' : 'error'}
                              variant="outlined"
                            />
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </TableContainer>
              </Card>
            </Box>
          </TabPanel>

          <TabPanel value={activeTab} index={2}>
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <Alert severity="warning">
                MCPサーバーの設定変更はシステムの再起動を必要とする場合があります。
              </Alert>

              <Card header="グローバル設定">
                <Grid container spacing={3}>
                  <Grid item xs={12} md={6}>
                    <TextField
                      fullWidth
                      label="接続タイムアウト"
                      defaultValue="30"
                      helperText="秒単位"
                      type="number"
                    />
                  </Grid>
                  <Grid item xs={12} md={6}>
                    <TextField
                      fullWidth
                      label="最大同時接続数"
                      defaultValue="10"
                      type="number"
                    />
                  </Grid>
                  <Grid item xs={12}>
                    <FormControlLabel
                      control={<Switch defaultChecked />}
                      label="自動再接続"
                    />
                    <FormControlLabel
                      control={<Switch defaultChecked />}
                      label="ヘルスチェック有効"
                    />
                  </Grid>
                </Grid>
              </Card>

              <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 2 }}>
                <Button variant="outlined">デフォルトに戻す</Button>
                <Button variant="contained">設定保存</Button>
              </Box>
            </Box>
          </TabPanel>
        </MuiCard>

        {/* Add Server Dialog */}
        <Dialog open={addDialogOpen} onClose={() => setAddDialogOpen(false)} maxWidth="sm" fullWidth>
          <DialogTitle>MCPサーバー追加</DialogTitle>
          <DialogContent>
            <TextField
              fullWidth
              label="サーバー名"
              value={newServer.name}
              onChange={(e) => setNewServer(prev => ({ ...prev, name: e.target.value }))}
              sx={{ mt: 2, mb: 2 }}
            />
            <TextField
              fullWidth
              select
              label="サーバータイプ"
              value={newServer.type}
              onChange={(e) => setNewServer(prev => ({ ...prev, type: e.target.value }))}
              sx={{ mb: 2 }}
            >
              {MCP_SERVER_TYPES.map((type) => (
                <MenuItem key={type.id} value={type.id}>
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                    <type.icon size={16} />
                    {type.name}
                  </Box>
                </MenuItem>
              ))}
            </TextField>
            <TextField
              fullWidth
              label="接続URL"
              value={newServer.url}
              onChange={(e) => setNewServer(prev => ({ ...prev, url: e.target.value }))}
              placeholder="http://localhost:3000"
              sx={{ mb: 2 }}
            />
            <FormControlLabel
              control={
                <Switch
                  checked={newServer.enabled}
                  onChange={(e) => setNewServer(prev => ({ ...prev, enabled: e.target.checked }))}
                />
              }
              label="有効化"
            />
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setAddDialogOpen(false)}>キャンセル</Button>
            <Button onClick={handleAddServer} variant="contained">
              追加
            </Button>
          </DialogActions>
        </Dialog>

        {/* Edit Server Dialog */}
        {editingServer && (
          <Dialog open={!!editingServer} onClose={() => setEditingServer(null)} maxWidth="sm" fullWidth>
            <DialogTitle>MCPサーバー編集</DialogTitle>
            <DialogContent>
              <TextField
                fullWidth
                label="サーバー名"
                value={editingServer.name}
                onChange={(e) => setEditingServer(prev => ({ ...prev, name: e.target.value }))}
                sx={{ mt: 2, mb: 2 }}
              />
              <TextField
                fullWidth
                label="接続URL"
                value={editingServer.url || ''}
                onChange={(e) => setEditingServer(prev => ({ ...prev, url: e.target.value }))}
                sx={{ mb: 2 }}
              />
              <FormControlLabel
                control={
                  <Switch
                    checked={editingServer.status === 'connected'}
                    onChange={(e) => setEditingServer(prev => ({
                      ...prev,
                      status: e.target.checked ? 'connected' : 'disconnected'
                    }))}
                  />
                }
                label="有効化"
              />
            </DialogContent>
            <DialogActions>
              <Button onClick={() => setEditingServer(null)}>キャンセル</Button>
              <Button onClick={handleSaveEdit} variant="contained">
                保存
              </Button>
            </DialogActions>
          </Dialog>
        )}
      </Box>
    </DashboardLayout>
  );
}
