# Day 3 TODO - 파일 작업 및 도구 호출 UI

## 목표
파일 시스템 탐색, 파일 뷰어, 도구 호출 시각화 등 Codex의 핵심 기능을 웹 UI에 통합합니다.

---

## 1. 파일 탐색기 UI (Commit 13)

### 요구사항
- 트리 구조 파일 탐색기
- 폴더 확장/축소
- 파일 아이콘 및 메타데이터 표시
- 파일 선택 및 열기

### 작업 내용

#### 파일 타입 정의
- [ ] `src/types/file.ts` 파일 생성
  ```typescript
  export enum FileType {
    FILE = 'file',
    DIRECTORY = 'directory',
    SYMLINK = 'symlink',
  }

  export interface FileNode {
    id: string;
    name: string;
    path: string;
    type: FileType;
    size?: number;
    modified?: number;
    children?: FileNode[];
    isExpanded?: boolean;
    isLoading?: boolean;
    extension?: string;
    gitStatus?: 'modified' | 'added' | 'deleted' | 'untracked';
  }

  export interface FileTreeState {
    root: FileNode[];
    selectedPath: string | null;
    expandedPaths: Set<string>;
  }
  ```

#### 파일 스토어 구현
- [ ] `src/store/file-store.ts` 생성
  ```typescript
  import { create } from 'zustand';
  import { FileNode, FileTreeState } from '@/types/file';

  interface FileState extends FileTreeState {
    // Actions
    setRoot: (nodes: FileNode[]) => void;
    selectFile: (path: string) => void;
    toggleExpanded: (path: string) => void;
    updateNode: (path: string, updates: Partial<FileNode>) => void;
    addNodes: (parentPath: string, nodes: FileNode[]) => void;
    refreshTree: () => Promise<void>;
  }

  export const useFileStore = create<FileState>((set, get) => ({
    root: [],
    selectedPath: null,
    expandedPaths: new Set(),

    setRoot: (nodes) => {
      set({ root: nodes });
    },

    selectFile: (path) => {
      set({ selectedPath: path });
    },

    toggleExpanded: (path) => {
      set((state) => {
        const expandedPaths = new Set(state.expandedPaths);
        if (expandedPaths.has(path)) {
          expandedPaths.delete(path);
        } else {
          expandedPaths.add(path);
        }
        return { expandedPaths };
      });
    },

    updateNode: (path, updates) => {
      const updateInTree = (nodes: FileNode[]): FileNode[] => {
        return nodes.map((node) => {
          if (node.path === path) {
            return { ...node, ...updates };
          }
          if (node.children) {
            return { ...node, children: updateInTree(node.children) };
          }
          return node;
        });
      };

      set((state) => ({
        root: updateInTree(state.root),
      }));
    },

    addNodes: (parentPath, nodes) => {
      const addToTree = (treeNodes: FileNode[]): FileNode[] => {
        return treeNodes.map((node) => {
          if (node.path === parentPath) {
            return { ...node, children: nodes, isLoading: false };
          }
          if (node.children) {
            return { ...node, children: addToTree(node.children) };
          }
          return node;
        });
      };

      set((state) => ({
        root: addToTree(state.root),
      }));
    },

    refreshTree: async () => {
      // API 호출로 파일 트리 새로고침
      try {
        const response = await apiClient.get('/files/tree');
        set({ root: response.data });
      } catch (error) {
        console.error('Failed to refresh file tree', error);
      }
    },
  }));
  ```

#### FileExplorer 컴포넌트
- [ ] `src/components/files/FileExplorer.tsx` 생성
  ```typescript
  import { useEffect } from 'react';
  import { ScrollArea } from '@/components/ui/scroll-area';
  import { useFileStore } from '@/store/file-store';
  import { FileTreeNode } from './FileTreeNode';
  import { Button } from '@/components/ui/button';
  import { RefreshCw, FolderOpen } from 'lucide-react';

  export function FileExplorer() {
    const { root, refreshTree } = useFileStore();
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
      loadInitialTree();
    }, []);

    const loadInitialTree = async () => {
      setIsLoading(true);
      await refreshTree();
      setIsLoading(false);
    };

    return (
      <div className="flex flex-col h-full border-r bg-muted/20">
        <div className="flex items-center justify-between p-3 border-b">
          <div className="flex items-center gap-2">
            <FolderOpen className="w-4 h-4" />
            <span className="font-semibold text-sm">Files</span>
          </div>
          <Button
            size="icon"
            variant="ghost"
            onClick={loadInitialTree}
            disabled={isLoading}
            className="h-7 w-7"
          >
            <RefreshCw className={cn('w-4 h-4', isLoading && 'animate-spin')} />
          </Button>
        </div>

        <ScrollArea className="flex-1">
          <div className="p-2">
            {isLoading && root.length === 0 ? (
              <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
                Loading files...
              </div>
            ) : root.length === 0 ? (
              <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
                No files found
              </div>
            ) : (
              root.map((node) => (
                <FileTreeNode key={node.id} node={node} level={0} />
              ))
            )}
          </div>
        </ScrollArea>
      </div>
    );
  }
  ```

#### FileTreeNode 컴포넌트
- [ ] `src/components/files/FileTreeNode.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { FileNode, FileType } from '@/types/file';
  import { useFileStore } from '@/store/file-store';
  import { getFileIcon } from '@/lib/file-icons';
  import { cn } from '@/lib/utils';
  import { ChevronRight, ChevronDown, Loader2 } from 'lucide-react';

  interface FileTreeNodeProps {
    node: FileNode;
    level: number;
  }

  export function FileTreeNode({ node, level }: FileTreeNodeProps) {
    const {
      selectedPath,
      expandedPaths,
      selectFile,
      toggleExpanded,
      updateNode,
      addNodes,
    } = useFileStore();

    const isDirectory = node.type === FileType.DIRECTORY;
    const isExpanded = expandedPaths.has(node.path);
    const isSelected = selectedPath === node.path;

    const handleClick = async () => {
      if (isDirectory) {
        toggleExpanded(node.path);

        // 자식 노드가 없으면 로드
        if (!node.children && !node.isLoading) {
          updateNode(node.path, { isLoading: true });
          try {
            const response = await apiClient.get(`/files/tree`, {
              params: { path: node.path },
            });
            addNodes(node.path, response.data);
          } catch (error) {
            console.error('Failed to load directory', error);
            updateNode(node.path, { isLoading: false });
          }
        }
      } else {
        selectFile(node.path);
      }
    };

    const FileIcon = getFileIcon(node.name, node.type);

    return (
      <div>
        <div
          className={cn(
            'flex items-center gap-1 px-2 py-1 rounded cursor-pointer hover:bg-accent',
            isSelected && 'bg-accent',
            'group'
          )}
          style={{ paddingLeft: `${level * 12 + 8}px` }}
          onClick={handleClick}
        >
          {isDirectory && (
            <span className="flex-shrink-0">
              {node.isLoading ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : isExpanded ? (
                <ChevronDown className="w-4 h-4" />
              ) : (
                <ChevronRight className="w-4 h-4" />
              )}
            </span>
          )}
          {!isDirectory && <span className="w-4" />}

          <FileIcon className="w-4 h-4 flex-shrink-0 text-muted-foreground" />

          <span className={cn('text-sm truncate', isSelected && 'font-medium')}>
            {node.name}
          </span>

          {node.gitStatus && (
            <span
              className={cn(
                'ml-auto text-xs px-1.5 py-0.5 rounded',
                node.gitStatus === 'modified' && 'text-yellow-600 bg-yellow-50',
                node.gitStatus === 'added' && 'text-green-600 bg-green-50',
                node.gitStatus === 'deleted' && 'text-red-600 bg-red-50',
                node.gitStatus === 'untracked' && 'text-blue-600 bg-blue-50'
              )}
            >
              {node.gitStatus[0].toUpperCase()}
            </span>
          )}
        </div>

        {isDirectory && isExpanded && node.children && (
          <div>
            {node.children.map((child) => (
              <FileTreeNode key={child.id} node={child} level={level + 1} />
            ))}
          </div>
        )}
      </div>
    );
  }
  ```

#### 파일 아이콘 유틸리티
- [ ] `src/lib/file-icons.ts` 생성
  ```typescript
  import {
    FileText,
    FileCode,
    FileJson,
    Image,
    File,
    Folder,
    FolderOpen,
  } from 'lucide-react';
  import { FileType } from '@/types/file';

  const extensionIcons: Record<string, any> = {
    // Code files
    js: FileCode,
    jsx: FileCode,
    ts: FileCode,
    tsx: FileCode,
    py: FileCode,
    go: FileCode,
    rs: FileCode,
    java: FileCode,
    cpp: FileCode,
    c: FileCode,

    // Config files
    json: FileJson,
    yaml: FileJson,
    yml: FileJson,
    toml: FileJson,
    xml: FileJson,

    // Text files
    md: FileText,
    txt: FileText,
    log: FileText,

    // Images
    png: Image,
    jpg: Image,
    jpeg: Image,
    gif: Image,
    svg: Image,
    webp: Image,
  };

  export function getFileIcon(name: string, type: FileType) {
    if (type === FileType.DIRECTORY) {
      return Folder;
    }

    const extension = name.split('.').pop()?.toLowerCase();
    if (extension && extensionIcons[extension]) {
      return extensionIcons[extension];
    }

    return File;
  }

  export function getFileLanguage(filename: string): string {
    const extension = filename.split('.').pop()?.toLowerCase();

    const languageMap: Record<string, string> = {
      js: 'javascript',
      jsx: 'javascript',
      ts: 'typescript',
      tsx: 'typescript',
      py: 'python',
      rs: 'rust',
      go: 'go',
      java: 'java',
      cpp: 'cpp',
      c: 'c',
      rb: 'ruby',
      php: 'php',
      html: 'html',
      css: 'css',
      scss: 'scss',
      json: 'json',
      yaml: 'yaml',
      yml: 'yaml',
      md: 'markdown',
      sh: 'bash',
    };

    return languageMap[extension || ''] || 'plaintext';
  }
  ```

### 예상 결과물
- 트리 구조 파일 탐색기
- 폴더 확장/축소 애니메이션
- 파일 아이콘 표시
- Git 상태 표시

### Commit 메시지
```
feat(web-ui): implement file explorer component

- Define file types and tree structure
- Create file store with Zustand
- Build FileExplorer with tree view
- Implement FileTreeNode with lazy loading
- Add file icons and git status indicators
- Support folder expansion and file selection
```

---

## 2. 파일 뷰어 (Commit 14)

### 요구사항
- 코드 에디터 통합
- 문법 하이라이팅
- 읽기 전용 모드
- 여러 파일 탭 지원

### 작업 내용

#### Monaco Editor 설치
- [ ] Monaco Editor 설치
  ```bash
  pnpm add @monaco-editor/react monaco-editor
  ```

#### FileViewer 컴포넌트
- [ ] `src/components/files/FileViewer.tsx` 생성
  ```typescript
  import { useEffect, useState } from 'react';
  import Editor from '@monaco-editor/react';
  import { useFileStore } from '@/store/file-store';
  import { apiClient } from '@/lib/api-client';
  import { getFileLanguage } from '@/lib/file-icons';
  import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
  import { Button } from '@/components/ui/button';
  import { X, Download, Copy } from 'lucide-react';
  import { toast } from '@/lib/toast';

  interface OpenFile {
    path: string;
    name: string;
    content: string;
    language: string;
  }

  export function FileViewer() {
    const [openFiles, setOpenFiles] = useState<OpenFile[]>([]);
    const [activeFile, setActiveFile] = useState<string | null>(null);
    const { selectedPath } = useFileStore();

    useEffect(() => {
      if (selectedPath) {
        loadFile(selectedPath);
      }
    }, [selectedPath]);

    const loadFile = async (path: string) => {
      // 이미 열린 파일인지 확인
      const existing = openFiles.find((f) => f.path === path);
      if (existing) {
        setActiveFile(path);
        return;
      }

      try {
        const response = await apiClient.get(`/files/content`, {
          params: { path },
        });

        const fileName = path.split('/').pop() || path;
        const language = getFileLanguage(fileName);

        const newFile: OpenFile = {
          path,
          name: fileName,
          content: response.data.content,
          language,
        };

        setOpenFiles((prev) => [...prev, newFile]);
        setActiveFile(path);
      } catch (error) {
        console.error('Failed to load file', error);
        toast.error('Failed to load file');
      }
    };

    const closeFile = (path: string) => {
      setOpenFiles((prev) => prev.filter((f) => f.path !== path));
      if (activeFile === path) {
        const remaining = openFiles.filter((f) => f.path !== path);
        setActiveFile(remaining.length > 0 ? remaining[0].path : null);
      }
    };

    const handleCopy = async () => {
      const file = openFiles.find((f) => f.path === activeFile);
      if (file) {
        await navigator.clipboard.writeText(file.content);
        toast.success('Copied to clipboard');
      }
    };

    const handleDownload = () => {
      const file = openFiles.find((f) => f.path === activeFile);
      if (file) {
        const blob = new Blob([file.content], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = file.name;
        a.click();
        URL.revokeObjectURL(url);
        toast.success('File downloaded');
      }
    };

    if (openFiles.length === 0) {
      return (
        <div className="flex items-center justify-center h-full text-muted-foreground">
          <div className="text-center">
            <FileText className="w-16 h-16 mx-auto mb-4 opacity-50" />
            <p>Select a file to view</p>
          </div>
        </div>
      );
    }

    return (
      <div className="flex flex-col h-full">
        <Tabs value={activeFile || ''} onValueChange={setActiveFile}>
          <div className="flex items-center justify-between border-b">
            <TabsList className="h-10 bg-transparent">
              {openFiles.map((file) => (
                <TabsTrigger
                  key={file.path}
                  value={file.path}
                  className="relative pr-8"
                >
                  {file.name}
                  <Button
                    size="icon"
                    variant="ghost"
                    className="absolute right-0 h-6 w-6"
                    onClick={(e) => {
                      e.stopPropagation();
                      closeFile(file.path);
                    }}
                  >
                    <X className="h-3 w-3" />
                  </Button>
                </TabsTrigger>
              ))}
            </TabsList>

            <div className="flex items-center gap-2 px-4">
              <Button size="sm" variant="ghost" onClick={handleCopy}>
                <Copy className="w-4 h-4 mr-2" />
                Copy
              </Button>
              <Button size="sm" variant="ghost" onClick={handleDownload}>
                <Download className="w-4 h-4 mr-2" />
                Download
              </Button>
            </div>
          </div>

          {openFiles.map((file) => (
            <TabsContent
              key={file.path}
              value={file.path}
              className="flex-1 m-0"
            >
              <Editor
                height="100%"
                language={file.language}
                value={file.content}
                theme="vs-dark"
                options={{
                  readOnly: true,
                  minimap: { enabled: true },
                  fontSize: 14,
                  lineNumbers: 'on',
                  scrollBeyondLastLine: false,
                  automaticLayout: true,
                }}
              />
            </TabsContent>
          ))}
        </Tabs>
      </div>
    );
  }
  ```

#### 파일 메타데이터 패널
- [ ] `src/components/files/FileMetadata.tsx` 생성
  ```typescript
  import { FileNode } from '@/types/file';
  import { formatBytes, formatDate } from '@/lib/format-utils';
  import { Card } from '@/components/ui/card';

  interface FileMetadataProps {
    file: FileNode;
  }

  export function FileMetadata({ file }: FileMetadataProps) {
    return (
      <Card className="p-4 space-y-2">
        <h3 className="font-semibold text-sm">File Information</h3>

        <div className="space-y-1 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Name:</span>
            <span className="font-mono">{file.name}</span>
          </div>

          <div className="flex justify-between">
            <span className="text-muted-foreground">Path:</span>
            <span className="font-mono text-xs truncate max-w-[200px]">
              {file.path}
            </span>
          </div>

          {file.size && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Size:</span>
              <span>{formatBytes(file.size)}</span>
            </div>
          )}

          {file.modified && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Modified:</span>
              <span>{formatDate(file.modified)}</span>
            </div>
          )}

          {file.extension && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Type:</span>
              <span className="uppercase">{file.extension}</span>
            </div>
          )}
        </div>
      </Card>
    );
  }
  ```

#### 포맷 유틸리티
- [ ] `src/lib/format-utils.ts` 생성
  ```typescript
  export function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 Bytes';

    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  }

  export function formatDate(timestamp: number): string {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 86400000) {
      // Less than 1 day
      return date.toLocaleTimeString();
    }

    return date.toLocaleDateString();
  }

  export function formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    if (ms < 3600000) return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
    return `${Math.floor(ms / 3600000)}h ${Math.floor((ms % 3600000) / 60000)}m`;
  }
  ```

### 예상 결과물
- Monaco Editor 통합
- 여러 파일 탭 지원
- 파일 메타데이터 표시
- 복사/다운로드 기능

### Commit 메시지
```
feat(web-ui): add file viewer with syntax highlighting

- Integrate Monaco Editor for code viewing
- Support multiple file tabs
- Add file metadata panel
- Implement copy and download functionality
- Create format utility functions
```

---

## 3. 파일 업로드/다운로드 (Commit 15)

### 요구사항
- 드래그 앤 드롭 파일 업로드
- 멀티 파일 업로드
- 업로드 진행률 표시
- 파일 다운로드

### 작업 내용

#### FileUpload 컴포넌트
- [ ] `src/components/files/FileUpload.tsx` 생성
  ```typescript
  import { useCallback, useState } from 'react';
  import { useDropzone } from 'react-dropzone';
  import { Button } from '@/components/ui/button';
  import { Progress } from '@/components/ui/progress';
  import { Upload, X, CheckCircle, AlertCircle } from 'lucide-react';
  import { apiClient } from '@/lib/api-client';
  import { toast } from '@/lib/toast';
  import { cn } from '@/lib/utils';

  interface UploadingFile {
    id: string;
    file: File;
    progress: number;
    status: 'pending' | 'uploading' | 'completed' | 'error';
    error?: string;
  }

  interface FileUploadProps {
    targetPath?: string;
    onUploadComplete?: () => void;
  }

  export function FileUpload({ targetPath = '/', onUploadComplete }: FileUploadProps) {
    const [uploadingFiles, setUploadingFiles] = useState<UploadingFile[]>([]);

    const onDrop = useCallback((acceptedFiles: File[]) => {
      const newFiles: UploadingFile[] = acceptedFiles.map((file) => ({
        id: crypto.randomUUID(),
        file,
        progress: 0,
        status: 'pending',
      }));

      setUploadingFiles((prev) => [...prev, ...newFiles]);

      // 업로드 시작
      newFiles.forEach((uploadingFile) => {
        uploadFile(uploadingFile);
      });
    }, [targetPath]);

    const { getRootProps, getInputProps, isDragActive } = useDropzone({
      onDrop,
      multiple: true,
    });

    const uploadFile = async (uploadingFile: UploadingFile) => {
      const formData = new FormData();
      formData.append('file', uploadingFile.file);
      formData.append('path', targetPath);

      try {
        setUploadingFiles((prev) =>
          prev.map((f) =>
            f.id === uploadingFile.id ? { ...f, status: 'uploading' } : f
          )
        );

        await apiClient.post('/files/upload', formData, {
          headers: {
            'Content-Type': 'multipart/form-data',
          },
          onUploadProgress: (progressEvent) => {
            const progress = progressEvent.total
              ? Math.round((progressEvent.loaded * 100) / progressEvent.total)
              : 0;

            setUploadingFiles((prev) =>
              prev.map((f) =>
                f.id === uploadingFile.id ? { ...f, progress } : f
              )
            );
          },
        });

        setUploadingFiles((prev) =>
          prev.map((f) =>
            f.id === uploadingFile.id
              ? { ...f, status: 'completed', progress: 100 }
              : f
          )
        );

        toast.success(`${uploadingFile.file.name} uploaded successfully`);
        onUploadComplete?.();
      } catch (error: any) {
        console.error('Upload failed', error);
        setUploadingFiles((prev) =>
          prev.map((f) =>
            f.id === uploadingFile.id
              ? {
                  ...f,
                  status: 'error',
                  error: error.message || 'Upload failed',
                }
              : f
          )
        );
        toast.error(`Failed to upload ${uploadingFile.file.name}`);
      }
    };

    const removeFile = (id: string) => {
      setUploadingFiles((prev) => prev.filter((f) => f.id !== id));
    };

    return (
      <div className="space-y-4">
        <div
          {...getRootProps()}
          className={cn(
            'border-2 border-dashed rounded-lg p-8 text-center cursor-pointer transition-colors',
            isDragActive
              ? 'border-primary bg-primary/5'
              : 'border-muted-foreground/25 hover:border-primary/50'
          )}
        >
          <input {...getInputProps()} />
          <Upload className="w-12 h-12 mx-auto mb-4 text-muted-foreground" />
          {isDragActive ? (
            <p className="text-primary">Drop files here...</p>
          ) : (
            <div>
              <p className="mb-2">Drag & drop files here, or click to select</p>
              <p className="text-sm text-muted-foreground">
                Multiple files supported
              </p>
            </div>
          )}
        </div>

        {uploadingFiles.length > 0 && (
          <div className="space-y-2">
            <h4 className="text-sm font-semibold">Uploading Files</h4>
            {uploadingFiles.map((file) => (
              <div
                key={file.id}
                className="flex items-center gap-3 p-3 rounded-lg border bg-card"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-sm font-medium truncate">
                      {file.file.name}
                    </span>
                    <span className="text-xs text-muted-foreground ml-2">
                      {formatBytes(file.file.size)}
                    </span>
                  </div>

                  {file.status === 'uploading' && (
                    <Progress value={file.progress} className="h-1" />
                  )}

                  {file.status === 'error' && (
                    <p className="text-xs text-destructive">{file.error}</p>
                  )}
                </div>

                <div className="flex-shrink-0">
                  {file.status === 'completed' && (
                    <CheckCircle className="w-5 h-5 text-green-500" />
                  )}
                  {file.status === 'error' && (
                    <AlertCircle className="w-5 h-5 text-destructive" />
                  )}
                  {(file.status === 'pending' || file.status === 'uploading') && (
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-6 w-6"
                      onClick={() => removeFile(file.id)}
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }
  ```

#### react-dropzone 설치
- [ ] 드래그 앤 드롭 라이브러리 설치
  ```bash
  pnpm add react-dropzone
  pnpm add -D @types/react-dropzone
  ```

#### shadcn Progress 컴포넌트 설치
- [ ] Progress 컴포넌트 설치
  ```bash
  npx shadcn@latest add progress
  ```

#### FileUploadDialog 컴포넌트
- [ ] `src/components/files/FileUploadDialog.tsx` 생성
  ```typescript
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { FileUpload } from './FileUpload';

  interface FileUploadDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    targetPath?: string;
    onUploadComplete?: () => void;
  }

  export function FileUploadDialog({
    open,
    onOpenChange,
    targetPath,
    onUploadComplete,
  }: FileUploadDialogProps) {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Upload Files</DialogTitle>
            <DialogDescription>
              Upload files to {targetPath || 'the current directory'}
            </DialogDescription>
          </DialogHeader>
          <FileUpload
            targetPath={targetPath}
            onUploadComplete={() => {
              onUploadComplete?.();
              onOpenChange(false);
            }}
          />
        </DialogContent>
      </Dialog>
    );
  }
  ```

### 예상 결과물
- 드래그 앤 드롭 업로드
- 실시간 진행률 표시
- 멀티 파일 업로드
- 에러 처리

### Commit 메시지
```
feat(web-ui): implement file upload and download

- Add FileUpload component with drag and drop
- Integrate react-dropzone
- Show upload progress with Progress component
- Support multiple file uploads
- Add FileUploadDialog wrapper
- Handle upload errors gracefully
```

---

## 4. 도구 호출 시각화 (Commit 16)

### 요구사항
- 도구 실행 상태 표시
- 도구 입력/출력 표시
- 확장/축소 가능한 UI
- 실행 시간 표시

### 작업 내용

#### ToolCallDisplay 컴포넌트
- [ ] `src/components/chat/ToolCallDisplay.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import { ToolCall } from '@/types/message';
  import { Card } from '@/components/ui/card';
  import { Badge } from '@/components/ui/badge';
  import { Button } from '@/components/ui/button';
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '@/components/ui/collapsible';
  import {
    ChevronDown,
    ChevronRight,
    Terminal,
    CheckCircle,
    XCircle,
    Loader2,
  } from 'lucide-react';
  import { cn } from '@/lib/utils';
  import { formatDuration } from '@/lib/format-utils';

  interface ToolCallDisplayProps {
    toolCall: ToolCall;
  }

  export function ToolCallDisplay({ toolCall }: ToolCallDisplayProps) {
    const [isOpen, setIsOpen] = useState(false);

    const statusConfig = {
      pending: {
        icon: Loader2,
        color: 'text-gray-500',
        bgColor: 'bg-gray-100',
        label: 'Pending',
        spin: false,
      },
      running: {
        icon: Loader2,
        color: 'text-blue-500',
        bgColor: 'bg-blue-100',
        label: 'Running',
        spin: true,
      },
      completed: {
        icon: CheckCircle,
        color: 'text-green-500',
        bgColor: 'bg-green-100',
        label: 'Completed',
        spin: false,
      },
      failed: {
        icon: XCircle,
        color: 'text-red-500',
        bgColor: 'bg-red-100',
        label: 'Failed',
        spin: false,
      },
    };

    const config = statusConfig[toolCall.status];
    const Icon = config.icon;
    const duration = toolCall.result
      ? Date.now() - toolCall.timestamp
      : undefined;

    return (
      <Card className="overflow-hidden">
        <Collapsible open={isOpen} onOpenChange={setIsOpen}>
          <div className="flex items-center gap-2 p-3 bg-muted/30">
            <CollapsibleTrigger asChild>
              <Button size="icon" variant="ghost" className="h-6 w-6">
                {isOpen ? (
                  <ChevronDown className="h-4 w-4" />
                ) : (
                  <ChevronRight className="h-4 w-4" />
                )}
              </Button>
            </CollapsibleTrigger>

            <Terminal className="w-4 h-4 text-muted-foreground" />

            <span className="font-mono text-sm font-semibold">
              {toolCall.name}
            </span>

            <Badge
              variant="secondary"
              className={cn('ml-auto', config.bgColor, config.color)}
            >
              <Icon className={cn('w-3 h-3 mr-1', config.spin && 'animate-spin')} />
              {config.label}
            </Badge>

            {duration && (
              <span className="text-xs text-muted-foreground">
                {formatDuration(duration)}
              </span>
            )}
          </div>

          <CollapsibleContent>
            <div className="p-3 space-y-3 border-t">
              {/* Arguments */}
              <div>
                <h4 className="text-xs font-semibold text-muted-foreground mb-1">
                  Arguments
                </h4>
                <pre className="text-xs bg-muted p-2 rounded overflow-x-auto">
                  {JSON.stringify(toolCall.arguments, null, 2)}
                </pre>
              </div>

              {/* Result */}
              {toolCall.result && (
                <div>
                  <h4 className="text-xs font-semibold text-muted-foreground mb-1">
                    Result
                  </h4>
                  <pre className="text-xs bg-muted p-2 rounded overflow-x-auto max-h-40 overflow-y-auto">
                    {toolCall.result}
                  </pre>
                </div>
              )}

              {/* Error */}
              {toolCall.error && (
                <div>
                  <h4 className="text-xs font-semibold text-destructive mb-1">
                    Error
                  </h4>
                  <pre className="text-xs bg-destructive/10 text-destructive p-2 rounded overflow-x-auto">
                    {toolCall.error}
                  </pre>
                </div>
              )}
            </div>
          </CollapsibleContent>
        </Collapsible>
      </Card>
    );
  }
  ```

#### shadcn Collapsible 설치
- [ ] Collapsible 컴포넌트 설치
  ```bash
  npx shadcn@latest add collapsible
  npx shadcn@latest add badge
  ```

#### ToolCallList 컴포넌트
- [ ] `src/components/chat/ToolCallList.tsx` 생성
  ```typescript
  import { ToolCall } from '@/types/message';
  import { ToolCallDisplay } from './ToolCallDisplay';

  interface ToolCallListProps {
    toolCalls: ToolCall[];
  }

  export function ToolCallList({ toolCalls }: ToolCallListProps) {
    if (toolCalls.length === 0) return null;

    return (
      <div className="space-y-2 mt-3">
        <h4 className="text-xs font-semibold text-muted-foreground">
          Tool Calls ({toolCalls.length})
        </h4>
        {toolCalls.map((toolCall) => (
          <ToolCallDisplay key={toolCall.id} toolCall={toolCall} />
        ))}
      </div>
    );
  }
  ```

#### 도구 아이콘 매핑
- [ ] `src/lib/tool-icons.ts` 생성
  ```typescript
  import {
    Terminal,
    FileText,
    Search,
    Globe,
    Code,
    Database,
    FolderOpen,
  } from 'lucide-react';

  export const toolIcons: Record<string, any> = {
    shell: Terminal,
    bash: Terminal,
    read: FileText,
    write: FileText,
    edit: Code,
    grep: Search,
    glob: FolderOpen,
    web_fetch: Globe,
    web_search: Search,
    database_query: Database,
  };

  export function getToolIcon(toolName: string) {
    const normalized = toolName.toLowerCase();
    return toolIcons[normalized] || Terminal;
  }
  ```

### 예상 결과물
- 도구 호출 카드 UI
- 확장/축소 기능
- 실행 상태 표시
- 입력/출력 포맷팅

### Commit 메시지
```
feat(web-ui): visualize tool calls in chat

- Create ToolCallDisplay component
- Add collapsible tool call details
- Show tool execution status with icons
- Display arguments, results, and errors
- Add ToolCallList wrapper component
- Implement tool icon mapping
```

---

## 5. 파일 Diff 뷰어 (Commit 17)

### 요구사항
- 파일 변경사항 비교
- Side-by-side 및 unified 뷰
- 변경사항 하이라이팅
- 라인 번호 표시

### 작업 내용

#### react-diff-viewer 설치
- [ ] Diff 뷰어 라이브러리 설치
  ```bash
  pnpm add react-diff-viewer-continued
  ```

#### FileDiff 컴포넌트
- [ ] `src/components/files/FileDiff.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import ReactDiffViewer, { DiffMethod } from 'react-diff-viewer-continued';
  import { Button } from '@/components/ui/button';
  import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
  import { Card } from '@/components/ui/card';
  import { GitCompare, SplitSquareHorizontal, AlignLeft } from 'lucide-react';

  interface FileDiffProps {
    oldContent: string;
    newContent: string;
    fileName: string;
    language?: string;
  }

  type ViewMode = 'split' | 'unified';

  export function FileDiff({
    oldContent,
    newContent,
    fileName,
    language = 'javascript',
  }: FileDiffProps) {
    const [viewMode, setViewMode] = useState<ViewMode>('split');

    const newStyles = {
      variables: {
        light: {
          codeFoldGutterBackground: '#f7f7f7',
          codeFoldBackground: '#f1f1f1',
          addedBackground: '#e6ffec',
          addedColor: '#24292e',
          removedBackground: '#ffeef0',
          removedColor: '#24292e',
          wordAddedBackground: '#acf2bd',
          wordRemovedBackground: '#fdb8c0',
          addedGutterBackground: '#cdffd8',
          removedGutterBackground: '#ffdce0',
          gutterBackground: '#f7f7f7',
          gutterBackgroundDark: '#f3f3f3',
          highlightBackground: '#fffbdd',
          highlightGutterBackground: '#fff5b1',
        },
        dark: {
          codeFoldGutterBackground: '#262626',
          codeFoldBackground: '#2d2d2d',
          addedBackground: '#044B53',
          addedColor: '#e6ffec',
          removedBackground: '#5c1e1e',
          removedColor: '#ffeef0',
          wordAddedBackground: '#055d67',
          wordRemovedBackground: '#7d2c2c',
          addedGutterBackground: '#034148',
          removedGutterBackground: '#632b30',
          gutterBackground: '#262626',
          gutterBackgroundDark: '#1c1c1c',
          highlightBackground: '#3d3d00',
          highlightGutterBackground: '#4d4d00',
        },
      },
    };

    return (
      <Card className="overflow-hidden">
        <div className="flex items-center justify-between p-3 border-b bg-muted/30">
          <div className="flex items-center gap-2">
            <GitCompare className="w-4 h-4" />
            <span className="font-mono text-sm font-semibold">{fileName}</span>
          </div>

          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant={viewMode === 'split' ? 'default' : 'ghost'}
              onClick={() => setViewMode('split')}
            >
              <SplitSquareHorizontal className="w-4 h-4 mr-2" />
              Split
            </Button>
            <Button
              size="sm"
              variant={viewMode === 'unified' ? 'default' : 'ghost'}
              onClick={() => setViewMode('unified')}
            >
              <AlignLeft className="w-4 h-4 mr-2" />
              Unified
            </Button>
          </div>
        </div>

        <div className="overflow-auto">
          <ReactDiffViewer
            oldValue={oldContent}
            newValue={newContent}
            splitView={viewMode === 'split'}
            compareMethod={DiffMethod.WORDS}
            styles={newStyles}
            leftTitle="Original"
            rightTitle="Modified"
            showDiffOnly={false}
            useDarkTheme={false}
          />
        </div>
      </Card>
    );
  }
  ```

#### DiffDialog 컴포넌트
- [ ] `src/components/files/DiffDialog.tsx` 생성
  ```typescript
  import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { FileDiff } from './FileDiff';

  interface DiffDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    fileName: string;
    oldContent: string;
    newContent: string;
    language?: string;
  }

  export function DiffDialog({
    open,
    onOpenChange,
    fileName,
    oldContent,
    newContent,
    language,
  }: DiffDialogProps) {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-6xl h-[80vh]">
          <DialogHeader>
            <DialogTitle>File Changes</DialogTitle>
          </DialogHeader>
          <FileDiff
            fileName={fileName}
            oldContent={oldContent}
            newContent={newContent}
            language={language}
          />
        </DialogContent>
      </Dialog>
    );
  }
  ```

#### 변경사항 요약
- [ ] `src/components/files/DiffSummary.tsx` 생성
  ```typescript
  interface DiffStats {
    additions: number;
    deletions: number;
    files: number;
  }

  interface DiffSummaryProps {
    stats: DiffStats;
  }

  export function DiffSummary({ stats }: DiffSummaryProps) {
    return (
      <div className="flex items-center gap-4 text-sm">
        <span className="text-muted-foreground">
          {stats.files} {stats.files === 1 ? 'file' : 'files'} changed
        </span>
        <span className="text-green-600">
          +{stats.additions} additions
        </span>
        <span className="text-red-600">
          -{stats.deletions} deletions
        </span>
      </div>
    );
  }
  ```

### 예상 결과물
- 파일 변경사항 비교
- Split/Unified 뷰 전환
- 변경사항 하이라이팅
- 통계 요약

### Commit 메시지
```
feat(web-ui): add file diff viewer

- Integrate react-diff-viewer-continued
- Create FileDiff component with split/unified views
- Add DiffDialog wrapper
- Implement DiffSummary for change statistics
- Support syntax highlighting in diffs
```

---

## 6. 승인 플로우 UI (Commit 18)

### 요구사항
- 도구 실행 전 사용자 승인 요청
- 승인/거부 버튼
- 항상 허용 옵션
- 승인 대기 상태 표시

### 작업 내용

#### ApprovalDialog 컴포넌트
- [ ] `src/components/chat/ApprovalDialog.tsx` 생성
  ```typescript
  import { useState } from 'react';
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/components/ui/dialog';
  import { Button } from '@/components/ui/button';
  import { Checkbox } from '@/components/ui/checkbox';
  import { Alert, AlertDescription } from '@/components/ui/alert';
  import { Terminal, AlertTriangle } from 'lucide-react';
  import { ToolCall } from '@/types/message';

  interface ApprovalDialogProps {
    open: boolean;
    toolCall: ToolCall;
    onApprove: (alwaysAllow: boolean) => void;
    onReject: () => void;
  }

  export function ApprovalDialog({
    open,
    toolCall,
    onApprove,
    onReject,
  }: ApprovalDialogProps) {
    const [alwaysAllow, setAlwaysAllow] = useState(false);

    const isDangerous = ['shell', 'bash', 'write', 'delete'].includes(
      toolCall.name.toLowerCase()
    );

    return (
      <Dialog open={open}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Terminal className="w-5 h-5" />
              Approval Required
            </DialogTitle>
            <DialogDescription>
              Codex wants to execute the following tool
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            {isDangerous && (
              <Alert variant="destructive">
                <AlertTriangle className="h-4 w-4" />
                <AlertDescription>
                  This tool can make changes to your system. Please review
                  carefully before approving.
                </AlertDescription>
              </Alert>
            )}

            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <span className="font-semibold">Tool:</span>
                <code className="px-2 py-1 bg-muted rounded font-mono text-sm">
                  {toolCall.name}
                </code>
              </div>

              <div>
                <span className="font-semibold block mb-1">Arguments:</span>
                <pre className="p-3 bg-muted rounded text-xs overflow-x-auto">
                  {JSON.stringify(toolCall.arguments, null, 2)}
                </pre>
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <Checkbox
                id="always-allow"
                checked={alwaysAllow}
                onCheckedChange={(checked) => setAlwaysAllow(checked as boolean)}
              />
              <label
                htmlFor="always-allow"
                className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
              >
                Always allow this tool without asking
              </label>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={onReject}>
              Reject
            </Button>
            <Button onClick={() => onApprove(alwaysAllow)}>
              Approve
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
  ```

#### shadcn Checkbox 및 Alert 설치
- [ ] 필요한 컴포넌트 설치
  ```bash
  npx shadcn@latest add checkbox
  npx shadcn@latest add alert
  ```

#### 승인 스토어
- [ ] `src/store/approval-store.ts` 생성
  ```typescript
  import { create } from 'zustand';
  import { persist } from 'zustand/middleware';
  import { ToolCall } from '@/types/message';

  interface ApprovalState {
    pendingApproval: ToolCall | null;
    alwaysAllowedTools: Set<string>;

    setPendingApproval: (toolCall: ToolCall | null) => void;
    addAlwaysAllowed: (toolName: string) => void;
    isAlwaysAllowed: (toolName: string) => boolean;
    clearAlwaysAllowed: () => void;
  }

  export const useApprovalStore = create<ApprovalState>()(
    persist(
      (set, get) => ({
        pendingApproval: null,
        alwaysAllowedTools: new Set(),

        setPendingApproval: (toolCall) => {
          set({ pendingApproval: toolCall });
        },

        addAlwaysAllowed: (toolName) => {
          set((state) => ({
            alwaysAllowedTools: new Set([...state.alwaysAllowedTools, toolName]),
          }));
        },

        isAlwaysAllowed: (toolName) => {
          return get().alwaysAllowedTools.has(toolName);
        },

        clearAlwaysAllowed: () => {
          set({ alwaysAllowedTools: new Set() });
        },
      }),
      {
        name: 'approval-storage',
        partialize: (state) => ({
          alwaysAllowedTools: Array.from(state.alwaysAllowedTools),
        }),
        merge: (persistedState: any, currentState) => ({
          ...currentState,
          alwaysAllowedTools: new Set(persistedState?.alwaysAllowedTools || []),
        }),
      }
    )
  );
  ```

#### ApprovalManager 컴포넌트
- [ ] `src/components/chat/ApprovalManager.tsx` 생성
  ```typescript
  import { useEffect } from 'react';
  import { useApprovalStore } from '@/store/approval-store';
  import { useChatStore } from '@/store/chat-store';
  import { ApprovalDialog } from './ApprovalDialog';
  import { useWebSocket } from '@/hooks/useWebSocket';

  export function ApprovalManager() {
    const { pendingApproval, setPendingApproval, addAlwaysAllowed } =
      useApprovalStore();
    const { updateToolCall } = useChatStore();
    const { sendMessage } = useWebSocket(WS_URL);

    const handleApprove = (alwaysAllow: boolean) => {
      if (!pendingApproval) return;

      if (alwaysAllow) {
        addAlwaysAllowed(pendingApproval.name);
      }

      // 승인 메시지 전송
      sendMessage({
        type: 'tool_approval',
        toolCallId: pendingApproval.id,
        approved: true,
      });

      updateToolCall(
        pendingApproval.id,
        pendingApproval.id,
        { status: 'running' }
      );

      setPendingApproval(null);
    };

    const handleReject = () => {
      if (!pendingApproval) return;

      // 거부 메시지 전송
      sendMessage({
        type: 'tool_approval',
        toolCallId: pendingApproval.id,
        approved: false,
      });

      updateToolCall(
        pendingApproval.id,
        pendingApproval.id,
        { status: 'failed', error: 'User rejected' }
      );

      setPendingApproval(null);
    };

    return (
      <ApprovalDialog
        open={!!pendingApproval}
        toolCall={pendingApproval!}
        onApprove={handleApprove}
        onReject={handleReject}
      />
    );
  }
  ```

#### ChatPage에 통합
- [ ] `src/pages/ChatPage.tsx` 업데이트
  ```typescript
  import { ApprovalManager } from '@/components/chat/ApprovalManager';

  export function ChatPage() {
    return (
      <div className="flex flex-col h-screen">
        {/* ... existing code ... */}
        <ApprovalManager />
      </div>
    );
  }
  ```

### 예상 결과물
- 도구 승인 다이얼로그
- 위험한 도구 경고
- 항상 허용 옵션
- 승인 상태 관리

### Commit 메시지
```
feat(web-ui): implement approval flow for tool execution

- Create ApprovalDialog component
- Add approval store with persistence
- Implement ApprovalManager
- Show warnings for dangerous tools
- Support "always allow" option
- Integrate with WebSocket for approval messages
```

---

## Day 3 완료 체크리스트

- [ ] 파일 탐색기 UI 구현 (트리 뷰, 아이콘)
- [ ] 파일 뷰어 (Monaco Editor, 멀티탭)
- [ ] 파일 업로드/다운로드 (드래그 앤 드롭, 진행률)
- [ ] 도구 호출 시각화 (상태 표시, 확장/축소)
- [ ] 파일 Diff 뷰어 (Split/Unified 뷰)
- [ ] 승인 플로우 (다이얼로그, 항상 허용)
- [ ] 모든 커밋 메시지 명확하게 작성
- [ ] 기능 테스트 및 검증

---

## 다음 단계 (Day 4 예고)

1. 세션 관리 구조 및 스토어
2. 세션 UI (생성, 삭제, 전환)
3. 히스토리 저장 및 로드 (IndexedDB)
4. 검색 기능 (전체 세션 검색)
5. 세션 내보내기 (JSON, Markdown)
6. 세션 통계 대시보드

---

## 참고 자료

- [Monaco Editor React](https://github.com/suren-atoyan/monaco-react)
- [react-dropzone](https://react-dropzone.js.org/)
- [react-diff-viewer](https://github.com/praneshr/react-diff-viewer)
- [Lucide Icons](https://lucide.dev/)

---

**Last Updated**: 2025-11-20
**Version**: 1.0
**Day**: 3 / 7
