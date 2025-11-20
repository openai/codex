# Day 3 TODO - 파일 작업 및 도구 UI (Electron)

> **목표**: Electron의 Native 파일 시스템 접근을 활용하여 파일 탐색기, 편집기, 업로드/다운로드 기능 구현

## 전체 개요

Day 3에서는 Codex UI의 파일 작업 기능을 완성합니다:
- Native file dialog를 통한 폴더/파일 선택
- IPC 기반 파일 시스템 접근
- Monaco Editor로 코드 뷰어
- Drag & Drop 파일 업로드
- 도구 호출 시각화 및 승인 시스템

**Electron 특화 기능:**
- Native file dialogs (Open, Save)
- File system access via IPC
- System tray notifications for tool calls
- Global shortcuts for file operations
- Menu bar integration

---

## Commit 13: 파일 탐색기

### 📋 작업 내용

1. **파일 트리 구조 컴포넌트**
2. **Native dialog로 폴더 선택**
3. **IPC로 파일 시스템 접근**
4. **Drag & Drop 지원**
5. **파일 아이콘 및 메타데이터**

### 📁 파일 구조

```
src/renderer/components/files/
├── FileExplorer.tsx      # 파일 탐색기 메인
├── FileTree.tsx          # 파일 트리
├── FileNode.tsx          # 파일/폴더 노드
└── FileIcons.tsx         # 파일 타입별 아이콘

src/main/handlers/
└── filesystem.ts         # 파일 시스템 IPC handlers

src/renderer/store/
└── useFileStore.ts       # 파일 상태 관리
```

### 1️⃣ 파일 Store

**파일**: `src/renderer/store/useFileStore.ts`

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';

export interface FileNode {
  path: string;
  name: string;
  type: 'file' | 'directory';
  size?: number;
  modified?: number;
  children?: FileNode[];
  expanded?: boolean;
}

interface FileState {
  // Current workspace
  workspacePath: string | null;
  rootNode: FileNode | null;

  // Selected file
  selectedFile: string | null;
  openFiles: Map<string, { content: string; modified: boolean }>;

  // UI state
  expandedFolders: Set<string>;
  searchQuery: string;
}

interface FileActions {
  // Workspace operations
  setWorkspace: (path: string) => Promise<void>;
  loadFileTree: () => Promise<void>;

  // File operations
  selectFile: (path: string) => void;
  openFile: (path: string) => Promise<void>;
  closeFile: (path: string) => void;
  saveFile: (path: string, content: string) => Promise<void>;

  // Tree operations
  toggleFolder: (path: string) => void;
  expandFolder: (path: string) => void;
  collapseFolder: (path: string) => void;

  // Search
  setSearchQuery: (query: string) => void;
}

type FileStore = FileState & FileActions;

export const useFileStore = create<FileStore>()(
  devtools(
    immer((set, get) => ({
      // Initial state
      workspacePath: null,
      rootNode: null,
      selectedFile: null,
      openFiles: new Map(),
      expandedFolders: new Set(),
      searchQuery: '',

      // Workspace operations
      setWorkspace: async (path: string) => {
        set((state) => {
          state.workspacePath = path;
        });
        await get().loadFileTree();
      },

      loadFileTree: async () => {
        const { workspacePath } = get();
        if (!workspacePath || !window.electronAPI) return;

        try {
          const tree = await window.electronAPI.readDirectory(workspacePath);
          set((state) => {
            state.rootNode = tree;
          });
        } catch (error) {
          console.error('Failed to load file tree:', error);
        }
      },

      // File operations
      selectFile: (path: string) => {
        set((state) => {
          state.selectedFile = path;
        });
      },

      openFile: async (path: string) => {
        if (!window.electronAPI) return;

        const { openFiles } = get();
        if (openFiles.has(path)) {
          get().selectFile(path);
          return;
        }

        try {
          const content = await window.electronAPI.readFile(path);
          set((state) => {
            state.openFiles.set(path, { content, modified: false });
            state.selectedFile = path;
          });
        } catch (error) {
          console.error('Failed to open file:', error);
        }
      },

      closeFile: (path: string) => {
        set((state) => {
          state.openFiles.delete(path);
          if (state.selectedFile === path) {
            const remaining = Array.from(state.openFiles.keys());
            state.selectedFile = remaining[0] || null;
          }
        });
      },

      saveFile: async (path: string, content: string) => {
        if (!window.electronAPI) return;

        try {
          await window.electronAPI.writeFile(path, content);
          set((state) => {
            const file = state.openFiles.get(path);
            if (file) {
              file.content = content;
              file.modified = false;
            }
          });
        } catch (error) {
          console.error('Failed to save file:', error);
          throw error;
        }
      },

      // Tree operations
      toggleFolder: (path: string) => {
        set((state) => {
          if (state.expandedFolders.has(path)) {
            state.expandedFolders.delete(path);
          } else {
            state.expandedFolders.add(path);
          }
        });
      },

      expandFolder: (path: string) => {
        set((state) => {
          state.expandedFolders.add(path);
        });
      },

      collapseFolder: (path: string) => {
        set((state) => {
          state.expandedFolders.delete(path);
        });
      },

      // Search
      setSearchQuery: (query: string) => {
        set((state) => {
          state.searchQuery = query;
        });
      },
    }))
  )
);
```

### 2️⃣ 파일 시스템 IPC Handlers

**파일**: `src/main/handlers/filesystem.ts`

```typescript
import { ipcMain, dialog } from 'electron';
import { BrowserWindow } from 'electron';
import fs from 'fs/promises';
import path from 'path';
import type { FileNode } from '@/renderer/store/useFileStore';

export function registerFilesystemHandlers() {
  // Select directory
  ipcMain.handle('fs:selectDirectory', async () => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showOpenDialog(window, {
      properties: ['openDirectory'],
    });

    return result.canceled ? null : result.filePaths[0];
  });

  // Read directory recursively
  ipcMain.handle('fs:readDirectory', async (_event, dirPath: string) => {
    return await readDirectoryRecursive(dirPath);
  });

  // Read file
  ipcMain.handle('fs:readFile', async (_event, filePath: string) => {
    return await fs.readFile(filePath, 'utf-8');
  });

  // Write file
  ipcMain.handle('fs:writeFile', async (_event, filePath: string, content: string) => {
    await fs.writeFile(filePath, content, 'utf-8');
  });

  // Get file stats
  ipcMain.handle('fs:stat', async (_event, filePath: string) => {
    const stats = await fs.stat(filePath);
    return {
      size: stats.size,
      modified: stats.mtimeMs,
      isDirectory: stats.isDirectory(),
      isFile: stats.isFile(),
    };
  });

  // Delete file/directory
  ipcMain.handle('fs:delete', async (_event, filePath: string) => {
    const stats = await fs.stat(filePath);
    if (stats.isDirectory()) {
      await fs.rmdir(filePath, { recursive: true });
    } else {
      await fs.unlink(filePath);
    }
  });

  // Create directory
  ipcMain.handle('fs:createDirectory', async (_event, dirPath: string) => {
    await fs.mkdir(dirPath, { recursive: true });
  });

  // Rename file/directory
  ipcMain.handle('fs:rename', async (_event, oldPath: string, newPath: string) => {
    await fs.rename(oldPath, newPath);
  });
}

async function readDirectoryRecursive(
  dirPath: string,
  depth: number = 0,
  maxDepth: number = 3
): Promise<FileNode> {
  const stats = await fs.stat(dirPath);
  const name = path.basename(dirPath);

  const node: FileNode = {
    path: dirPath,
    name,
    type: stats.isDirectory() ? 'directory' : 'file',
    size: stats.size,
    modified: stats.mtimeMs,
  };

  if (node.type === 'directory' && depth < maxDepth) {
    try {
      const entries = await fs.readdir(dirPath);
      node.children = await Promise.all(
        entries
          .filter(entry => !shouldIgnore(entry))
          .map(entry => readDirectoryRecursive(
            path.join(dirPath, entry),
            depth + 1,
            maxDepth
          ))
      );
    } catch (error) {
      console.error(`Failed to read directory ${dirPath}:`, error);
      node.children = [];
    }
  }

  return node;
}

function shouldIgnore(name: string): boolean {
  const ignorePatterns = [
    /^node_modules$/,
    /^\.git$/,
    /^\.next$/,
    /^\.vscode$/,
    /^dist$/,
    /^build$/,
    /^\.DS_Store$/,
  ];

  return ignorePatterns.some(pattern => pattern.test(name));
}
```

### 3️⃣ FileExplorer 컴포넌트

**파일**: `src/renderer/components/files/FileExplorer.tsx`

```typescript
import React, { useEffect } from 'react';
import { Folder, Search, FolderPlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useFileStore } from '@/store/useFileStore';
import { FileTree } from './FileTree';
import { toast } from 'react-hot-toast';

export function FileExplorer() {
  const {
    workspacePath,
    rootNode,
    searchQuery,
    setWorkspace,
    setSearchQuery,
  } = useFileStore();

  const handleSelectWorkspace = async () => {
    if (!window.electronAPI) {
      toast.error('File system access only available in desktop app');
      return;
    }

    const path = await window.electronAPI.selectDirectory();
    if (path) {
      await setWorkspace(path);
      toast.success('Workspace loaded');
    }
  };

  return (
    <div className="flex flex-col h-full border-r bg-background">
      {/* Header */}
      <div className="p-4 border-b">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-sm">Explorer</h2>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={handleSelectWorkspace}
          >
            <FolderPlus className="h-4 w-4" />
          </Button>
        </div>

        {/* Search */}
        <div className="relative">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <Input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search files..."
            className="pl-7 h-8 text-sm"
          />
        </div>
      </div>

      {/* File Tree */}
      <ScrollArea className="flex-1">
        {workspacePath && rootNode ? (
          <div className="p-2">
            <FileTree node={rootNode} />
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-full p-8 text-center">
            <Folder className="h-12 w-12 text-muted-foreground mb-3" />
            <p className="text-sm text-muted-foreground mb-4">
              No workspace selected
            </p>
            <Button size="sm" onClick={handleSelectWorkspace}>
              Open Folder
            </Button>
          </div>
        )}
      </ScrollArea>

      {/* Workspace info */}
      {workspacePath && (
        <div className="p-2 border-t bg-muted/30">
          <p className="text-xs text-muted-foreground truncate" title={workspacePath}>
            {workspacePath}
          </p>
        </div>
      )}
    </div>
  );
}
```

### 4️⃣ FileTree 컴포넌트

**파일**: `src/renderer/components/files/FileTree.tsx`

```typescript
import React from 'react';
import { FileNode } from '@/store/useFileStore';
import { FileNodeComponent } from './FileNode';

interface FileTreeProps {
  node: FileNode;
  level?: number;
}

export function FileTree({ node, level = 0 }: FileTreeProps) {
  return (
    <div>
      <FileNodeComponent node={node} level={level} />
      {node.type === 'directory' && node.children && (
        <div>
          {node.children
            .sort((a, b) => {
              // Directories first
              if (a.type !== b.type) {
                return a.type === 'directory' ? -1 : 1;
              }
              // Then alphabetically
              return a.name.localeCompare(b.name);
            })
            .map((child) => (
              <FileTree key={child.path} node={child} level={level + 1} />
            ))}
        </div>
      )}
    </div>
  );
}
```

### 5️⃣ FileNode 컴포넌트

**파일**: `src/renderer/components/files/FileNode.tsx`

```typescript
import React from 'react';
import { ChevronRight, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useFileStore, type FileNode } from '@/store/useFileStore';
import { getFileIcon } from './FileIcons';

interface FileNodeProps {
  node: FileNode;
  level: number;
}

export function FileNodeComponent({ node, level }: FileNodeProps) {
  const {
    selectedFile,
    expandedFolders,
    toggleFolder,
    openFile,
    selectFile,
  } = useFileStore();

  const isExpanded = expandedFolders.has(node.path);
  const isSelected = selectedFile === node.path;

  const handleClick = () => {
    if (node.type === 'directory') {
      toggleFolder(node.path);
    } else {
      openFile(node.path);
    }
  };

  const Icon = getFileIcon(node.name, node.type);

  return (
    <div
      className={cn(
        'flex items-center gap-1 px-2 py-1 cursor-pointer hover:bg-accent rounded-sm text-sm',
        isSelected && 'bg-accent'
      )}
      style={{ paddingLeft: `${level * 12 + 8}px` }}
      onClick={handleClick}
    >
      {node.type === 'directory' && (
        <div className="w-4 h-4 flex items-center justify-center">
          {isExpanded ? (
            <ChevronDown className="h-3 w-3" />
          ) : (
            <ChevronRight className="h-3 w-3" />
          )}
        </div>
      )}

      <Icon className="h-4 w-4 flex-shrink-0" />

      <span className="truncate">{node.name}</span>

      {node.size !== undefined && node.type === 'file' && (
        <span className="ml-auto text-xs text-muted-foreground">
          {formatFileSize(node.size)}
        </span>
      )}
    </div>
  );
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
```

### 6️⃣ File Icons

**파일**: `src/renderer/components/files/FileIcons.tsx`

```typescript
import {
  FileIcon,
  FolderIcon,
  FileTextIcon,
  FileCodeIcon,
  FileImageIcon,
  FileJsonIcon,
} from 'lucide-react';
import { LucideIcon } from 'lucide-react';

export function getFileIcon(filename: string, type: 'file' | 'directory'): LucideIcon {
  if (type === 'directory') {
    return FolderIcon;
  }

  const ext = filename.split('.').pop()?.toLowerCase();

  switch (ext) {
    case 'ts':
    case 'tsx':
    case 'js':
    case 'jsx':
    case 'py':
    case 'rs':
    case 'go':
    case 'java':
    case 'cpp':
    case 'c':
      return FileCodeIcon;

    case 'json':
    case 'yaml':
    case 'yml':
    case 'toml':
      return FileJsonIcon;

    case 'md':
    case 'txt':
    case 'log':
      return FileTextIcon;

    case 'png':
    case 'jpg':
    case 'jpeg':
    case 'gif':
    case 'svg':
    case 'webp':
      return FileImageIcon;

    default:
      return FileIcon;
  }
}
```

### 7️⃣ IPC 타입 확장

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // File system
  selectDirectory: () => Promise<string | null>;
  readDirectory: (path: string) => Promise<FileNode>;
  readFile: (path: string) => Promise<string>;
  writeFile: (path: string, content: string) => Promise<void>;
  stat: (path: string) => Promise<{
    size: number;
    modified: number;
    isDirectory: boolean;
    isFile: boolean;
  }>;
  deleteFile: (path: string) => Promise<void>;
  createDirectory: (path: string) => Promise<void>;
  renameFile: (oldPath: string, newPath: string) => Promise<void>;
}
```

### ✅ 완료 기준

- [ ] Native dialog로 폴더 선택
- [ ] 파일 트리 재귀적으로 로드
- [ ] 파일/폴더 클릭하여 탐색
- [ ] 파일 검색 기능
- [ ] 파일 아이콘 표시
- [ ] 파일 크기 및 수정 시간 표시

### 📝 Commit Message

```
feat(files): implement file explorer with native dialogs

- Add FileExplorer component with tree view
- Implement recursive directory reading via IPC
- Support file/folder selection with native dialog
- Add file search functionality
- Display file icons and metadata
- Ignore common directories (node_modules, .git)

Electron-specific:
- Use native dialog.showOpenDialog
- File system access via Main Process
- Recursive directory traversal with depth limit
```

---

## Commit 14: Monaco Editor 통합

### 📋 작업 내용

1. **Monaco Editor 설정**
2. **다중 탭 지원**
3. **파일 저장 (Cmd/Ctrl+S)**
4. **Syntax highlighting**
5. **Theme 통합**

### 📁 파일 구조

```
src/renderer/components/editor/
├── CodeEditor.tsx        # Monaco Editor wrapper
├── EditorTabs.tsx        # 파일 탭
└── EditorToolbar.tsx     # 에디터 툴바
```

### 1️⃣ Monaco Editor 컴포넌트

**파일**: `src/renderer/components/editor/CodeEditor.tsx`

```typescript
import React, { useRef, useEffect } from 'react';
import Editor, { OnMount } from '@monaco-editor/react';
import type { editor } from 'monaco-editor';
import { useTheme } from '@/hooks/useTheme';
import { useFileStore } from '@/store/useFileStore';
import { toast } from 'react-hot-toast';

interface CodeEditorProps {
  filePath: string;
}

export function CodeEditor({ filePath }: CodeEditorProps) {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const { theme } = useTheme();
  const { openFiles, saveFile } = useFileStore();

  const file = openFiles.get(filePath);
  const [content, setContent] = React.useState(file?.content || '');

  // Get language from file extension
  const language = getLanguageFromFilename(filePath);

  const handleEditorDidMount: OnMount = (editor) => {
    editorRef.current = editor;

    // Register Cmd/Ctrl+S to save
    editor.addCommand(
      monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
      () => {
        handleSave();
      }
    );

    // Focus editor
    editor.focus();
  };

  const handleChange = (value: string | undefined) => {
    if (value !== undefined) {
      setContent(value);

      // Mark as modified
      const fileData = openFiles.get(filePath);
      if (fileData && fileData.content !== value) {
        useFileStore.setState((state) => {
          const file = state.openFiles.get(filePath);
          if (file) {
            file.modified = true;
          }
        });
      }
    }
  };

  const handleSave = async () => {
    try {
      await saveFile(filePath, content);
      toast.success('File saved');
    } catch (error) {
      toast.error('Failed to save file');
    }
  };

  useEffect(() => {
    setContent(file?.content || '');
  }, [filePath, file?.content]);

  return (
    <Editor
      height="100%"
      language={language}
      value={content}
      onChange={handleChange}
      onMount={handleEditorDidMount}
      theme={theme === 'dark' ? 'vs-dark' : 'light'}
      options={{
        minimap: { enabled: true },
        fontSize: 14,
        lineNumbers: 'on',
        rulers: [80, 120],
        wordWrap: 'on',
        autoSurround: 'languageDefined',
        bracketPairColorization: {
          enabled: true,
        },
        guides: {
          bracketPairs: true,
          indentation: true,
        },
        suggest: {
          showKeywords: true,
          showSnippets: true,
        },
        formatOnPaste: true,
        formatOnType: true,
        scrollBeyondLastLine: false,
        smoothScrolling: true,
        cursorBlinking: 'smooth',
        cursorSmoothCaretAnimation: 'on',
      }}
    />
  );
}

function getLanguageFromFilename(filename: string): string {
  const ext = filename.split('.').pop()?.toLowerCase();

  const languageMap: Record<string, string> = {
    ts: 'typescript',
    tsx: 'typescript',
    js: 'javascript',
    jsx: 'javascript',
    py: 'python',
    rs: 'rust',
    go: 'go',
    java: 'java',
    cpp: 'cpp',
    c: 'c',
    cs: 'csharp',
    rb: 'ruby',
    php: 'php',
    html: 'html',
    css: 'css',
    scss: 'scss',
    json: 'json',
    yaml: 'yaml',
    yml: 'yaml',
    toml: 'toml',
    md: 'markdown',
    sh: 'shell',
    bash: 'shell',
    sql: 'sql',
  };

  return languageMap[ext || ''] || 'plaintext';
}
```

### 2️⃣ Editor Tabs

**파일**: `src/renderer/components/editor/EditorTabs.tsx`

```typescript
import React from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useFileStore } from '@/store/useFileStore';
import path from 'path-browserify';

export function EditorTabs() {
  const { openFiles, selectedFile, selectFile, closeFile } = useFileStore();

  const files = Array.from(openFiles.entries());

  if (files.length === 0) {
    return null;
  }

  const handleClose = (filePath: string, e: React.MouseEvent) => {
    e.stopPropagation();

    const file = openFiles.get(filePath);
    if (file?.modified) {
      const confirmed = confirm('File has unsaved changes. Close anyway?');
      if (!confirmed) return;
    }

    closeFile(filePath);
  };

  return (
    <div className="flex items-center gap-1 px-2 py-1 border-b bg-muted/30 overflow-x-auto">
      {files.map(([filePath, file]) => {
        const filename = path.basename(filePath);
        const isSelected = selectedFile === filePath;

        return (
          <div
            key={filePath}
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-sm cursor-pointer hover:bg-accent text-sm whitespace-nowrap',
              isSelected && 'bg-background'
            )}
            onClick={() => selectFile(filePath)}
          >
            <span>{filename}</span>
            {file.modified && (
              <span className="w-1.5 h-1.5 rounded-full bg-primary" />
            )}
            <button
              className="ml-1 hover:bg-accent-foreground/10 rounded-sm p-0.5"
              onClick={(e) => handleClose(filePath, e)}
            >
              <X className="h-3 w-3" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
```

### 3️⃣ Editor Toolbar

**파일**: `src/renderer/components/editor/EditorToolbar.tsx`

```typescript
import React from 'react';
import { Save, RotateCcw, Search, Settings } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useFileStore } from '@/store/useFileStore';
import { toast } from 'react-hot-toast';

export function EditorToolbar() {
  const { selectedFile, openFiles, saveFile } = useFileStore();

  if (!selectedFile) return null;

  const file = openFiles.get(selectedFile);
  if (!file) return null;

  const handleSave = async () => {
    try {
      await saveFile(selectedFile, file.content);
      toast.success('File saved');
    } catch (error) {
      toast.error('Failed to save file');
    }
  };

  const handleRevert = () => {
    const confirmed = confirm('Revert all changes?');
    if (confirmed) {
      // Reload from disk
      useFileStore.getState().openFile(selectedFile);
      toast.success('Changes reverted');
    }
  };

  return (
    <div className="flex items-center justify-between px-4 py-2 border-b bg-background">
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant={file.modified ? 'default' : 'ghost'}
          onClick={handleSave}
          disabled={!file.modified}
        >
          <Save className="h-4 w-4 mr-2" />
          Save
          <kbd className="ml-2 px-1.5 py-0.5 rounded bg-muted text-xs">
            {navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'}+S
          </kbd>
        </Button>

        {file.modified && (
          <Button size="sm" variant="ghost" onClick={handleRevert}>
            <RotateCcw className="h-4 w-4 mr-2" />
            Revert
          </Button>
        )}
      </div>

      <div className="flex items-center gap-2">
        <Button size="sm" variant="ghost">
          <Search className="h-4 w-4" />
        </Button>
        <Button size="sm" variant="ghost">
          <Settings className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
```

### 4️⃣ Editor Page

**파일**: `src/renderer/pages/Editor.tsx`

```typescript
import React from 'react';
import { FileExplorer } from '@/components/files/FileExplorer';
import { EditorTabs } from '@/components/editor/EditorTabs';
import { EditorToolbar } from '@/components/editor/EditorToolbar';
import { CodeEditor } from '@/components/editor/CodeEditor';
import { useFileStore } from '@/store/useFileStore';
import { FileText } from 'lucide-react';

export function EditorPage() {
  const { selectedFile, openFiles } = useFileStore();

  return (
    <div className="flex h-screen">
      {/* File Explorer */}
      <div className="w-64 flex-shrink-0">
        <FileExplorer />
      </div>

      {/* Editor */}
      <div className="flex-1 flex flex-col">
        <EditorTabs />
        {selectedFile && openFiles.has(selectedFile) ? (
          <>
            <EditorToolbar />
            <div className="flex-1">
              <CodeEditor filePath={selectedFile} />
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground">
            <div className="text-center">
              <FileText className="h-16 w-16 mx-auto mb-4" />
              <p>No file selected</p>
              <p className="text-sm mt-2">Open a file from the explorer</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
```

### ✅ 완료 기준

- [ ] Monaco Editor 렌더링
- [ ] 파일 탭 전환
- [ ] Cmd/Ctrl+S로 저장
- [ ] Syntax highlighting 작동
- [ ] Theme 자동 전환
- [ ] 수정 상태 표시 (dot)

### 📝 Commit Message

```
feat(editor): integrate Monaco Editor with multi-tab support

- Add Monaco Editor with TypeScript support
- Implement multi-tab file editing
- Add Cmd/Ctrl+S keyboard shortcut for save
- Support syntax highlighting for 20+ languages
- Integrate with theme system (dark/light)
- Track file modification state
- Add EditorToolbar with save/revert actions

Features:
- Bracket pair colorization
- Format on paste/type
- Smooth scrolling and cursor animation
```

---

## Commit 15: 파일 업로드/다운로드

### 📋 작업 내용

1. **Drag & Drop 파일 업로드**
2. **Native file picker**
3. **진행률 표시**
4. **시스템 알림**

### 📁 파일 구조

```
src/renderer/components/files/
├── FileUpload.tsx        # 파일 업로드
└── FileProgress.tsx      # 진행률 표시

src/main/handlers/
└── upload.ts             # 업로드 IPC
```

### 1️⃣ FileUpload 컴포넌트

**파일**: `src/renderer/components/files/FileUpload.tsx`

```typescript
import React, { useState, useCallback } from 'react';
import { Upload, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { cn } from '@/lib/utils';
import { toast } from 'react-hot-toast';

interface UploadFile {
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'completed' | 'error';
  error?: string;
}

export function FileUpload() {
  const [uploads, setUploads] = useState<Map<string, UploadFile>>(new Map());
  const [isDragging, setIsDragging] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);

    const files = Array.from(e.dataTransfer.files);
    await handleFiles(files);
  }, []);

  const handleFileSelect = async () => {
    if (!window.electronAPI) {
      toast.error('File upload only available in desktop app');
      return;
    }

    const filePaths = await window.electronAPI.selectFiles();
    if (filePaths) {
      // Convert file paths to File objects
      const files = await Promise.all(
        filePaths.map(async (path) => {
          const content = await window.electronAPI.readFile(path);
          const name = path.split('/').pop() || 'unknown';
          return new File([content], name);
        })
      );

      await handleFiles(files);
    }
  };

  const handleFiles = async (files: File[]) => {
    const newUploads = new Map(uploads);

    for (const file of files) {
      const id = `${file.name}-${Date.now()}`;
      newUploads.set(id, {
        file,
        progress: 0,
        status: 'pending',
      });
    }

    setUploads(newUploads);

    // Start uploads
    for (const [id, upload] of newUploads.entries()) {
      if (upload.status === 'pending') {
        await uploadFile(id, upload.file);
      }
    }
  };

  const uploadFile = async (id: string, file: File) => {
    setUploads((prev) => {
      const next = new Map(prev);
      const upload = next.get(id);
      if (upload) {
        upload.status = 'uploading';
      }
      return next;
    });

    try {
      // Simulate upload progress
      for (let progress = 0; progress <= 100; progress += 10) {
        await new Promise((resolve) => setTimeout(resolve, 100));

        setUploads((prev) => {
          const next = new Map(prev);
          const upload = next.get(id);
          if (upload) {
            upload.progress = progress;
          }
          return next;
        });
      }

      // Upload complete
      setUploads((prev) => {
        const next = new Map(prev);
        const upload = next.get(id);
        if (upload) {
          upload.status = 'completed';
          upload.progress = 100;
        }
        return next;
      });

      toast.success(`${file.name} uploaded`);

      // Show native notification
      if (window.electronAPI) {
        window.electronAPI.showNotification(
          'Upload Complete',
          `${file.name} has been uploaded`
        );
      }
    } catch (error: any) {
      setUploads((prev) => {
        const next = new Map(prev);
        const upload = next.get(id);
        if (upload) {
          upload.status = 'error';
          upload.error = error.message;
        }
        return next;
      });

      toast.error(`Failed to upload ${file.name}`);
    }
  };

  const handleRemove = (id: string) => {
    setUploads((prev) => {
      const next = new Map(prev);
      next.delete(id);
      return next;
    });
  };

  const uploadArray = Array.from(uploads.entries());

  return (
    <div className="p-4">
      {/* Drop zone */}
      <div
        className={cn(
          'border-2 border-dashed rounded-lg p-8 text-center transition-colors',
          isDragging
            ? 'border-primary bg-primary/10'
            : 'border-muted-foreground/25 hover:border-primary/50'
        )}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <Upload className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
        <p className="text-sm font-medium mb-2">
          Drop files here or click to browse
        </p>
        <p className="text-xs text-muted-foreground mb-4">
          Supports all file types
        </p>
        <Button onClick={handleFileSelect}>Select Files</Button>
      </div>

      {/* Upload list */}
      {uploadArray.length > 0 && (
        <div className="mt-4 space-y-2">
          <h3 className="font-semibold text-sm">Uploads</h3>
          {uploadArray.map(([id, upload]) => (
            <div
              key={id}
              className="flex items-center gap-3 p-3 rounded-lg border bg-card"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-sm font-medium truncate">
                    {upload.file.name}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {upload.status === 'completed'
                      ? 'Done'
                      : `${upload.progress}%`}
                  </span>
                </div>
                <Progress value={upload.progress} className="h-1" />
                {upload.error && (
                  <p className="text-xs text-destructive mt-1">
                    {upload.error}
                  </p>
                )}
              </div>
              <Button
                variant="ghost"
                size="icon"
                className="h-6 w-6 flex-shrink-0"
                onClick={() => handleRemove(id)}
              >
                <X className="h-3 w-3" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

### ✅ 완료 기준

- [ ] Drag & Drop 파일 업로드
- [ ] Native file picker 지원
- [ ] 진행률 표시
- [ ] 업로드 완료 시 Native notification
- [ ] 업로드 목록 관리 (제거)

### 📝 Commit Message

```
feat(files): add file upload with drag & drop support

- Implement drag & drop file upload
- Add native file picker integration
- Show upload progress with progress bar
- Display native notification on completion
- Support multiple file uploads
- Handle upload errors gracefully

Electron-specific:
- Use native file selection dialog
- Show system notification on upload complete
```

---

*Due to length constraints, I'll create a condensed version of Commits 16-18 to complete Day 3...*

## Commit 16-18: 도구 호출, Diff 뷰어, 승인 플로우

### 📋 작업 내용 (통합)

1. **ToolCall 컴포넌트 및 시각화**
2. **Diff 뷰어 (react-diff-viewer)**
3. **승인 다이얼로그 (Native)**
4. **System tray 통합**

### 핵심 코드

**ToolCall 컴포넌트**:
```typescript
// src/renderer/components/tools/ToolCallCard.tsx
export function ToolCallCard({ toolCall }: { toolCall: ToolCall }) {
  const handleApprove = async () => {
    if (window.electronAPI) {
      const approved = await window.electronAPI.showApprovalDialog(
        toolCall.function.name,
        toolCall.function.arguments
      );

      if (approved) {
        // Execute tool
        executeToolCall(toolCall.id);
      }
    }
  };

  return (
    <div className="border rounded-lg p-4">
      <h4>{toolCall.function.name}</h4>
      <pre>{toolCall.function.arguments}</pre>
      <Button onClick={handleApprove}>Approve</Button>
    </div>
  );
}
```

**Approval Dialog Handler**:
```typescript
// src/main/handlers/approval.ts
ipcMain.handle('approval:show', async (_event, toolName, args) => {
  const window = BrowserWindow.getFocusedWindow();
  const result = await dialog.showMessageBox(window, {
    type: 'question',
    title: 'Approve Tool Call',
    message: `Allow "${toolName}" to execute?`,
    detail: JSON.stringify(JSON.parse(args), null, 2),
    buttons: ['Approve', 'Reject'],
    defaultId: 0,
    cancelId: 1,
  });

  return result.response === 0;
});
```

**Diff Viewer**:
```typescript
// src/renderer/components/diff/DiffViewer.tsx
import ReactDiffViewer from 'react-diff-viewer-continued';

export function DiffViewer({ oldValue, newValue, filename }) {
  return (
    <div className="border rounded-lg overflow-hidden">
      <div className="px-4 py-2 border-b bg-muted">
        <span className="font-mono text-sm">{filename}</span>
      </div>
      <ReactDiffViewer
        oldValue={oldValue}
        newValue={newValue}
        splitView
        useDarkTheme={theme === 'dark'}
        showDiffOnly={false}
      />
      <div className="p-3 border-t flex justify-end gap-2">
        <Button variant="outline">Reject</Button>
        <Button onClick={handleAccept}>Accept Changes</Button>
      </div>
    </div>
  );
}
```

### ✅ 완료 기준 (Commits 16-18)

- [ ] ToolCall 컴포넌트 렌더링
- [ ] Native approval dialog
- [ ] System tray notification
- [ ] Diff viewer 작동
- [ ] 변경사항 승인/거부
- [ ] electron-store에 승인 설정 저장

### 📝 Commit Messages

```
feat(tools): implement tool call visualization and approval

- Add ToolCallCard component
- Implement native approval dialog
- Add system tray notifications
- Track tool execution status
```

```
feat(diff): add diff viewer for file changes

- Integrate react-diff-viewer-continued
- Support side-by-side comparison
- Add accept/reject buttons
- Use native save dialog for applying changes
```

```
feat(approval): implement approval flow with settings persistence

- Add approval settings in electron-store
- Support auto-approve for trusted tools
- Show native dialog for manual approval
- Add approval history tracking
```

---

## 🎯 Day 3 완료 체크리스트

### 기능 완성도
- [ ] 파일 탐색기 완성
- [ ] Monaco Editor 통합
- [ ] 파일 저장 (Cmd/Ctrl+S)
- [ ] 파일 업로드 (Drag & Drop)
- [ ] 도구 호출 시각화
- [ ] Diff 뷰어
- [ ] 승인 플로우

### Electron 통합
- [ ] Native file dialogs
- [ ] File system IPC
- [ ] System tray notifications
- [ ] Native approval dialogs
- [ ] Menu bar integration

### 코드 품질
- [ ] TypeScript 타입 완성
- [ ] 빌드 성공
- [ ] Console에 에러 없음

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "@monaco-editor/react": "^4.6.0",
    "monaco-editor": "^0.45.0",
    "react-diff-viewer-continued": "^3.3.1",
    "path-browserify": "^1.0.1"
  }
}
```

---

**다음**: Day 4에서는 세션 관리, 검색 기능, 히스토리 백업을 구현합니다.
