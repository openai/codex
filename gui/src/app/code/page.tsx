'use client';

import React, { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  Paper,
  TextField,
  Button,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
  Chip,
  Alert,
  CircularProgress,
  Grid,
  Divider,
  IconButton,
  Tooltip,
} from '@mui/material';
import {
  Play,
  Stop,
  Save,
  FolderOpen,
  Terminal,
  Code as CodeIcon,
  Zap,
} from 'lucide-react';
import { DashboardLayout } from '@/components/templates/DashboardLayout';
import { Card } from '@/components/atoms/Card';
import { useCodex } from '@/lib/context/CodexContext';

interface CodeExecutionResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  executionTime: number;
}

const SUPPORTED_LANGUAGES = [
  { value: 'javascript', label: 'JavaScript', extension: 'js' },
  { value: 'typescript', label: 'TypeScript', extension: 'ts' },
  { value: 'python', label: 'Python', extension: 'py' },
  { value: 'rust', label: 'Rust', extension: 'rs' },
  { value: 'go', label: 'Go', extension: 'go' },
  { value: 'bash', label: 'Bash', extension: 'sh' },
  { value: 'powershell', label: 'PowerShell', extension: 'ps1' },
];

const CODE_TEMPLATES = {
  javascript: `// JavaScript Code Example
function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

console.log('Fibonacci of 10:', fibonacci(10));`,
  typescript: `// TypeScript Code Example
interface User {
  id: number;
  name: string;
  email: string;
}

function createUser(id: number, name: string, email: string): User {
  return { id, name, email };
}

const user = createUser(1, 'John Doe', 'john@example.com');
console.log('Created user:', user);`,
  python: `# Python Code Example
def fibonacci(n: int) -> int:
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

if __name__ == "__main__":
    result = fibonacci(10)
    print(f"Fibonacci of 10: {result}")`,
  rust: `// Rust Code Example
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn main() {
    let result = fibonacci(10);
    println!("Fibonacci of 10: {}", result);
}`,
  go: `// Go Code Example
package main

import "fmt"

func fibonacci(n int) int {
    if n <= 1 {
        return n
    }
    return fibonacci(n-1) + fibonacci(n-2)
}

func main() {
    result := fibonacci(10)
    fmt.Printf("Fibonacci of 10: %d\\n", result)
}`,
  bash: `#!/bin/bash

# Bash Script Example
echo "Current directory: $(pwd)"
echo "Files in directory:"
ls -la

# Simple loop
for i in {1..5}; do
    echo "Count: $i"
done`,
  powershell: `# PowerShell Script Example
Write-Host "Current directory: $(Get-Location)"
Write-Host "Files in directory:"
Get-ChildItem -Force

# Simple loop
for ($i = 1; $i -le 5; $i++) {
    Write-Host "Count: $i"
}`,
};

export default function CodeExecutionPage() {
  const { executeCommand, state } = useCodex();
  const [code, setCode] = useState(CODE_TEMPLATES.javascript);
  const [language, setLanguage] = useState('javascript');
  const [filename, setFilename] = useState('script.js');
  const [isExecuting, setIsExecuting] = useState(false);
  const [result, setResult] = useState<CodeExecutionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [savedFiles, setSavedFiles] = useState<string[]>([]);

  // Update filename when language changes
  useEffect(() => {
    const lang = SUPPORTED_LANGUAGES.find(l => l.value === language);
    if (lang) {
      setFilename(`script.${lang.extension}`);
    }
  }, [language]);

  const handleLanguageChange = (newLanguage: string) => {
    setLanguage(newLanguage);
    setCode(CODE_TEMPLATES[newLanguage as keyof typeof CODE_TEMPLATES] || '');
    setResult(null);
    setError(null);
  };

  const handleExecute = async () => {
    if (!code.trim()) {
      setError('コードを入力してください');
      return;
    }

    setIsExecuting(true);
    setResult(null);
    setError(null);

    const startTime = Date.now();

    try {
      let command: string;
      let args: string[] = [];

      // Create temporary file and execute based on language
      switch (language) {
        case 'javascript':
          command = `node -e "${code.replace(/"/g, '\\"')}"`;
          break;
        case 'typescript':
          command = `npx ts-node -e "${code.replace(/"/g, '\\"')}"`;
          break;
        case 'python':
          command = `python3 -c "${code.replace(/"/g, '\\"')}"`;
          break;
        case 'rust':
          // For Rust, we'd need to compile first, but for simplicity:
          command = `echo "${code}" > /tmp/temp.rs && rustc /tmp/temp.rs -o /tmp/temp && /tmp/temp`;
          break;
        case 'go':
          command = `echo "${code}" > /tmp/temp.go && go run /tmp/temp.go`;
          break;
        case 'bash':
          command = `bash -c "${code.replace(/"/g, '\\"')}"`;
          break;
        case 'powershell':
          command = `powershell -Command "${code.replace(/"/g, '\\"')}"`;
          break;
        default:
          throw new Error(`Unsupported language: ${language}`);
      }

      const execResult = await executeCommand(command);

      const executionTime = Date.now() - startTime;

      setResult({
        exitCode: execResult.exitCode,
        stdout: execResult.stdout,
        stderr: execResult.stderr,
        executionTime,
      });

    } catch (err) {
      setError(err instanceof Error ? err.message : '実行中にエラーが発生しました');
    } finally {
      setIsExecuting(false);
    }
  };

  const handleSave = async () => {
    try {
      // Save code to a file
      const saveCommand = `echo "${code.replace(/"/g, '\\"')}" > ${filename}`;
      await executeCommand(saveCommand);
      setSavedFiles(prev => [...prev, filename]);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存中にエラーが発生しました');
    }
  };

  const handleLoad = async () => {
    try {
      const loadCommand = `cat ${filename}`;
      const loadResult = await executeCommand(loadCommand);
      if (loadResult.exitCode === 0) {
        setCode(loadResult.stdout);
        setError(null);
      } else {
        setError('ファイルの読み込みに失敗しました');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'ファイルの読み込み中にエラーが発生しました');
    }
  };

  return (
    <DashboardLayout title="コード実行">
      <Box sx={{ height: 'calc(100vh - 200px)', display: 'flex', flexDirection: 'column', gap: 2 }}>

        {/* Controls */}
        <Card header="コード実行設定">
          <Grid container spacing={2} alignItems="center">
            <Grid item xs={12} md={3}>
              <FormControl fullWidth size="small">
                <InputLabel>言語</InputLabel>
                <Select
                  value={language}
                  label="言語"
                  onChange={(e) => handleLanguageChange(e.target.value)}
                >
                  {SUPPORTED_LANGUAGES.map((lang) => (
                    <MenuItem key={lang.value} value={lang.value}>
                      {lang.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
            </Grid>

            <Grid item xs={12} md={3}>
              <TextField
                fullWidth
                size="small"
                label="ファイル名"
                value={filename}
                onChange={(e) => setFilename(e.target.value)}
                placeholder={`script.${SUPPORTED_LANGUAGES.find(l => l.value === language)?.extension}`}
              />
            </Grid>

            <Grid item xs={12} md={6}>
              <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                <Button
                  variant="contained"
                  startIcon={isExecuting ? <CircularProgress size={16} /> : <Play />}
                  onClick={handleExecute}
                  disabled={isExecuting || !state.isConnected}
                  sx={{
                    background: 'linear-gradient(45deg, #0061a4, #1976d2)',
                    '&:hover': {
                      background: 'linear-gradient(45deg, #004d8f, #1565c0)',
                    },
                  }}
                >
                  {isExecuting ? '実行中...' : '実行'}
                </Button>

                <Button
                  variant="outlined"
                  startIcon={<Save />}
                  onClick={handleSave}
                  disabled={!code.trim()}
                >
                  保存
                </Button>

                <Button
                  variant="outlined"
                  startIcon={<FolderOpen />}
                  onClick={handleLoad}
                >
                  読み込み
                </Button>

                <Tooltip title="クイック実行">
                  <IconButton
                    color="secondary"
                    onClick={handleExecute}
                    disabled={isExecuting || !state.isConnected}
                  >
                    <Zap size={20} />
                  </IconButton>
                </Tooltip>
              </Box>
            </Grid>
          </Grid>

          {!state.isConnected && (
            <Alert severity="warning" sx={{ mt: 2 }}>
              Codexサーバーに接続されていません。コード実行にはサーバー接続が必要です。
            </Alert>
          )}
        </Card>

        {/* Code Editor */}
        <Card header={
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <CodeIcon size={20} />
            <Typography variant="h6">コードエディタ</Typography>
            <Chip
              label={SUPPORTED_LANGUAGES.find(l => l.value === language)?.label}
              size="small"
              color="primary"
              variant="outlined"
            />
          </Box>
        }>
          <TextField
            fullWidth
            multiline
            minRows={15}
            maxRows={25}
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="ここにコードを入力してください..."
            sx={{
              '& .MuiInputBase-root': {
                fontFamily: 'monospace',
                fontSize: '14px',
                lineHeight: 1.5,
              },
            }}
          />
        </Card>

        {/* Results */}
        {error && (
          <Alert severity="error" sx={{ mb: 2 }}>
            {error}
          </Alert>
        )}

        {result && (
          <Card header={
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
              <Terminal size={20} />
              <Typography variant="h6">実行結果</Typography>
              <Chip
                label={`終了コード: ${result.exitCode}`}
                size="small"
                color={result.exitCode === 0 ? 'success' : 'error'}
              />
              <Chip
                label={`${result.executionTime}ms`}
                size="small"
                variant="outlined"
              />
            </Box>
          }>
            {result.stdout && (
              <Box sx={{ mb: result.stderr ? 2 : 0 }}>
                <Typography variant="subtitle2" sx={{ mb: 1, color: 'success.main' }}>
                  標準出力:
                </Typography>
                <Paper
                  sx={{
                    p: 2,
                    backgroundColor: 'grey.900',
                    color: 'grey.100',
                    fontFamily: 'monospace',
                    fontSize: '14px',
                    maxHeight: '300px',
                    overflow: 'auto',
                  }}
                >
                  <pre style={{ margin: 0, whiteSpace: 'pre-wrap' }}>
                    {result.stdout}
                  </pre>
                </Paper>
              </Box>
            )}

            {result.stderr && (
              <Box>
                <Typography variant="subtitle2" sx={{ mb: 1, color: 'error.main' }}>
                  標準エラー出力:
                </Typography>
                <Paper
                  sx={{
                    p: 2,
                    backgroundColor: 'error.dark',
                    color: 'error.contrastText',
                    fontFamily: 'monospace',
                    fontSize: '14px',
                    maxHeight: '300px',
                    overflow: 'auto',
                  }}
                >
                  <pre style={{ margin: 0, whiteSpace: 'pre-wrap' }}>
                    {result.stderr}
                  </pre>
                </Paper>
              </Box>
            )}
          </Card>
        )}

        {/* Saved Files */}
        {savedFiles.length > 0 && (
          <Card header="保存されたファイル">
            <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
              {savedFiles.map((file) => (
                <Chip
                  key={file}
                  label={file}
                  variant="outlined"
                  onClick={() => setFilename(file)}
                />
              ))}
            </Box>
          </Card>
        )}
      </Box>
    </DashboardLayout>
  );
}
