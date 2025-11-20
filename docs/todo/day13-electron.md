# Day 13 TODO - 성능 모니터링 및 분석 (Electron)

> **목표**: APM, 로깅, 에러 추적, 프로파일링으로 프로덕션 안정성 확보

## 전체 개요

Day 13은 Codex UI의 성능 모니터링 시스템을 완성합니다:
- CPU/Memory/Network 모니터링
- 실시간 성능 대시보드
- 구조화된 로깅 시스템
- Sentry 에러 추적
- React 프로파일링
- 자동 최적화 제안

**Electron 특화:**
- systeminformation으로 시스템 메트릭 수집
- Dock/Taskbar에 성능 인디케이터
- Native notification으로 성능 경고
- electron-log로 파일 로깅
- Crash reporter 통합
- Performance API 활용

---

## Commit 73: 메트릭 수집

### 📋 작업 내용

1. **CPU/Memory 모니터링**
2. **네트워크 트래픽 추적**
3. **API 레이턴시 측정**
4. **에러율 계산**

### 📁 파일 구조

```
src/main/monitoring/
├── MetricsCollector.ts   # 메트릭 수집기
├── SystemMetrics.ts      # 시스템 메트릭
└── types.ts              # 메트릭 타입

src/renderer/components/monitoring/
├── PerformanceWidget.tsx # 성능 위젯
└── MetricsChart.tsx      # 메트릭 차트

src/renderer/store/
└── useMetricsStore.ts    # 메트릭 상태
```

### 1️⃣ 메트릭 타입 정의

**파일**: `src/renderer/types/metrics.ts`

```typescript
export interface SystemMetrics {
  cpu: {
    usage: number; // 0-100
    temperature?: number;
    cores: number;
  };
  memory: {
    total: number; // bytes
    used: number;
    free: number;
    usagePercent: number;
  };
  disk: {
    total: number;
    used: number;
    free: number;
  };
  network: {
    sent: number; // bytes
    received: number;
    latency?: number; // ms
  };
}

export interface AppMetrics {
  startupTime: number; // ms
  messageCount: number;
  sessionCount: number;
  apiCalls: {
    total: number;
    success: number;
    error: number;
    avgLatency: number; // ms
  };
  errors: {
    total: number;
    rate: number; // errors per minute
    lastError?: {
      message: string;
      timestamp: number;
    };
  };
}

export interface PerformanceMetrics {
  fps: number;
  renderTime: number; // ms
  bundleSize: number; // bytes
  loadTime: number; // ms
  memoryLeaks: boolean;
}

export interface MetricsSnapshot {
  timestamp: number;
  system: SystemMetrics;
  app: AppMetrics;
  performance: PerformanceMetrics;
}
```

### 2️⃣ Metrics Collector

**파일**: `src/main/monitoring/MetricsCollector.ts`

```typescript
import si from 'systeminformation';
import { BrowserWindow } from 'electron';
import type { SystemMetrics } from '@/renderer/types/metrics';

export class MetricsCollector {
  private intervalId: NodeJS.Timeout | null = null;
  private window: BrowserWindow | null = null;

  constructor(window: BrowserWindow) {
    this.window = window;
  }

  start(interval = 5000): void {
    this.intervalId = setInterval(() => {
      this.collectMetrics();
    }, interval);
  }

  stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  private async collectMetrics(): Promise<void> {
    try {
      const metrics = await this.getSystemMetrics();

      // Send to renderer
      if (this.window) {
        this.window.webContents.send('metrics:update', metrics);
      }
    } catch (error) {
      console.error('Failed to collect metrics:', error);
    }
  }

  async getSystemMetrics(): Promise<SystemMetrics> {
    const [cpu, mem, disk, net] = await Promise.all([
      si.currentLoad(),
      si.mem(),
      si.fsSize(),
      si.networkStats(),
    ]);

    return {
      cpu: {
        usage: Math.round(cpu.currentLoad),
        temperature: cpu.cpus[0]?.temperature,
        cores: cpu.cpus.length,
      },
      memory: {
        total: mem.total,
        used: mem.used,
        free: mem.free,
        usagePercent: Math.round((mem.used / mem.total) * 100),
      },
      disk: {
        total: disk[0]?.size || 0,
        used: disk[0]?.used || 0,
        free: disk[0]?.available || 0,
      },
      network: {
        sent: net[0]?.tx_sec || 0,
        received: net[0]?.rx_sec || 0,
      },
    };
  }

  async getProcessMetrics(): Promise<{
    cpu: number;
    memory: number;
  }> {
    const metrics = await si.processes();
    const currentProcess = metrics.list.find(
      (p) => p.pid === process.pid
    );

    return {
      cpu: currentProcess?.cpu || 0,
      memory: currentProcess?.mem || 0,
    };
  }
}
```

### 3️⃣ App Metrics Tracker

**파일**: `src/renderer/services/metricsTracker.ts`

```typescript
import { create } from 'zustand';
import type { AppMetrics } from '@/types/metrics';

interface MetricsState {
  metrics: AppMetrics;
  apiCallStart: (endpoint: string) => string;
  apiCallEnd: (id: string, success: boolean, latency: number) => void;
  recordError: (error: Error) => void;
  incrementMessageCount: () => void;
  incrementSessionCount: () => void;
}

export const useMetricsTracker = create<MetricsState>((set, get) => ({
  metrics: {
    startupTime: 0,
    messageCount: 0,
    sessionCount: 0,
    apiCalls: {
      total: 0,
      success: 0,
      error: 0,
      avgLatency: 0,
    },
    errors: {
      total: 0,
      rate: 0,
    },
  },

  apiCallStart: (endpoint: string) => {
    const id = `${endpoint}-${Date.now()}`;
    // Store in map for tracking
    return id;
  },

  apiCallEnd: (id: string, success: boolean, latency: number) => {
    set((state) => {
      const { apiCalls } = state.metrics;
      const newTotal = apiCalls.total + 1;
      const newSuccess = success ? apiCalls.success + 1 : apiCalls.success;
      const newError = !success ? apiCalls.error + 1 : apiCalls.error;

      // Calculate new average latency
      const newAvgLatency =
        (apiCalls.avgLatency * apiCalls.total + latency) / newTotal;

      return {
        metrics: {
          ...state.metrics,
          apiCalls: {
            total: newTotal,
            success: newSuccess,
            error: newError,
            avgLatency: newAvgLatency,
          },
        },
      };
    });
  },

  recordError: (error: Error) => {
    set((state) => ({
      metrics: {
        ...state.metrics,
        errors: {
          total: state.metrics.errors.total + 1,
          rate: state.metrics.errors.rate, // Calculate in interval
          lastError: {
            message: error.message,
            timestamp: Date.now(),
          },
        },
      },
    }));
  },

  incrementMessageCount: () => {
    set((state) => ({
      metrics: {
        ...state.metrics,
        messageCount: state.metrics.messageCount + 1,
      },
    }));
  },

  incrementSessionCount: () => {
    set((state) => ({
      metrics: {
        ...state.metrics,
        sessionCount: state.metrics.sessionCount + 1,
      },
    }));
  },
}));

// Track startup time
if (typeof window !== 'undefined') {
  window.addEventListener('load', () => {
    const startupTime = performance.now();
    useMetricsTracker.setState((state) => ({
      metrics: {
        ...state.metrics,
        startupTime,
      },
    }));
  });
}
```

### 4️⃣ Performance Widget

**파일**: `src/renderer/components/monitoring/PerformanceWidget.tsx`

```typescript
import React, { useEffect, useState } from 'react';
import { Activity, Cpu, HardDrive, Network } from 'lucide-react';
import { Card } from '@/components/ui/card';
import type { SystemMetrics } from '@/types/metrics';

export function PerformanceWidget() {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);

  useEffect(() => {
    if (!window.electronAPI) return;

    // Listen for metrics updates
    window.electronAPI.on('metrics:update', (data: SystemMetrics) => {
      setMetrics(data);
    });

    // Request initial metrics
    window.electronAPI.getSystemMetrics().then(setMetrics);
  }, []);

  if (!metrics) return null;

  const items = [
    {
      icon: Cpu,
      label: 'CPU',
      value: `${metrics.cpu.usage}%`,
      color: metrics.cpu.usage > 80 ? 'text-red-500' : 'text-green-500',
    },
    {
      icon: Activity,
      label: 'Memory',
      value: `${metrics.memory.usagePercent}%`,
      color: metrics.memory.usagePercent > 80 ? 'text-red-500' : 'text-green-500',
    },
    {
      icon: HardDrive,
      label: 'Disk',
      value: formatBytes(metrics.disk.used),
      color: 'text-blue-500',
    },
    {
      icon: Network,
      label: 'Network',
      value: `${formatBytes(metrics.network.received)}/s`,
      color: 'text-purple-500',
    },
  ];

  return (
    <div className="grid grid-cols-4 gap-2">
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <Card key={item.label} className="p-3">
            <div className="flex items-center gap-2">
              <Icon className={`h-4 w-4 ${item.color}`} />
              <div className="flex-1 min-w-0">
                <p className="text-xs text-muted-foreground">{item.label}</p>
                <p className="text-sm font-semibold truncate">{item.value}</p>
              </div>
            </div>
          </Card>
        );
      })}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
```

### 5️⃣ IPC Handlers

**파일**: `src/main/handlers/metrics.ts`

```typescript
import { ipcMain } from 'electron';
import { MetricsCollector } from '../monitoring/MetricsCollector';

let metricsCollector: MetricsCollector;

export function registerMetricsHandlers(window: BrowserWindow) {
  metricsCollector = new MetricsCollector(window);

  // Start collecting
  ipcMain.handle('metrics:start', () => {
    metricsCollector.start();
  });

  // Stop collecting
  ipcMain.handle('metrics:stop', () => {
    metricsCollector.stop();
  });

  // Get current metrics
  ipcMain.handle('metrics:getSystem', async () => {
    return await metricsCollector.getSystemMetrics();
  });

  // Get process metrics
  ipcMain.handle('metrics:getProcess', async () => {
    return await metricsCollector.getProcessMetrics();
  });
}
```

### ✅ 완료 기준

- [ ] 시스템 메트릭 수집
- [ ] 앱 메트릭 추적
- [ ] 실시간 업데이트
- [ ] 성능 위젯 표시
- [ ] Dock/Taskbar 인디케이터

### 📝 Commit Message

```
feat(monitoring): implement metrics collection system

- Add MetricsCollector with systeminformation
- Track CPU, Memory, Disk, Network usage
- Collect app metrics (API calls, errors)
- Create PerformanceWidget for real-time display
- Send metrics updates via IPC

Electron-specific:
- Use systeminformation for system metrics
- Track process-level metrics
- Update dock/taskbar badge
```

---

## Commit 74: 성능 대시보드

### 📋 작업 내용

1. **실시간 차트 (Chart.js)**
2. **성능 트렌드 분석**
3. **병목 지점 식별**
4. **알림 설정**

### 핵심 코드

**파일**: `src/renderer/components/monitoring/PerformanceDashboard.tsx`

```typescript
import React, { useEffect, useState } from 'react';
import { Line } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';
import type { MetricsSnapshot } from '@/types/metrics';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend
);

export function PerformanceDashboard() {
  const [snapshots, setSnapshots] = useState<MetricsSnapshot[]>([]);

  useEffect(() => {
    if (!window.electronAPI) return;

    window.electronAPI.on('metrics:update', (metrics: any) => {
      setSnapshots((prev) => {
        const updated = [
          ...prev,
          {
            timestamp: Date.now(),
            system: metrics,
            app: {}, // TODO: Add app metrics
            performance: {}, // TODO: Add perf metrics
          },
        ];

        // Keep last 60 data points (5 minutes at 5s intervals)
        return updated.slice(-60);
      });
    });
  }, []);

  const chartData = {
    labels: snapshots.map((s) => new Date(s.timestamp).toLocaleTimeString()),
    datasets: [
      {
        label: 'CPU Usage (%)',
        data: snapshots.map((s) => s.system.cpu.usage),
        borderColor: 'rgb(255, 99, 132)',
        backgroundColor: 'rgba(255, 99, 132, 0.5)',
      },
      {
        label: 'Memory Usage (%)',
        data: snapshots.map((s) => s.system.memory.usagePercent),
        borderColor: 'rgb(53, 162, 235)',
        backgroundColor: 'rgba(53, 162, 235, 0.5)',
      },
    ],
  };

  return (
    <div className="p-4 space-y-4">
      <h2 className="text-lg font-semibold">Performance Dashboard</h2>
      <div className="h-64">
        <Line data={chartData} options={{ responsive: true, maintainAspectRatio: false }} />
      </div>
    </div>
  );
}
```

### ✅ 완료 기준

- [ ] 실시간 차트 표시
- [ ] 트렌드 분석
- [ ] 병목 지점 하이라이트
- [ ] 알림 임계값 설정

### 📝 Commit Message

```
feat(monitoring): add performance dashboard with charts

- Integrate Chart.js for real-time visualization
- Display CPU, Memory, Network trends
- Identify performance bottlenecks
- Add configurable alert thresholds
- Show historical data (last 5 minutes)
```

---

## Commit 75: 로깅 시스템

### 📋 작업 내용

1. **electron-log 통합**
2. **구조화된 로깅**
3. **로그 레벨 관리**
4. **로그 검색**

### 핵심 코드

**파일**: `src/main/logging/Logger.ts`

```typescript
import log from 'electron-log';
import path from 'path';
import { app } from 'electron';

// Configure electron-log
log.transports.file.resolvePathFn = () =>
  path.join(app.getPath('userData'), 'logs', 'main.log');

log.transports.file.level = 'info';
log.transports.console.level = 'debug';

// Customize format
log.transports.file.format = '[{y}-{m}-{d} {h}:{i}:{s}.{ms}] [{level}] {text}';

export class Logger {
  private context: string;

  constructor(context: string) {
    this.context = context;
  }

  debug(message: string, ...args: any[]): void {
    log.debug(`[${this.context}] ${message}`, ...args);
  }

  info(message: string, ...args: any[]): void {
    log.info(`[${this.context}] ${message}`, ...args);
  }

  warn(message: string, ...args: any[]): void {
    log.warn(`[${this.context}] ${message}`, ...args);
  }

  error(message: string, error?: Error, ...args: any[]): void {
    log.error(`[${this.context}] ${message}`, error, ...args);
  }

  // Structured logging
  log(level: 'debug' | 'info' | 'warn' | 'error', data: Record<string, any>): void {
    log[level](`[${this.context}]`, JSON.stringify(data));
  }
}

export const logger = new Logger('Main');
```

### ✅ 완료 기준

- [ ] electron-log 통합
- [ ] 구조화된 로그
- [ ] 로그 파일 rotation
- [ ] 로그 뷰어 UI

### 📝 Commit Message

```
feat(monitoring): implement structured logging with electron-log

- Configure electron-log for file and console
- Add Logger class with context
- Support structured logging (JSON)
- Implement log level filtering
- Add log viewer UI in settings
```

---

## Commits 76-78: Sentry, 프로파일링, 최적화

*Remaining commits summarized*

### Commit 76: Sentry 에러 추적
- @sentry/electron 통합
- Source maps 업로드
- Release tracking
- User feedback

**Sentry 설정**:
```typescript
import * as Sentry from '@sentry/electron';

Sentry.init({
  dsn: process.env.SENTRY_DSN,
  release: app.getVersion(),
  environment: process.env.NODE_ENV,
  beforeSend(event) {
    // Filter sensitive data
    return event;
  },
});
```

### Commit 77: React 프로파일링
- React DevTools 통합
- Render performance 측정
- Component tree analysis
- Memory leak 감지

### Commit 78: 자동 최적화
- Bundle analyzer
- Code splitting 제안
- Image optimization
- Lazy loading 권장

---

## 🎯 Day 13 완료 체크리스트

### 기능 완성도
- [ ] 메트릭 수집 작동
- [ ] 성능 대시보드 표시
- [ ] 로깅 시스템 완성
- [ ] Sentry 에러 추적
- [ ] 프로파일링 도구
- [ ] 최적화 제안

### Electron 통합
- [ ] systeminformation 수집
- [ ] electron-log 파일 저장
- [ ] Sentry crash reporter
- [ ] Dock badge 업데이트

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "@sentry/electron": "^4.15.0",
    "electron-log": "^5.0.3",
    "systeminformation": "^5.21.20",
    "chart.js": "^4.4.1",
    "react-chartjs-2": "^5.2.0"
  }
}
```

---

**다음**: Day 14에서는 UI/UX 폴리싱 및 최종 완성을 진행합니다.
