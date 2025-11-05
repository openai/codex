'use client';

import React, { useState } from 'react';
import {
  Box,
  Typography,
  Paper,
  TextField,
  Button,
  Grid,
  Card as MuiCard,
  CardContent,
  Chip,
  Avatar,
  List,
  ListItem,
  ListItemText,
  ListItemAvatar,
  Divider,
  Alert,
  LinearProgress,
  Accordion,
  AccordionSummary,
  AccordionDetails,
  Tooltip,
  IconButton,
  CircularProgress,
} from '@mui/material';
import {
  Search,
  ChevronDown,
  ExternalLink,
  Clock,
  CheckCircle,
  AlertCircle,
  Globe,
  BookOpen,
  TrendingUp,
} from 'lucide-react';
import { DashboardLayout } from '@/components/templates/DashboardLayout';
import { Card } from '@/components/atoms/Card';
import { useCodex } from '@/lib/context/CodexContext';

interface ResearchSource {
  url: string;
  title: string;
  snippet: string;
  confidence: number;
  publishedAt?: string;
}

export default function ResearchPage() {
  const { state, runResearch } = useCodex();
  const [query, setQuery] = useState('');
  const [isResearching, setIsResearching] = useState(false);

  const handleResearch = async () => {
    if (!query.trim()) return;

    setIsResearching(true);
    try {
      await runResearch(query);
      setQuery('');
    } catch (error) {
      console.error('Research failed:', error);
    } finally {
      setIsResearching(false);
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'searching':
      case 'analyzing':
        return <CircularProgress size={16} />;
      case 'completed':
        return <CheckCircle size={16} color="#4caf50" />;
      case 'failed':
        return <AlertCircle size={16} color="#f44336" />;
      default:
        return <Clock size={16} />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'searching':
      case 'analyzing':
        return 'warning';
      case 'completed':
        return 'success';
      case 'failed':
        return 'error';
      default:
        return 'default';
    }
  };

  const getConfidenceColor = (confidence: number) => {
    if (confidence >= 0.8) return 'success';
    if (confidence >= 0.6) return 'warning';
    return 'error';
  };

  return (
    <DashboardLayout title="Deep Research">
      <Box sx={{ p: 3 }}>
        <Typography variant="h4" sx={{ mb: 2, fontWeight: 700 }}>
          Deep Research
        </Typography>
        <Typography variant="body1" color="text.secondary" sx={{ mb: 4 }}>
          複数のソースから情報を収集・分析し、高品質な研究結果を提供します。技術調査、ベストプラクティス調査などに最適です。
        </Typography>

        {/* Research Input */}
        <Card header={
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Search size={20} />
            <Typography variant="h6">新規研究</Typography>
          </Box>
        }>
          <Box sx={{ display: 'flex', gap: 2, alignItems: 'flex-start' }}>
            <TextField
              fullWidth
              multiline
              rows={3}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="研究したいトピックや質問を入力してください...&#10;例: React Server Components のベストプラクティス&#10;例: Rust の非同期プログラミングパターン"
              sx={{ flex: 1 }}
            />
            <Button
              variant="contained"
              onClick={handleResearch}
              disabled={!query.trim() || isResearching}
              sx={{
                minWidth: 120,
                height: 56,
                background: 'linear-gradient(45deg, #7b1fa2, #9c27b0)',
                '&:hover': {
                  background: 'linear-gradient(45deg, #4a148c, #7b1fa2)',
                },
              }}
              startIcon={isResearching ? <CircularProgress size={16} /> : <Search />}
            >
              {isResearching ? '検索中...' : '研究開始'}
            </Button>
          </Box>

          <Box sx={{ mt: 2, display: 'flex', gap: 1, flexWrap: 'wrap' }}>
            <Chip label="技術調査" variant="outlined" size="small" />
            <Chip label="ベストプラクティス" variant="outlined" size="small" />
            <Chip label="パフォーマンス" variant="outlined" size="small" />
            <Chip label="セキュリティ" variant="outlined" size="small" />
            <Chip label="アーキテクチャ" variant="outlined" size="small" />
          </Box>
        </Card>

        {state.error && (
          <Alert severity="error" sx={{ mt: 3 }}>
            {state.error}
          </Alert>
        )}

        {/* Research Results */}
        <Box sx={{ mt: 4 }}>
          <Typography variant="h5" sx={{ mb: 3, fontWeight: 600 }}>
            研究履歴
          </Typography>

          {state.researchResults.length === 0 ? (
            <Paper sx={{ p: 4, textAlign: 'center', color: 'text.secondary' }}>
              <BookOpen size={48} style={{ marginBottom: 16, opacity: 0.5 }} />
              <Typography variant="h6" gutterBottom>
                まだ研究結果がありません
              </Typography>
              <Typography variant="body2">
                上のフォームから新しい研究を開始してください
              </Typography>
            </Paper>
          ) : (
            <Grid container spacing={3}>
              {state.researchResults.map((result) => (
                <Grid item xs={12} key={result.id}>
                  <MuiCard>
                    <CardContent>
                      <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                        {getStatusIcon(result.status)}
                        <Typography variant="h6" sx={{ ml: 1, flex: 1 }}>
                          {result.query}
                        </Typography>
                        <Chip
                          label={result.status === 'searching' ? '検索中' :
                                result.status === 'analyzing' ? '分析中' :
                                result.status === 'completed' ? '完了' : '失敗'}
                          color={getStatusColor(result.status)}
                          size="small"
                        />
                      </Box>

                      <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap' }}>
                        <Typography variant="caption" color="text.secondary">
                          開始: {new Date(result.startedAt).toLocaleString('ja-JP')}
                        </Typography>
                        {result.completedAt && (
                          <Typography variant="caption" color="text.secondary">
                            完了: {new Date(result.completedAt).toLocaleString('ja-JP')}
                          </Typography>
                        )}
                        <Typography variant="caption" color="text.secondary">
                          ソース数: {result.sources?.length || 0}
                        </Typography>
                      </Box>

                      {result.status === 'searching' || result.status === 'analyzing' ? (
                        <Box sx={{ mb: 2 }}>
                          <LinearProgress />
                          <Typography variant="caption" color="text.secondary" sx={{ mt: 1, display: 'block' }}>
                            {result.status === 'searching' ? '情報を収集中...' : '分析中...'}
                          </Typography>
                        </Box>
                      ) : null}

                      {result.summary && (
                        <Alert severity="info" sx={{ mb: 2 }}>
                          <Typography variant="body2">
                            {result.summary}
                          </Typography>
                        </Alert>
                      )}

                      {result.sources && result.sources.length > 0 && (
                        <Accordion>
                          <AccordionSummary expandIcon={<ChevronDown />}>
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                              <Globe size={16} />
                              <Typography variant="subtitle1">
                                情報源 ({result.sources.length}件)
                              </Typography>
                            </Box>
                          </AccordionSummary>
                          <AccordionDetails sx={{ p: 0 }}>
                            <List dense>
                              {result.sources.map((source, index) => (
                                <React.Fragment key={index}>
                                  <ListItem alignItems="flex-start">
                                    <ListItemAvatar>
                                      <Avatar sx={{ bgcolor: `${getConfidenceColor(source.confidence)}.main` }}>
                                        <TrendingUp size={16} />
                                      </Avatar>
                                    </ListItemAvatar>
                                    <ListItemText
                                      primary={
                                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                                          <Typography variant="subtitle2" sx={{ flex: 1 }}>
                                            {source.title}
                                          </Typography>
                                          <Chip
                                            label={`${Math.round(source.confidence * 100)}%`}
                                            size="small"
                                            color={getConfidenceColor(source.confidence)}
                                            variant="outlined"
                                          />
                                          <Tooltip title="リンクを開く">
                                            <IconButton
                                              size="small"
                                              onClick={() => window.open(source.url, '_blank')}
                                            >
                                              <ExternalLink size={14} />
                                            </IconButton>
                                          </Tooltip>
                                        </Box>
                                      }
                                      secondary={
                                        <Box>
                                          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
                                            {source.snippet}
                                          </Typography>
                                          <Typography variant="caption" color="text.secondary">
                                            {source.url}
                                            {source.publishedAt && ` • ${new Date(source.publishedAt).toLocaleDateString('ja-JP')}`}
                                          </Typography>
                                        </Box>
                                      }
                                    />
                                  </ListItem>
                                  {index < result.sources.length - 1 && <Divider component="li" />}
                                </React.Fragment>
                              ))}
                            </List>
                          </AccordionDetails>
                        </Accordion>
                      )}
                    </CardContent>
                  </MuiCard>
                </Grid>
              ))}
            </Grid>
          )}
        </Box>

        {/* Research Tips */}
        <Card header="研究のヒント" sx={{ mt: 4 }}>
          <Grid container spacing={2}>
            <Grid item xs={12} md={6}>
              <Typography variant="subtitle1" sx={{ mb: 1, fontWeight: 600 }}>
                効果的なクエリ例
              </Typography>
              <Box component="ul" sx={{ pl: 2, m: 0 }}>
                <li>具体的な技術やフレームワーク名を含む</li>
                <li>比較や選択肢を尋ねる</li>
                <li>ベストプラクティスを求める</li>
                <li>パフォーマンスやセキュリティに関する質問</li>
              </Box>
            </Grid>
            <Grid item xs={12} md={6}>
              <Typography variant="subtitle1" sx={{ mb: 1, fontWeight: 600 }}>
                研究結果の活用
              </Typography>
              <Box component="ul" sx={{ pl: 2, m: 0 }}>
                <li>信頼性の高いソースを優先</li>
                <li>最新の情報を確認</li>
                <li>複数の視点から検討</li>
                <li>実装前に検証</li>
              </Box>
            </Grid>
          </Grid>
        </Card>
      </Box>
    </DashboardLayout>
  );
}
