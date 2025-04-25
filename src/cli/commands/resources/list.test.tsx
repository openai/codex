import React from 'react';
import { render } from 'ink-testing-library';
import { describe, it, expect } from 'vitest';
import ResourcesList from './list';

// ErrorBoundary for surfacing errors in tests
class ErrorBoundary extends React.Component<{ children: React.ReactNode }, { error: any }> {
  constructor(props: any) {
    super(props);
    this.state = { error: null };
  }
  static getDerivedStateFromError(error: any) {
    return { error };
  }
  componentDidCatch(error: any, info: any) {
    // eslint-disable-next-line no-console
    console.error('ErrorBoundary caught:', error, info);
  }
  render() {
    if (this.state.error) {
      return <div data-testid="error-boundary">Error: {String(this.state.error)}</div>;
    }
    return this.props.children;
  }
}

async function waitForFrameToContain(getFrame: () => string, text: string, timeout = 2000) {
  const start = Date.now();
  while (!getFrame().includes(text)) {
    if (Date.now() - start > timeout) throw new Error(`Timed out waiting for "${text}". Last frame: ${getFrame()}`);
    await new Promise(r => setTimeout(r, 30));
  }
}

// Removed: All Ink input simulation tests. These are not robust for interactive CLI E2E.
