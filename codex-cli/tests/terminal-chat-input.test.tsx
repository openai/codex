import React from 'react';
import { render, waitFor } from 'ink-testing-library';
import TerminalChatInput from '../src/components/chat/terminal-chat-input';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock useStdin to simulate terminal input
vi.mock('ink', async () => {
  const original = await vi.importActual('ink');
  return {
    ...original,
    useStdin: () => ({
      stdin: {
        on: vi.fn(),
        off: vi.fn(),
        isTTY: true,
        setRawMode: vi.fn(),
      },
      setRawMode: vi.fn(),
    }),
  };
});

// Mock process.stdout
const originalStdout = process.stdout;
const mockStdout = {
  ...originalStdout,
  write: vi.fn(),
  isTTY: true,
};

describe('TerminalChatInput', () => {
  beforeEach(() => {
    // Replace stdout with mock
    process.stdout = mockStdout as any;
  });

  afterEach(() => {
    // Restore stdout
    process.stdout = originalStdout;
    vi.clearAllMocks();
  });

  it('shows the input box properly in terminal', async () => {
    // Create minimal props for TerminalChatInput
    const props = {
      isNew: true,
      loading: false,
      submitInput: vi.fn(),
      confirmationPrompt: null,
      submitConfirmation: vi.fn(),
      setLastResponseId: vi.fn(),
      setItems: vi.fn(),
      contextLeftPercent: 100,
      openOverlay: vi.fn(),
      openModelOverlay: vi.fn(),
      openApprovalOverlay: vi.fn(),
      openHelpOverlay: vi.fn(),
      openDiffOverlay: vi.fn(),
      onCompact: vi.fn(),
      interruptAgent: vi.fn(),
      active: true,
      thinkingSeconds: 0,
    };

    // Render the component
    const { lastFrame } = render(<TerminalChatInput {...props} />);

    // Wait for render to complete
    await waitFor(() => {
      // Check that the component renders and contains message box elements
      expect(lastFrame()).toBeTruthy();
      expect(lastFrame()).toContain('│'); // Border characters should be visible
      
      // On Darwin/macOS, verify cursor visibility is enabled
      if (process.platform === 'darwin') {
        expect(mockStdout.write).toHaveBeenCalledWith('\x1b[?25h');
      }
    });
  });

  it('handles input correctly on Darwin platforms', async () => {
    // Mock platform as 'darwin' to simulate macOS
    const originalPlatform = process.platform;
    Object.defineProperty(process, 'platform', {
      value: 'darwin',
      configurable: true
    });

    const props = {
      isNew: true,
      loading: false,
      submitInput: vi.fn(),
      confirmationPrompt: null,
      submitConfirmation: vi.fn(),
      setLastResponseId: vi.fn(),
      setItems: vi.fn(),
      contextLeftPercent: 100,
      openOverlay: vi.fn(),
      openModelOverlay: vi.fn(),
      openApprovalOverlay: vi.fn(),
      openHelpOverlay: vi.fn(),
      openDiffOverlay: vi.fn(),
      onCompact: vi.fn(),
      interruptAgent: vi.fn(),
      active: true,
      thinkingSeconds: 0,
    };

    // Render the component
    const component = render(<TerminalChatInput {...props} />);

    // Test input box visibility
    expect(component.lastFrame()).toBeTruthy();
    expect(component.lastFrame()).toContain('│'); // Border should be visible

    // Restore original platform
    Object.defineProperty(process, 'platform', {
      value: originalPlatform,
      configurable: true
    });
  });
}); 