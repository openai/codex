# Day 9 TODO - 멀티모달 지원 (Electron)

> **목표**: 이미지, PDF, 파일 첨부 및 스크린샷 캡처 기능으로 멀티모달 AI 경험 제공

## 전체 개요

Day 9는 Codex UI에 멀티모달 입력을 완성합니다:
- 이미지 업로드 및 처리 (압축, 썸네일)
- OCR (Tesseract.js)
- PDF 파싱 및 미리보기
- 다중 파일 첨부 시스템
- 스크린샷 캡처 (desktopCapturer)
- 미디어 갤러리 및 Lightbox

**Electron 특화:**
- Native file picker로 다중 파일 선택
- desktopCapturer API로 스크린샷
- Native clipboard 통합
- 파일 시스템 접근 (썸네일 저장)
- Native notification (OCR 완료, 파일 업로드)
- Progress bar (파일 처리)

---

## Commit 49: 이미지 업로드 및 처리

### 📋 작업 내용

1. **Drag & Drop 이미지 업로드**
2. **이미지 압축 (sharp)**
3. **썸네일 생성**
4. **EXIF 데이터 추출**
5. **이미지 메타데이터 저장**

### 📁 파일 구조

```
src/main/handlers/
└── image.ts              # 이미지 처리 IPC

src/renderer/components/media/
├── ImageUpload.tsx       # 이미지 업로드
├── ImagePreview.tsx      # 이미지 미리보기
└── ImageMetadata.tsx     # EXIF 정보 표시

src/renderer/store/
└── useMediaStore.ts      # 미디어 상태 관리

src/renderer/types/
└── media.ts              # 미디어 타입
```

### 1️⃣ 미디어 타입 정의

**파일**: `src/renderer/types/media.ts`

```typescript
export interface ImageMetadata {
  width: number;
  height: number;
  format: string;
  size: number;
  exif?: {
    make?: string;
    model?: string;
    dateTime?: string;
    gps?: {
      latitude?: number;
      longitude?: number;
    };
  };
}

export interface MediaFile {
  id: string;
  type: 'image' | 'pdf' | 'file';
  name: string;
  path: string;
  size: number;
  mimeType: string;
  thumbnail?: string;
  metadata?: ImageMetadata;
  uploadedAt: number;
  messageId?: string;
  ocrText?: string;
  pdfPages?: number;
}

export interface UploadProgress {
  id: string;
  fileName: string;
  progress: number;
  status: 'pending' | 'uploading' | 'processing' | 'completed' | 'error';
  error?: string;
}
```

### 2️⃣ 이미지 처리 IPC Handler

**파일**: `src/main/handlers/image.ts`

```typescript
import { ipcMain, dialog } from 'electron';
import { BrowserWindow } from 'electron';
import sharp from 'sharp';
import fs from 'fs/promises';
import path from 'path';
import { app } from 'electron';

const THUMBNAILS_DIR = path.join(app.getPath('userData'), 'thumbnails');
const MAX_IMAGE_SIZE = 1920; // Max width/height
const THUMBNAIL_SIZE = 200;

// Ensure thumbnails directory exists
async function ensureThumbnailsDir() {
  try {
    await fs.mkdir(THUMBNAILS_DIR, { recursive: true });
  } catch (error) {
    console.error('Failed to create thumbnails directory:', error);
  }
}

export function registerImageHandlers() {
  ensureThumbnailsDir();

  // Select images
  ipcMain.handle('image:select', async () => {
    const window = BrowserWindow.getFocusedWindow();
    if (!window) return null;

    const result = await dialog.showOpenDialog(window, {
      properties: ['openFile', 'multiSelections'],
      filters: [
        {
          name: 'Images',
          extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff'],
        },
      ],
    });

    return result.canceled ? null : result.filePaths;
  });

  // Process image
  ipcMain.handle('image:process', async (_event, imagePath: string) => {
    try {
      const image = sharp(imagePath);
      const metadata = await image.metadata();

      // Generate thumbnail
      const thumbnailName = `thumb_${Date.now()}_${path.basename(imagePath)}`;
      const thumbnailPath = path.join(THUMBNAILS_DIR, thumbnailName);

      await sharp(imagePath)
        .resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, {
          fit: 'cover',
          position: 'center',
        })
        .jpeg({ quality: 80 })
        .toFile(thumbnailPath);

      // Compress if needed
      let processedPath = imagePath;
      if (
        metadata.width &&
        metadata.height &&
        (metadata.width > MAX_IMAGE_SIZE || metadata.height > MAX_IMAGE_SIZE)
      ) {
        const compressedName = `compressed_${Date.now()}_${path.basename(imagePath)}`;
        const compressedPath = path.join(THUMBNAILS_DIR, compressedName);

        await sharp(imagePath)
          .resize(MAX_IMAGE_SIZE, MAX_IMAGE_SIZE, {
            fit: 'inside',
            withoutEnlargement: true,
          })
          .jpeg({ quality: 85 })
          .toFile(compressedPath);

        processedPath = compressedPath;
      }

      // Extract EXIF
      const exifData = metadata.exif
        ? {
            make: metadata.exif.Make?.toString(),
            model: metadata.exif.Model?.toString(),
            dateTime: metadata.exif.DateTime?.toString(),
          }
        : undefined;

      return {
        originalPath: imagePath,
        processedPath,
        thumbnailPath,
        metadata: {
          width: metadata.width,
          height: metadata.height,
          format: metadata.format,
          size: (await fs.stat(processedPath)).size,
          exif: exifData,
        },
      };
    } catch (error) {
      console.error('Failed to process image:', error);
      throw error;
    }
  });

  // Convert to base64
  ipcMain.handle('image:toBase64', async (_event, imagePath: string) => {
    try {
      const buffer = await fs.readFile(imagePath);
      const base64 = buffer.toString('base64');
      const ext = path.extname(imagePath).slice(1);
      return `data:image/${ext};base64,${base64}`;
    } catch (error) {
      console.error('Failed to convert image to base64:', error);
      throw error;
    }
  });

  // Get image from clipboard
  ipcMain.handle('image:fromClipboard', async () => {
    const { clipboard, nativeImage } = await import('electron');
    const image = clipboard.readImage();

    if (image.isEmpty()) {
      return null;
    }

    const buffer = image.toPNG();
    const fileName = `clipboard_${Date.now()}.png`;
    const filePath = path.join(THUMBNAILS_DIR, fileName);

    await fs.writeFile(filePath, buffer);

    return filePath;
  });
}
```

### 3️⃣ Media Store

**파일**: `src/renderer/store/useMediaStore.ts`

```typescript
import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';
import type { MediaFile, UploadProgress } from '@/types/media';
import { nanoid } from 'nanoid';

interface MediaState {
  files: Map<string, MediaFile>;
  uploads: Map<string, UploadProgress>;
  selectedFileId: string | null;
}

interface MediaActions {
  uploadImages: (imagePaths: string[]) => Promise<MediaFile[]>;
  uploadFromClipboard: () => Promise<MediaFile | null>;
  removeFile: (id: string) => void;
  selectFile: (id: string | null) => void;
  getFilesByMessage: (messageId: string) => MediaFile[];
}

export const useMediaStore = create<MediaState & MediaActions>()(
  devtools(
    immer((set, get) => ({
      files: new Map(),
      uploads: new Map(),
      selectedFileId: null,

      uploadImages: async (imagePaths: string[]) => {
        if (!window.electronAPI) return [];

        const uploadedFiles: MediaFile[] = [];

        for (const imagePath of imagePaths) {
          const uploadId = nanoid();

          // Add to uploads
          set((state) => {
            state.uploads.set(uploadId, {
              id: uploadId,
              fileName: imagePath.split('/').pop() || 'unknown',
              progress: 0,
              status: 'processing',
            });
          });

          try {
            // Process image
            const result = await window.electronAPI.processImage(imagePath);

            // Update progress
            set((state) => {
              const upload = state.uploads.get(uploadId);
              if (upload) {
                upload.progress = 50;
              }
            });

            // Convert to base64 for display
            const base64 = await window.electronAPI.imageToBase64(result.processedPath);

            // Create media file
            const mediaFile: MediaFile = {
              id: nanoid(),
              type: 'image',
              name: imagePath.split('/').pop() || 'unknown',
              path: result.processedPath,
              size: result.metadata.size,
              mimeType: `image/${result.metadata.format}`,
              thumbnail: result.thumbnailPath,
              metadata: result.metadata,
              uploadedAt: Date.now(),
            };

            // Add to files
            set((state) => {
              state.files.set(mediaFile.id, mediaFile);

              // Update upload progress
              const upload = state.uploads.get(uploadId);
              if (upload) {
                upload.progress = 100;
                upload.status = 'completed';
              }
            });

            uploadedFiles.push(mediaFile);

            // Remove upload after delay
            setTimeout(() => {
              set((state) => {
                state.uploads.delete(uploadId);
              });
            }, 2000);
          } catch (error) {
            console.error('Failed to upload image:', error);

            set((state) => {
              const upload = state.uploads.get(uploadId);
              if (upload) {
                upload.status = 'error';
                upload.error = (error as Error).message;
              }
            });
          }
        }

        return uploadedFiles;
      },

      uploadFromClipboard: async () => {
        if (!window.electronAPI) return null;

        try {
          const imagePath = await window.electronAPI.imageFromClipboard();
          if (!imagePath) return null;

          const files = await get().uploadImages([imagePath]);
          return files[0] || null;
        } catch (error) {
          console.error('Failed to upload from clipboard:', error);
          return null;
        }
      },

      removeFile: (id: string) => {
        set((state) => {
          state.files.delete(id);
          if (state.selectedFileId === id) {
            state.selectedFileId = null;
          }
        });
      },

      selectFile: (id: string | null) => {
        set((state) => {
          state.selectedFileId = id;
        });
      },

      getFilesByMessage: (messageId: string) => {
        const { files } = get();
        return Array.from(files.values()).filter((f) => f.messageId === messageId);
      },
    }))
  )
);
```

### 4️⃣ ImageUpload Component

**파일**: `src/renderer/components/media/ImageUpload.tsx`

```typescript
import React, { useCallback, useState } from 'react';
import { Upload, Image as ImageIcon, Clipboard } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useMediaStore } from '@/store/useMediaStore';
import { toast } from 'react-hot-toast';

interface ImageUploadProps {
  onUpload?: (fileIds: string[]) => void;
  className?: string;
}

export function ImageUpload({ onUpload, className }: ImageUploadProps) {
  const { uploadImages, uploadFromClipboard, uploads } = useMediaStore();
  const [isDragging, setIsDragging] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragging(false);

      const files = Array.from(e.dataTransfer.files);
      const imagePaths = files
        .filter((f) => f.type.startsWith('image/'))
        .map((f) => f.path);

      if (imagePaths.length === 0) {
        toast.error('No images found in drop');
        return;
      }

      const uploadedFiles = await uploadImages(imagePaths);
      onUpload?.(uploadedFiles.map((f) => f.id));
      toast.success(`Uploaded ${uploadedFiles.length} image(s)`);
    },
    [uploadImages, onUpload]
  );

  const handleSelectImages = async () => {
    if (!window.electronAPI) return;

    const imagePaths = await window.electronAPI.selectImages();
    if (!imagePaths || imagePaths.length === 0) return;

    const uploadedFiles = await uploadImages(imagePaths);
    onUpload?.(uploadedFiles.map((f) => f.id));
    toast.success(`Uploaded ${uploadedFiles.length} image(s)`);
  };

  const handlePasteFromClipboard = async () => {
    const file = await uploadFromClipboard();
    if (file) {
      onUpload?.([file.id]);
      toast.success('Pasted image from clipboard');
    } else {
      toast.error('No image in clipboard');
    }
  };

  const uploadArray = Array.from(uploads.values());
  const hasUploads = uploadArray.length > 0;

  return (
    <div className={className}>
      {/* Drop zone */}
      <div
        className={cn(
          'border-2 border-dashed rounded-lg p-6 text-center transition-colors',
          isDragging
            ? 'border-primary bg-primary/10'
            : 'border-muted-foreground/25 hover:border-primary/50'
        )}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <ImageIcon className="h-10 w-10 mx-auto mb-3 text-muted-foreground" />
        <p className="text-sm font-medium mb-1">Drop images here</p>
        <p className="text-xs text-muted-foreground mb-4">
          or click to browse
        </p>
        <div className="flex gap-2 justify-center">
          <Button onClick={handleSelectImages}>
            <Upload className="h-4 w-4 mr-2" />
            Select Images
          </Button>
          <Button variant="outline" onClick={handlePasteFromClipboard}>
            <Clipboard className="h-4 w-4 mr-2" />
            Paste
          </Button>
        </div>
      </div>

      {/* Upload progress */}
      {hasUploads && (
        <div className="mt-4 space-y-2">
          {uploadArray.map((upload) => (
            <div
              key={upload.id}
              className="flex items-center gap-3 p-3 rounded-lg border bg-card"
            >
              <div className="flex-1">
                <p className="text-sm font-medium truncate">{upload.fileName}</p>
                <div className="flex items-center gap-2 mt-1">
                  <div className="flex-1 h-1 bg-muted rounded-full overflow-hidden">
                    <div
                      className="h-full bg-primary transition-all"
                      style={{ width: `${upload.progress}%` }}
                    />
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {upload.progress}%
                  </span>
                </div>
                {upload.error && (
                  <p className="text-xs text-destructive mt-1">{upload.error}</p>
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

### 5️⃣ IPC 타입 확장

**파일**: `src/preload/index.d.ts` (확장)

```typescript
export interface ElectronAPI {
  // ... 기존 메서드들 ...

  // Image operations
  selectImages: () => Promise<string[] | null>;
  processImage: (imagePath: string) => Promise<{
    originalPath: string;
    processedPath: string;
    thumbnailPath: string;
    metadata: ImageMetadata;
  }>;
  imageToBase64: (imagePath: string) => Promise<string>;
  imageFromClipboard: () => Promise<string | null>;
}
```

### ✅ 완료 기준

- [ ] 이미지 drag & drop 작동
- [ ] sharp로 이미지 압축
- [ ] 썸네일 생성
- [ ] EXIF 데이터 추출
- [ ] 클립보드에서 붙여넣기
- [ ] Native file picker

### 📝 Commit Message

```
feat(media): implement image upload with compression and thumbnails

- Add drag & drop image upload
- Implement image compression with sharp
- Generate thumbnails (200x200)
- Extract EXIF metadata
- Support clipboard paste
- Use native file picker for multi-selection
- Show upload progress

Electron-specific:
- Native file dialogs for image selection
- File system access for thumbnail storage
- Clipboard API for paste
- IPC for image processing
```

---

## Commit 50: 이미지 OCR

### 📋 작업 내용

1. **Tesseract.js 통합**
2. **OCR 실행 UI**
3. **다국어 지원**
4. **OCR 결과 저장**

### 1️⃣ OCR Handler

**파일**: `src/main/handlers/ocr.ts`

```typescript
import { ipcMain } from 'electron';
import { createWorker } from 'tesseract.js';
import path from 'path';
import { app } from 'electron';

const TESSDATA_DIR = path.join(app.getPath('userData'), 'tessdata');

export function registerOCRHandlers() {
  // Run OCR
  ipcMain.handle('ocr:recognize', async (_event, imagePath: string, lang = 'eng') => {
    try {
      const worker = await createWorker(lang, 1, {
        langPath: TESSDATA_DIR,
        logger: (m) => {
          // Send progress to renderer
          if (m.status === 'recognizing text') {
            _event.sender.send('ocr:progress', {
              progress: m.progress,
              status: m.status,
            });
          }
        },
      });

      const {
        data: { text, confidence },
      } = await worker.recognize(imagePath);

      await worker.terminate();

      return {
        text,
        confidence,
      };
    } catch (error) {
      console.error('OCR failed:', error);
      throw error;
    }
  });

  // Get available languages
  ipcMain.handle('ocr:getLanguages', () => {
    return [
      { code: 'eng', name: 'English' },
      { code: 'kor', name: 'Korean' },
      { code: 'jpn', name: 'Japanese' },
      { code: 'chi_sim', name: 'Chinese (Simplified)' },
      { code: 'chi_tra', name: 'Chinese (Traditional)' },
      { code: 'spa', name: 'Spanish' },
      { code: 'fra', name: 'French' },
      { code: 'deu', name: 'German' },
    ];
  });
}
```

### 2️⃣ OCR Component

**파일**: `src/renderer/components/media/OCRButton.tsx`

```typescript
import React, { useState } from 'react';
import { ScanText, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { useMediaStore } from '@/store/useMediaStore';
import { toast } from 'react-hot-toast';

interface OCRButtonProps {
  fileId: string;
}

export function OCRButton({ fileId }: OCRButtonProps) {
  const [isRunning, setIsRunning] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [ocrText, setOcrText] = useState('');
  const [confidence, setConfidence] = useState(0);
  const [language, setLanguage] = useState('eng');
  const [progress, setProgress] = useState(0);

  const { files } = useMediaStore();
  const file = files.get(fileId);

  if (!file || file.type !== 'image') return null;

  const handleRunOCR = async () => {
    if (!window.electronAPI) return;

    setIsRunning(true);
    setProgress(0);

    try {
      // Listen for progress
      window.electronAPI.on('ocr:progress', (data: any) => {
        setProgress(Math.round(data.progress * 100));
      });

      const result = await window.electronAPI.runOCR(file.path, language);

      setOcrText(result.text);
      setConfidence(result.confidence);
      setDialogOpen(true);

      // Save OCR result
      useMediaStore.setState((state) => {
        const f = state.files.get(fileId);
        if (f) {
          f.ocrText = result.text;
        }
      });

      toast.success('OCR completed');

      // Show native notification
      if (window.electronAPI) {
        window.electronAPI.showNotification(
          'OCR Complete',
          `Extracted ${result.text.length} characters with ${Math.round(result.confidence)}% confidence`
        );
      }
    } catch (error) {
      toast.error('OCR failed');
      console.error('OCR error:', error);
    } finally {
      setIsRunning(false);
      setProgress(0);
    }
  };

  return (
    <>
      <Button
        variant="outline"
        size="sm"
        onClick={handleRunOCR}
        disabled={isRunning}
      >
        {isRunning ? (
          <>
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            {progress}%
          </>
        ) : (
          <>
            <ScanText className="h-4 w-4 mr-2" />
            Run OCR
          </>
        )}
      </Button>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>OCR Result</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div>
              <label className="text-sm font-medium">Confidence</label>
              <p className="text-2xl font-bold">{Math.round(confidence)}%</p>
            </div>
            <div>
              <label className="text-sm font-medium">Extracted Text</label>
              <textarea
                className="w-full h-64 p-3 rounded-lg border bg-muted/50 font-mono text-sm"
                value={ocrText}
                readOnly
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  navigator.clipboard.writeText(ocrText);
                  toast.success('Copied to clipboard');
                }}
              >
                Copy
              </Button>
              <Button onClick={() => setDialogOpen(false)}>Close</Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
```

### ✅ 완료 기준

- [ ] Tesseract.js OCR 작동
- [ ] 진행률 표시
- [ ] 다국어 지원 (8개 언어)
- [ ] OCR 결과 저장
- [ ] Native notification

### 📝 Commit Message

```
feat(media): add OCR support with Tesseract.js

- Integrate Tesseract.js for OCR
- Support 8 languages (eng, kor, jpn, chi_sim, etc.)
- Show real-time progress during recognition
- Display confidence score
- Save OCR text with media file
- Native notification on completion

Features:
- Copy OCR text to clipboard
- Multi-language support
```

---

## Commits 51-54: PDF, 파일 첨부, 스크린샷, 갤러리

*Remaining commits summarized for brevity*

### Commit 51: PDF 처리
- PDF.js 통합
- 페이지별 미리보기
- 텍스트 추출
- PDF to images conversion

### Commit 52: 파일 첨부 시스템
- 다중 파일 첨부
- 파일 타입 감지 및 아이콘
- 파일 크기 제한
- 첨부 파일 관리

### Commit 53: 스크린샷 캡처
- desktopCapturer API 사용
- 전체 화면 / 영역 선택 캡처
- 클립보드 복사
- 주석 기능 (선택사항)

**핵심 코드 - desktopCapturer**:
```typescript
// src/main/handlers/screenshot.ts
import { ipcMain, desktopCapturer } from 'electron';

ipcMain.handle('screenshot:getSources', async () => {
  const sources = await desktopCapturer.getSources({
    types: ['window', 'screen'],
    thumbnailSize: { width: 1920, height: 1080 },
  });

  return sources.map(source => ({
    id: source.id,
    name: source.name,
    thumbnail: source.thumbnail.toDataURL(),
  }));
});
```

### Commit 54: 미디어 갤러리
- 그리드 레이아웃
- Lightbox 뷰어
- 이미지 확대/축소
- 다운로드 기능

---

## 🎯 Day 9 완료 체크리스트

### 기능 완성도
- [ ] 이미지 업로드 (drag & drop)
- [ ] 이미지 압축 및 썸네일
- [ ] OCR 지원 (8개 언어)
- [ ] PDF 미리보기
- [ ] 파일 첨부 시스템
- [ ] 스크린샷 캡처
- [ ] 미디어 갤러리

### Electron 통합
- [ ] Native file picker
- [ ] desktopCapturer API
- [ ] Clipboard 통합
- [ ] Native notifications
- [ ] 파일 시스템 접근

### 성능
- [ ] 이미지 압축 < 2MB
- [ ] 썸네일 생성 < 500ms
- [ ] OCR 처리 < 5초

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "sharp": "^0.33.0",
    "tesseract.js": "^5.0.0",
    "pdfjs-dist": "^4.0.379"
  }
}
```

---

**다음**: Day 10에서는 고급 도구 및 워크플로우 자동화를 구현합니다.
