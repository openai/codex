'use client';

import React, { useState } from 'react';
import {
  Box,
  Typography,
  Grid,
  Card as MuiCard,
  CardContent,
  Chip,
  Avatar,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Alert,
  LinearProgress,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Tooltip,
  IconButton,
  Tabs,
  Tab,
} from '@mui/material';
import {
  Shield,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Clock,
  Play,
  FileText,
  Code,
  Database,
  Globe,
  Lock,
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
      id={`security-tabpanel-${index}`}
      aria-labelledby={`security-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

export default function SecurityPage() {
  const { state, runSecurityScan } = useCodex();
  const [scanDialogOpen, setScanDialogOpen] = useState(false);
  const [scanTarget, setScanTarget] = useState('');
  const [scanType, setScanType] = useState('code');
  const [isScanning, setIsScanning] = useState(false);
  const [activeTab, setActiveTab] = useState(0);

  const handleScan = async () => {
    if (!scanTarget.trim()) return;

    setIsScanning(true);
    try {
      await runSecurityScan(scanType, scanTarget);
      setScanDialogOpen(false);
      setScanTarget('');
    } catch (error) {
      console.error('Security scan failed:', error);
    } finally {
      setIsScanning(false);
    }
  };

  const getSeverityIcon = (severity: string) => {
    switch (severity) {
      case 'critical':
        return <XCircle size={16} color="#d32f2f" />;
      case 'high':
        return <AlertTriangle size={16} color="#f57c00" />;
      case 'medium':
        return <AlertTriangle size={16} color="#fbc02d" />;
      case 'low':
        return <AlertTriangle size={16} color="#388e3c" />;
      default:
        return <CheckCircle size={16} color="#388e3c" />;
    }
  };

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case 'critical':
        return 'error';
      case 'high':
        return 'warning';
      case 'medium':
        return 'warning';
      case 'low':
        return 'success';
      default:
        return 'default';
    }
  };

  const getScanStatusColor = (status: string) => {
    switch (status) {
      case 'running':
        return 'warning';
      case 'completed':
        return 'success';
      case 'failed':
        return 'error';
      default:
        return 'default';
    }
  };

  const getScanTypeIcon = (type: string) => {
    switch (type) {
      case 'dependency':
        return <Database size={16} />;
      case 'code':
        return <Code size={16} />;
      case 'secrets':
        return <Lock size={16} />;
      default:
        return <Shield size={16} />;
    }
  };

  // Mock data for demonstration
  const mockFindings = [
    {
      id: '1',
      severity: 'high',
      title: 'SQLインジェクションの脆弱性',
      description: 'ユーザー入力が適切にサニタイズされていない可能性があります',
      location: { file: 'src/api/user.js', line: 45 },
      recommendation: 'プリペアドステートメントを使用するか、入力を適切にエスケープしてください',
    },
    {
      id: '2',
      severity: 'medium',
      title: '機密情報のハードコーディング',
      description: 'APIキーがソースコードに直接記述されています',
      location: { file: 'config/database.js', line: 12 },
      recommendation: '環境変数または秘密管理サービスを使用してください',
    },
    {
      id: '3',
      severity: 'low',
      title: '古い依存関係',
      description: '使用しているライブラリにセキュリティアップデートがあります',
      location: { file: 'package.json' },
      recommendation: '依存関係を最新バージョンに更新してください',
    },
  ];

  const securityStats = {
    totalScans: state.securityScans.length,
    criticalVulnerabilities: mockFindings.filter(f => f.severity === 'critical').length,
    highVulnerabilities: mockFindings.filter(f => f.severity === 'high').length,
    mediumVulnerabilities: mockFindings.filter(f => f.severity === 'medium').length,
    lowVulnerabilities: mockFindings.filter(f => f.severity === 'low').length,
    lastScanDate: new Date().toLocaleDateString('ja-JP'),
  };

  return (
    <DashboardLayout title="セキュリティダッシュボード">
      <Box sx={{ p: 3 }}>
        <Typography variant="h4" sx={{ mb: 2, fontWeight: 700 }}>
          セキュリティダッシュボード
        </Typography>
        <Typography variant="body1" color="text.secondary" sx={{ mb: 4 }}>
          コード、依存関係、設定のセキュリティを継続的に監視し、脆弱性を検出します。
        </Typography>

        {/* Security Stats */}
        <Grid container spacing={3} sx={{ mb: 4 }}>
          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'success.main' }}>
                  <CheckCircle size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700 }}>
                    {securityStats.totalScans}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    総スキャン数
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>

          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'error.main' }}>
                  <XCircle size={20} />
                </Avatar>
                <Box>
                  <Typography variant="h4" sx={{ fontWeight: 700, color: 'error.main' }}>
                    {securityStats.criticalVulnerabilities}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    クリティカル
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
                  <Typography variant="h4" sx={{ fontWeight: 700, color: 'warning.main' }}>
                    {securityStats.highVulnerabilities}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    高リスク
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>

          <Grid item xs={12} md={3}>
            <Card>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Avatar sx={{ bgcolor: 'info.main' }}>
                  <Clock size={20} />
                </Avatar>
                <Box>
                  <Typography variant="body2" sx={{ fontWeight: 600 }}>
                    最終スキャン
                  </Typography>
                  <Typography variant="caption" color="text.secondary">
                    {securityStats.lastScanDate}
                  </Typography>
                </Box>
              </Box>
            </Card>
          </Grid>
        </Grid>

        {/* Quick Actions */}
        <Card header="セキュリティスキャン" sx={{ mb: 4 }}>
          <Grid container spacing={2}>
            <Grid item xs={12} md={4}>
              <Button
                fullWidth
                variant="contained"
                startIcon={<Code />}
                onClick={() => {
                  setScanType('code');
                  setScanDialogOpen(true);
                }}
                sx={{
                  py: 2,
                  background: 'linear-gradient(45deg, #1976d2, #2196f3)',
                  '&:hover': {
                    background: 'linear-gradient(45deg, #0d47a1, #1976d2)',
                  },
                }}
              >
                コードスキャン
              </Button>
            </Grid>
            <Grid item xs={12} md={4}>
              <Button
                fullWidth
                variant="contained"
                startIcon={<Database />}
                onClick={() => {
                  setScanType('dependency');
                  setScanDialogOpen(true);
                }}
                sx={{
                  py: 2,
                  background: 'linear-gradient(45deg, #388e3c, #4caf50)',
                  '&:hover': {
                    background: 'linear-gradient(45deg, #1b5e20, #388e3c)',
                  },
                }}
              >
                依存関係スキャン
              </Button>
            </Grid>
            <Grid item xs={12} md={4}>
              <Button
                fullWidth
                variant="contained"
                startIcon={<Lock />}
                onClick={() => {
                  setScanType('secrets');
                  setScanDialogOpen(true);
                }}
                sx={{
                  py: 2,
                  background: 'linear-gradient(45deg, #f57c00, #ff9800)',
                  '&:hover': {
                    background: 'linear-gradient(45deg, #e65100, #f57c00)',
                  },
                }}
              >
                シークレットスキャン
              </Button>
            </Grid>
          </Grid>
        </Card>

        {/* Scan Results */}
        <MuiCard>
          <Box sx={{ borderBottom: 1, borderColor: 'divider' }}>
            <Tabs value={activeTab} onChange={(_, newValue) => setActiveTab(newValue)}>
              <Tab label="脆弱性一覧" />
              <Tab label="スキャン履歴" />
              <Tab label="セキュリティレポート" />
            </Tabs>
          </Box>

          <TabPanel value={activeTab} index={0}>
            <TableContainer>
              <Table>
                <TableHead>
                  <TableRow>
                    <TableCell>深刻度</TableCell>
                    <TableCell>タイトル</TableCell>
                    <TableCell>場所</TableCell>
                    <TableCell>推奨事項</TableCell>
                    <TableCell>アクション</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {mockFindings.map((finding) => (
                    <TableRow key={finding.id} hover>
                      <TableCell>
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                          {getSeverityIcon(finding.severity)}
                          <Chip
                            label={finding.severity}
                            size="small"
                            color={getSeverityColor(finding.severity)}
                            variant="outlined"
                          />
                        </Box>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontWeight: 500 }}>
                          {finding.title}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          {finding.description}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2">
                          {finding.location.file}
                          {finding.location.line && `:${finding.location.line}`}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ maxWidth: 200 }}>
                          {finding.recommendation}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Button size="small" variant="outlined">
                          修正
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </TabPanel>

          <TabPanel value={activeTab} index={1}>
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              {state.securityScans.map((scan) => (
                <MuiCard key={scan.id} variant="outlined">
                  <CardContent>
                    <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                        {getScanTypeIcon(scan.type)}
                        <Typography variant="h6">
                          {scan.type === 'code' ? 'コードスキャン' :
                           scan.type === 'dependency' ? '依存関係スキャン' :
                           scan.type === 'secrets' ? 'シークレットスキャン' : 'セキュリティスキャン'}
                        </Typography>
                        <Chip
                          label={scan.status === 'running' ? '実行中' :
                                scan.status === 'completed' ? '完了' : '失敗'}
                          color={getScanStatusColor(scan.status)}
                          size="small"
                        />
                      </Box>
                      <Typography variant="caption" color="text.secondary">
                        {new Date(scan.startedAt).toLocaleString('ja-JP')}
                      </Typography>
                    </Box>

                    {scan.status === 'running' && (
                      <LinearProgress sx={{ mb: 1 }} />
                    )}

                    <Typography variant="body2" color="text.secondary">
                      検出された脆弱性: {scan.findings?.length || 0}件
                    </Typography>
                  </CardContent>
                </MuiCard>
              ))}
            </Box>
          </TabPanel>

          <TabPanel value={activeTab} index={2}>
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <Alert severity="success">
                <Typography variant="body2">
                  セキュリティスコア: 92/100 - 良好なセキュリティ状態を維持しています。
                </Typography>
              </Alert>

              <Grid container spacing={3}>
                <Grid item xs={12} md={6}>
                  <Card header="脆弱性の傾向">
                    <Box sx={{ textAlign: 'center', py: 2 }}>
                      <Typography variant="h3" sx={{ color: 'success.main', fontWeight: 700 }}>
                        ↓ 15%
                      </Typography>
                      <Typography variant="body2" color="text.secondary">
                        先月比脆弱性減少
                      </Typography>
                    </Box>
                  </Card>
                </Grid>

                <Grid item xs={12} md={6}>
                  <Card header="対応済み脆弱性">
                    <Box sx={{ textAlign: 'center', py: 2 }}>
                      <Typography variant="h3" sx={{ color: 'primary.main', fontWeight: 700 }}>
                        23件
                      </Typography>
                      <Typography variant="body2" color="text.secondary">
                        今月の修正数
                      </Typography>
                    </Box>
                  </Card>
                </Grid>
              </Grid>
            </Box>
          </TabPanel>
        </MuiCard>

        {/* Scan Dialog */}
        <Dialog open={scanDialogOpen} onClose={() => setScanDialogOpen(false)} maxWidth="sm" fullWidth>
          <DialogTitle sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Shield size={20} />
            セキュリティスキャン
          </DialogTitle>
          <DialogContent>
            <TextField
              fullWidth
              label="スキャン対象"
              value={scanTarget}
              onChange={(e) => setScanTarget(e.target.value)}
              placeholder={scanType === 'code' ? './src' : scanType === 'dependency' ? './package.json' : './'}
              sx={{ mt: 2 }}
            />
            <Alert severity="info" sx={{ mt: 2 }}>
              {scanType === 'code' && 'ソースコードのセキュリティ脆弱性をスキャンします。'}
              {scanType === 'dependency' && '依存関係の既知の脆弱性をチェックします。'}
              {scanType === 'secrets' && 'ハードコードされたシークレットやAPIキーを検出します。'}
            </Alert>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setScanDialogOpen(false)}>キャンセル</Button>
            <Button
              onClick={handleScan}
              variant="contained"
              disabled={!scanTarget.trim() || isScanning}
              startIcon={isScanning ? <LinearProgress size={16} /> : <Play />}
            >
              {isScanning ? 'スキャン中...' : 'スキャン開始'}
            </Button>
          </DialogActions>
        </Dialog>
      </Box>
    </DashboardLayout>
  );
}
