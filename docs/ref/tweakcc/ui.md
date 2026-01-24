# tweakcc UI Components

> React/Ink Terminal UI Documentation

## Overview

tweakcc uses React with the Ink framework to render a terminal-based user interface. Ink provides React components that render to the terminal instead of the DOM.

## Technology Stack

| Component | Library | Version |
|-----------|---------|---------|
| UI Logic | React | 19.1.1 |
| Terminal Renderer | Ink | 6.1.0 |
| Terminal Colors | Chalk | 5.5.0 |
| Terminal Links | ink-link | 4.1.0 |
| Terminal Images | ink-image | 2.0.0 |

## Component Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         App.tsx                                  │
│                    (Root Component)                              │
│              ┌──────────────────────────┐                       │
│              │    SettingsContext       │                       │
│              │    (Global State)        │                       │
│              └──────────────────────────┘                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐                │
│  │  Header    │  │ Piebald    │  │Notification│                │
│  │            │  │Announcement│  │            │                │
│  └────────────┘  └────────────┘  └────────────┘                │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                      MainView                            │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │              SelectInput (Menu)                  │    │   │
│  │  │  > *Apply customizations                        │    │   │
│  │  │    Themes                                        │    │   │
│  │  │    Thinking verbs                                │    │   │
│  │  │    ...                                           │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   View Components                        │   │
│  │  ThemesView | ThinkingVerbsView | ToolsetsView | etc.   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### `App.tsx` - Root Application

**Location:** `src/ui/App.tsx`

**Purpose:** Root component that manages global state and view routing.

**Props:**
```typescript
interface AppProps {
  startupCheckInfo: StartupCheckInfo
  configMigrated: boolean
}
```

**State:**
```typescript
const [config, setConfig] = useState<TweakccConfig>()
const [showPiebaldAnnouncement, setShowPiebaldAnnouncement] = useState(true)
const [currentView, setCurrentView] = useState<MainMenuItem | null>(null)
const [notification, setNotification] = useState<Notification | null>(null)
```

**Context Provider:**
```typescript
const SettingsContext = createContext<{
  settings: Settings
  updateSettings: (updates: Partial<Settings>) => void
}>()
```

**Keyboard Shortcuts:**

| Key | Action |
|-----|--------|
| Ctrl+C | Exit application |
| Q | Exit (in main menu) |
| Escape | Exit / Go back |
| H | Hide announcement |

**Lifecycle:**
1. Load config on mount
2. Show startup warnings if needed
3. Render main menu
4. Route to selected view
5. Persist changes on update

---

### `MainView.tsx` - Main Menu

**Location:** `src/ui/components/MainView.tsx`

**Purpose:** Display main menu with all available options.

**Menu Items:**
```typescript
enum MainMenuItem {
  APPLY_CHANGES = '*Apply customizations',
  THEMES = 'Themes',
  THINKING_VERBS = 'Thinking verbs',
  THINKING_STYLE = 'Thinking style',
  USER_MESSAGE_DISPLAY = 'User message display',
  MISC = 'Misc',
  TOOLSETS = 'Toolsets',
  VIEW_SYSTEM_PROMPTS = 'View system prompts',
  RESTORE_ORIGINAL = 'Restore original Claude Code',
  OPEN_CONFIG = 'Open config.json',
  OPEN_CLI = "Open Claude Code's cli.js",
  EXIT = 'Exit'
}
```

**Features:**
- Shows asterisk (*) on Apply when changes pending
- Displays warnings for version changes
- Shows config migration notice
- Highlights current selection

---

### `Header.tsx` - Section Headers

**Location:** `src/ui/components/Header.tsx`

**Purpose:** Consistent section header styling.

**Props:**
```typescript
interface HeaderProps {
  title: string
  subtitle?: string
}
```

**Usage:**
```tsx
<Header
  title="Themes"
  subtitle="Create and edit custom themes"
/>
```

---

### `SelectInput.tsx` - Menu Selector

**Location:** `src/ui/components/SelectInput.tsx`

**Purpose:** Reusable selection component with keyboard navigation.

**Props:**
```typescript
interface SelectInputProps<T> {
  items: SelectItem<T>[]
  onSelect: (item: T) => void
  initialIndex?: number
  highlightColor?: string
  showIndicator?: boolean
}

interface SelectItem<T> {
  label: string
  value: T
  disabled?: boolean
}
```

**Keyboard Navigation:**
- Arrow Up/Down: Navigate items
- Enter: Select item
- j/k: Vim-style navigation

---

## Theme Components

### `ThemesView.tsx` - Theme Manager

**Location:** `src/ui/components/ThemesView.tsx`

**Purpose:** List, create, edit, and delete themes.

**Features:**
- List all themes (built-in + custom)
- Create new theme (copy from existing)
- Edit theme colors
- Delete custom themes
- Select active theme

---

### `ThemeEditView.tsx` - Theme Editor

**Location:** `src/ui/components/ThemeEditView.tsx`

**Purpose:** Edit individual theme colors.

**Features:**
- List all 62+ color properties
- Edit each color value
- Preview color in terminal
- Validate color format

---

### `ColorPicker.tsx` - Color Input

**Location:** `src/ui/components/ColorPicker.tsx`

**Purpose:** Input and validate colors.

**Supported Formats:**
- RGB: `rgb(255, 100, 50)`
- Hex: `#ff6432`
- HSL: `hsl(20, 100%, 60%)`
- ANSI: `red`, `blue`, etc.

**Features:**
- Real-time validation
- Color preview
- Format conversion

---

### `ThemePreview.tsx` - Color Preview

**Location:** `src/ui/components/ThemePreview.tsx`

**Purpose:** Display color swatches for theme preview.

---

### `ColoredColorName.tsx` - Color Display

**Location:** `src/ui/components/ColoredColorName.tsx`

**Purpose:** Display color name with the color applied.

```tsx
<ColoredColorName color="rgb(255, 100, 50)" name="error" />
// Renders "error" in rgb(255, 100, 50) color
```

---

## Thinking Components

### `ThinkingVerbsView.tsx` - Verb Editor

**Location:** `src/ui/components/ThinkingVerbsView.tsx`

**Purpose:** Edit the list of thinking action verbs.

**Features:**
- View current verbs
- Add new verbs
- Remove verbs
- Edit format string
- Reset to defaults

---

### `ThinkingStyleView.tsx` - Style Editor

**Location:** `src/ui/components/ThinkingStyleView.tsx`

**Purpose:** Configure spinner animation.

**Options:**
- Animation phases (characters)
- Update interval (speed)
- Reverse mirror toggle

---

## Display Components

### `UserMessageDisplayView.tsx` - Message Styling

**Location:** `src/ui/components/UserMessageDisplayView.tsx`

**Purpose:** Configure user message appearance.

**Options:**
- Format string
- Text styling (bold, italic, etc.)
- Foreground/background colors
- Border style and color
- Padding

---

## Tool Components

### `ToolsetsView.tsx` - Toolset Manager

**Location:** `src/ui/components/ToolsetsView.tsx`

**Purpose:** Manage tool restriction groups.

**Features:**
- List toolsets
- Create new toolset
- Edit toolset
- Delete toolset
- Select active toolset

---

### `ToolsetEditView.tsx` - Toolset Editor

**Location:** `src/ui/components/ToolsetEditView.tsx`

**Purpose:** Edit individual toolset.

**Features:**
- Set toolset name
- Select allowed tools
- Toggle "all tools" option

---

## Misc Components

### `MiscView.tsx` - Misc Settings

**Location:** `src/ui/components/MiscView.tsx`

**Purpose:** Configure miscellaneous options.

**Options:**
- Show tweakcc version
- Show patches applied
- Expand thinking blocks
- Enable conversation title
- Hide startup banner
- Hide Ctrl+G hint
- Hide Clawd logo
- Increase file read limit

---

### `InstallationPicker.tsx` - Installation Selector

**Location:** `src/ui/components/InstallationPicker.tsx`

**Purpose:** Select from multiple Claude Code installations.

**Features:**
- List all detected installations
- Show version and type
- Show path
- Highlight recommended (latest)

---

### `ChangeNameView.tsx` - Name Editor

**Location:** `src/ui/components/ChangeNameView.tsx`

**Purpose:** Generic name editing component.

**Usage:**
- Rename themes
- Rename toolsets

---

### `PiebaldAnnouncement.tsx` - Welcome Banner

**Location:** `src/ui/components/PiebaldAnnouncement.tsx`

**Purpose:** Show welcome message and announcements.

**Features:**
- Dismissible with 'H' key
- Shows tweakcc version
- Links to documentation

---

## Custom Hooks

### `useNonInitialEffect.ts`

**Location:** `src/ui/hooks/useNonInitialEffect.ts`

**Purpose:** useEffect that skips the initial render.

```typescript
function useNonInitialEffect(
  effect: EffectCallback,
  deps: DependencyList
): void {
  const initialRender = useRef(true);

  useEffect(() => {
    if (initialRender.current) {
      initialRender.current = false;
      return;
    }
    return effect();
  }, deps);
}
```

**Usage:**
```tsx
// Don't run on initial render
useNonInitialEffect(() => {
  saveSettings(settings);
}, [settings]);
```

---

## Context API

### SettingsContext

**Purpose:** Share settings and update function across components.

**Definition:**
```typescript
interface SettingsContextValue {
  settings: Settings
  updateSettings: (updates: Partial<Settings>) => void
}

const SettingsContext = createContext<SettingsContextValue | null>(null)
```

**Provider:**
```tsx
<SettingsContext.Provider value={{ settings, updateSettings }}>
  {children}
</SettingsContext.Provider>
```

**Consumer:**
```tsx
function MyComponent() {
  const { settings, updateSettings } = useContext(SettingsContext);

  const handleChange = () => {
    updateSettings({ themeId: 'dark' });
  };

  return <Text>{settings.themeId}</Text>;
}
```

---

## Ink Primitives

### Common Components

| Component | Purpose | Example |
|-----------|---------|---------|
| `<Box>` | Flexbox container | `<Box flexDirection="column">` |
| `<Text>` | Text display | `<Text bold color="green">` |
| `<Spacer>` | Flexible space | `<Spacer />` |
| `<Newline>` | Line break | `<Newline />` |

### Text Styling

```tsx
<Text bold>Bold text</Text>
<Text italic>Italic text</Text>
<Text underline>Underlined text</Text>
<Text strikethrough>Strikethrough text</Text>
<Text inverse>Inverse colors</Text>
<Text color="green">Green text</Text>
<Text backgroundColor="blue">Blue background</Text>
<Text dimColor>Dimmed text</Text>
```

### Box Styling

```tsx
<Box
  flexDirection="column"
  padding={1}
  borderStyle="round"
  borderColor="cyan"
>
  {children}
</Box>
```

### Input Handling

```tsx
import { useInput } from 'ink';

function MyComponent() {
  useInput((input, key) => {
    if (key.escape) {
      handleEscape();
    }
    if (input === 'q') {
      handleQuit();
    }
    if (key.return) {
      handleEnter();
    }
  });

  return <Text>Press Q to quit</Text>;
}
```

---

## Keyboard Handling

### Global Shortcuts

Handled in `App.tsx`:

```tsx
useInput((input, key) => {
  // Exit on Ctrl+C
  if (key.ctrl && input === 'c') {
    process.exit(0);
  }

  // Quit on 'q' or Escape in main menu
  if (!currentView && (input === 'q' || key.escape)) {
    process.exit(0);
  }

  // Hide announcement on 'h'
  if (input === 'h' && showPiebaldAnnouncement) {
    setShowPiebaldAnnouncement(false);
  }
});
```

### Navigation Shortcuts

| Key | Action |
|-----|--------|
| Arrow Up | Previous item |
| Arrow Down | Next item |
| Enter | Select item |
| Escape | Go back |
| k | Previous item (vim) |
| j | Next item (vim) |

---

## Rendering Flow

```
User Action
    │
    ▼
useInput() hook
    │
    ▼
State Update (useState)
    │
    ▼
React Re-render
    │
    ▼
Ink Reconciler
    │
    ▼
Terminal Output
```

---

## Best Practices

### State Management

1. Use SettingsContext for shared state
2. Local state for component-specific UI
3. Persist to config on meaningful changes

### Performance

1. Minimize re-renders with useCallback/useMemo
2. Avoid complex calculations in render
3. Use useNonInitialEffect for side effects

### Accessibility

1. Provide keyboard navigation
2. Use consistent color schemes
3. Support different terminal sizes

### Error Handling

1. Wrap components in error boundaries
2. Display user-friendly error messages
3. Provide recovery options
