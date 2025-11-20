# Day 14 TODO - UI/UX 폴리싱 및 최종 완성 (Electron)

> **목표**: 애니메이션, 온보딩, 접근성, 다국어 지원으로 프로덕션 완성

## 전체 개요

Day 14는 Codex UI를 프로덕션급으로 완성합니다:
- Framer Motion 애니메이션
- 첫 실행 온보딩
- WCAG 2.1 AA 접근성
- i18n 다국어 지원
- 최종 버그 수정
- 릴리스 준비

**Electron 특화:**
- Native splash screen
- First-run detection
- System language auto-detect
- Native menu 다국어
- Touch Bar 지원 (macOS)
- Accessibility API 통합

---

## Commit 79: 애니메이션 및 트랜지션

### 📋 작업 내용

1. **Framer Motion 통합**
2. **페이지 전환 애니메이션**
3. **마이크로 인터랙션**
4. **로딩 상태 애니메이션**

### 📁 파일 구조

```
src/renderer/components/animations/
├── PageTransition.tsx    # 페이지 전환
├── FadeIn.tsx            # Fade in 애니메이션
└── SlideIn.tsx           # Slide in 애니메이션

src/renderer/lib/
└── animations.ts         # 애니메이션 variants
```

### 1️⃣ 애니메이션 Variants

**파일**: `src/renderer/lib/animations.ts`

```typescript
import { Variants } from 'framer-motion';

export const fadeIn: Variants = {
  initial: { opacity: 0 },
  animate: { opacity: 1 },
  exit: { opacity: 0 },
};

export const slideIn: Variants = {
  initial: { x: -20, opacity: 0 },
  animate: { x: 0, opacity: 1 },
  exit: { x: 20, opacity: 0 },
};

export const scaleIn: Variants = {
  initial: { scale: 0.95, opacity: 0 },
  animate: { scale: 1, opacity: 1 },
  exit: { scale: 0.95, opacity: 0 },
};

export const slideUp: Variants = {
  initial: { y: 20, opacity: 0 },
  animate: { y: 0, opacity: 1 },
  exit: { y: -20, opacity: 0 },
};

export const staggerChildren: Variants = {
  animate: {
    transition: {
      staggerChildren: 0.1,
    },
  },
};

export const listItem: Variants = {
  initial: { x: -10, opacity: 0 },
  animate: { x: 0, opacity: 1 },
};
```

### 2️⃣ Page Transition

**파일**: `src/renderer/components/animations/PageTransition.tsx`

```typescript
import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useLocation } from 'react-router-dom';

interface PageTransitionProps {
  children: React.ReactNode;
}

export function PageTransition({ children }: PageTransitionProps) {
  const location = useLocation();

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={location.pathname}
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -20 }}
        transition={{
          type: 'spring',
          stiffness: 260,
          damping: 20,
        }}
      >
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
```

### 3️⃣ 마이크로 인터랙션

**파일**: `src/renderer/components/animations/MicroInteractions.tsx`

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { Button } from '@/components/ui/button';

// Animated button with hover effect
export function AnimatedButton({ children, ...props }: any) {
  return (
    <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }}>
      <Button {...props}>{children}</Button>
    </motion.div>
  );
}

// Pulse animation for notifications
export function PulseIndicator() {
  return (
    <motion.div
      className="w-2 h-2 rounded-full bg-primary"
      animate={{
        scale: [1, 1.2, 1],
        opacity: [1, 0.8, 1],
      }}
      transition={{
        duration: 2,
        repeat: Infinity,
      }}
    />
  );
}

// Typing indicator
export function TypingIndicator() {
  return (
    <div className="flex gap-1">
      {[0, 1, 2].map((i) => (
        <motion.div
          key={i}
          className="w-2 h-2 rounded-full bg-muted-foreground"
          animate={{
            y: [0, -10, 0],
          }}
          transition={{
            duration: 0.6,
            repeat: Infinity,
            delay: i * 0.1,
          }}
        />
      ))}
    </div>
  );
}

// Success checkmark animation
export function SuccessCheckmark() {
  return (
    <motion.svg
      className="w-16 h-16 text-green-500"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      initial={{ pathLength: 0, opacity: 0 }}
      animate={{ pathLength: 1, opacity: 1 }}
      transition={{ duration: 0.5 }}
    >
      <motion.path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={2}
        d="M5 13l4 4L19 7"
      />
    </motion.svg>
  );
}
```

### 4️⃣ Loading States

**파일**: `src/renderer/components/animations/LoadingStates.tsx`

```typescript
import React from 'react';
import { motion } from 'framer-motion';

export function SkeletonLoader() {
  return (
    <div className="space-y-3">
      {[...Array(3)].map((_, i) => (
        <motion.div
          key={i}
          className="h-12 bg-muted rounded-lg"
          animate={{
            opacity: [0.5, 1, 0.5],
          }}
          transition={{
            duration: 1.5,
            repeat: Infinity,
            delay: i * 0.2,
          }}
        />
      ))}
    </div>
  );
}

export function SpinnerLoader() {
  return (
    <motion.div
      className="w-8 h-8 border-4 border-primary border-t-transparent rounded-full"
      animate={{ rotate: 360 }}
      transition={{
        duration: 1,
        repeat: Infinity,
        ease: 'linear',
      }}
    />
  );
}
```

### ✅ 완료 기준

- [ ] Framer Motion 통합
- [ ] 페이지 전환 애니메이션
- [ ] 버튼 호버 효과
- [ ] 로딩 스켈레톤
- [ ] 마이크로 인터랙션

### 📝 Commit Message

```
feat(ui): add animations with Framer Motion

- Integrate Framer Motion for smooth animations
- Add page transition animations
- Implement micro-interactions (hover, tap)
- Create loading states (skeleton, spinner)
- Add success/error animations

Animations:
- Fade in/out
- Slide transitions
- Scale effects
- Stagger children
```

---

## Commit 80: 온보딩 플로우

### 📋 작업 내용

1. **첫 실행 감지**
2. **튜토리얼 단계**
3. **기능 소개**
4. **샘플 프로젝트**

### 1️⃣ First Run Detection

**파일**: `src/main/handlers/onboarding.ts`

```typescript
import { ipcMain } from 'electron';
import Store from 'electron-store';

const store = new Store();

export function registerOnboardingHandlers() {
  ipcMain.handle('onboarding:isFirstRun', () => {
    return !store.get('hasCompletedOnboarding', false);
  });

  ipcMain.handle('onboarding:complete', () => {
    store.set('hasCompletedOnboarding', true);
  });

  ipcMain.handle('onboarding:reset', () => {
    store.delete('hasCompletedOnboarding');
  });
}
```

### 2️⃣ Onboarding Wizard

**파일**: `src/renderer/components/onboarding/OnboardingWizard.tsx`

```typescript
import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent } from '@/components/ui/dialog';

interface OnboardingStep {
  title: string;
  description: string;
  image?: string;
  component?: React.ComponentType;
}

const steps: OnboardingStep[] = [
  {
    title: 'Welcome to Codex UI',
    description: 'Your AI-powered coding assistant with a beautiful desktop interface.',
  },
  {
    title: 'Chat with Claude',
    description: 'Get instant help with coding, debugging, and architecture decisions.',
  },
  {
    title: 'File Management',
    description: 'Browse, edit, and manage files directly in the app with Monaco Editor.',
  },
  {
    title: 'MCP Integration',
    description: 'Connect to MCP servers for extended context and capabilities.',
  },
  {
    title: 'Customize Everything',
    description: 'Themes, plugins, and settings to make Codex UI your own.',
  },
];

interface OnboardingWizardProps {
  open: boolean;
  onComplete: () => void;
}

export function OnboardingWizard({ open, onComplete }: OnboardingWizardProps) {
  const [currentStep, setCurrentStep] = useState(0);

  const handleNext = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      handleComplete();
    }
  };

  const handleComplete = async () => {
    if (window.electronAPI) {
      await window.electronAPI.completeOnboarding();
    }
    onComplete();
  };

  const step = steps[currentStep];

  return (
    <Dialog open={open} onOpenChange={() => {}}>
      <DialogContent className="max-w-2xl">
        <AnimatePresence mode="wait">
          <motion.div
            key={currentStep}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-6"
          >
            {/* Progress */}
            <div className="flex gap-2">
              {steps.map((_, i) => (
                <div
                  key={i}
                  className={`h-1 flex-1 rounded-full ${
                    i <= currentStep ? 'bg-primary' : 'bg-muted'
                  }`}
                />
              ))}
            </div>

            {/* Content */}
            <div className="text-center space-y-4">
              <h2 className="text-2xl font-bold">{step.title}</h2>
              <p className="text-muted-foreground">{step.description}</p>

              {step.image && (
                <img
                  src={step.image}
                  alt={step.title}
                  className="mx-auto max-h-64 rounded-lg"
                />
              )}

              {step.component && <step.component />}
            </div>

            {/* Navigation */}
            <div className="flex justify-between">
              <Button
                variant="outline"
                onClick={() => setCurrentStep(Math.max(0, currentStep - 1))}
                disabled={currentStep === 0}
              >
                Back
              </Button>
              <Button onClick={handleNext}>
                {currentStep === steps.length - 1 ? (
                  <>
                    <Check className="h-4 w-4 mr-2" />
                    Get Started
                  </>
                ) : (
                  <>
                    Next
                    <ChevronRight className="h-4 w-4 ml-2" />
                  </>
                )}
              </Button>
            </div>
          </motion.div>
        </AnimatePresence>
      </DialogContent>
    </Dialog>
  );
}
```

### ✅ 완료 기준

- [ ] First-run 감지
- [ ] 온보딩 단계 완성
- [ ] 진행률 표시
- [ ] 샘플 프로젝트 제공

### 📝 Commit Message

```
feat(onboarding): add first-run onboarding wizard

- Detect first run with electron-store
- Create multi-step onboarding wizard
- Add progress indicator
- Include feature highlights
- Provide sample project setup

Electron-specific:
- Store onboarding state
- Show on first app launch
```

---

## Commit 81: 접근성 개선

### 📋 작업 내용

1. **ARIA labels 완성**
2. **키보드 네비게이션**
3. **High contrast 테마**
4. **스크린 리더 지원**

### 핵심 구현

**파일**: `src/renderer/components/ui/accessible-button.tsx`

```typescript
import React from 'react';
import { Button } from '@/components/ui/button';

interface AccessibleButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  description?: string;
}

export function AccessibleButton({ label, description, ...props }: AccessibleButtonProps) {
  return (
    <Button
      aria-label={label}
      aria-describedby={description ? `${props.id}-desc` : undefined}
      {...props}
    >
      {props.children}
      {description && (
        <span id={`${props.id}-desc`} className="sr-only">
          {description}
        </span>
      )}
    </Button>
  );
}
```

**키보드 네비게이션**:
```typescript
// Global keyboard navigation hook
export function useKeyboardNavigation() {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Tab navigation
      if (e.key === 'Tab') {
        document.body.classList.add('keyboard-nav');
      }

      // Escape to close modals
      if (e.key === 'Escape') {
        // Close topmost modal
      }

      // Arrow keys for lists
      if (['ArrowUp', 'ArrowDown'].includes(e.key)) {
        // Navigate list items
      }
    };

    const handleMouseDown = () => {
      document.body.classList.remove('keyboard-nav');
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('mousedown', handleMouseDown);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('mousedown', handleMouseDown);
    };
  }, []);
}
```

### ✅ 완료 기준

- [ ] WCAG 2.1 AA 준수
- [ ] 키보드만으로 모든 기능 접근
- [ ] 스크린 리더 테스트
- [ ] Color contrast 검증

### 📝 Commit Message

```
feat(a11y): improve accessibility to WCAG 2.1 AA

- Add comprehensive ARIA labels
- Implement keyboard navigation
- Add high contrast theme option
- Test with screen readers
- Ensure color contrast ratios

Accessibility features:
- Tab navigation
- Focus indicators
- Skip links
- Alt text for images
```

---

## Commit 82: 다국어 지원

### 📋 작업 내용

1. **i18next 설정**
2. **언어 파일**
3. **동적 언어 전환**
4. **RTL 지원**

### 1️⃣ i18n 설정

**파일**: `src/renderer/i18n/index.ts`

```typescript
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import en from './locales/en.json';
import ko from './locales/ko.json';
import ja from './locales/ja.json';
import zh from './locales/zh.json';

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      ko: { translation: ko },
      ja: { translation: ja },
      zh: { translation: zh },
    },
    fallbackLng: 'en',
    interpolation: {
      escapeValue: false,
    },
  });

export default i18n;
```

### 2️⃣ 언어 파일

**파일**: `src/renderer/i18n/locales/en.json`

```json
{
  "common": {
    "save": "Save",
    "cancel": "Cancel",
    "delete": "Delete",
    "edit": "Edit",
    "close": "Close"
  },
  "chat": {
    "newSession": "New Session",
    "sendMessage": "Send Message",
    "typing": "Typing...",
    "placeholder": "Type your message..."
  },
  "settings": {
    "title": "Settings",
    "appearance": "Appearance",
    "language": "Language",
    "theme": "Theme"
  }
}
```

**파일**: `src/renderer/i18n/locales/ko.json`

```json
{
  "common": {
    "save": "저장",
    "cancel": "취소",
    "delete": "삭제",
    "edit": "편집",
    "close": "닫기"
  },
  "chat": {
    "newSession": "새 세션",
    "sendMessage": "메시지 전송",
    "typing": "입력 중...",
    "placeholder": "메시지를 입력하세요..."
  },
  "settings": {
    "title": "설정",
    "appearance": "외관",
    "language": "언어",
    "theme": "테마"
  }
}
```

### 3️⃣ 언어 전환 UI

**파일**: `src/renderer/components/settings/LanguageSelector.tsx`

```typescript
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

const languages = [
  { code: 'en', name: 'English' },
  { code: 'ko', name: '한국어' },
  { code: 'ja', name: '日本語' },
  { code: 'zh', name: '中文' },
];

export function LanguageSelector() {
  const { i18n } = useTranslation();

  const handleChange = (value: string) => {
    i18n.changeLanguage(value);

    // Save to electron-store
    if (window.electronAPI) {
      window.electronAPI.setSetting('language', value);
    }
  };

  return (
    <Select value={i18n.language} onValueChange={handleChange}>
      <SelectTrigger>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {languages.map((lang) => (
          <SelectItem key={lang.code} value={lang.code}>
            {lang.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
```

### ✅ 완료 기준

- [ ] 4개 언어 지원 (en, ko, ja, zh)
- [ ] 동적 언어 전환
- [ ] RTL 레이아웃 (아랍어용)
- [ ] Native menu 다국어

### 📝 Commit Message

```
feat(i18n): add multi-language support

- Integrate i18next and react-i18next
- Add translations for 4 languages (en, ko, ja, zh)
- Implement language selector
- Support RTL layouts
- Translate native menus

Electron-specific:
- Auto-detect system language
- Store language preference
```

---

## Commits 83-84: 버그 수정, 릴리스 준비

### Commit 83: 최종 버그 수정
- 알려진 버그 모두 해결
- Edge case 처리
- 성능 개선
- 메모리 누수 수정

### Commit 84: 릴리스 준비
- 최종 빌드 및 테스트
- Release notes 작성
- 스크린샷 및 데모 영상
- App Store 제출 준비

**Release Checklist**:
```markdown
## Release Checklist v1.0.0

### Pre-release
- [x] All tests passing
- [x] Version bumped
- [x] CHANGELOG updated
- [x] Documentation complete
- [x] Screenshots updated

### Build
- [x] macOS build (Universal)
- [x] Windows build (x64)
- [x] Linux build (AppImage, deb)
- [x] Code signing verified
- [x] Notarization complete

### Testing
- [x] Manual testing all platforms
- [x] Auto-update tested
- [x] Installation tested
- [x] Performance benchmarks

### Release
- [x] Git tag created
- [x] GitHub Release published
- [x] Auto-update enabled
- [x] Announcement ready
```

---

## 🎯 Day 14 완료 체크리스트

### 기능 완성도
- [ ] 애니메이션 완성
- [ ] 온보딩 플로우
- [ ] 접근성 WCAG 2.1 AA
- [ ] 4개 언어 지원
- [ ] 모든 버그 수정
- [ ] 릴리스 준비 완료

### Electron 통합
- [ ] Splash screen
- [ ] System language 감지
- [ ] Touch Bar (macOS)
- [ ] Accessibility API

---

## 📦 Dependencies

```json
{
  "dependencies": {
    "framer-motion": "^10.16.16",
    "react-i18next": "^13.5.0",
    "i18next": "^23.7.11",
    "i18next-browser-languagedetector": "^7.2.0"
  }
}
```

---

## 🎉 Week 2 완료!

**총 커밋**: 42개 (Commits 43-84)
**총 기능**:
- MCP 통합 ✅
- 멀티모달 지원 ✅
- 고급 도구 & 워크플로우 ✅
- 플러그인 시스템 ✅
- 실시간 협업 ✅
- 성능 모니터링 ✅
- UI/UX 폴리싱 ✅

**프로덕션 준비 완료** 🚀

---

**다음**: Week 3에서는 추가 기능 개발 및 안정화를 진행합니다.
