import React from 'react';
import { render } from 'ink-testing-library';
import { describe, it, expect } from 'vitest';
import TemplatesList from './list';

describe('TemplatesList Pagination CLI', () => {
  it('renders first page and navigates to next', async () => {
    const { lastFrame, stdin } = render(<TemplatesList />);
    await new Promise(r => setTimeout(r, 150));
    expect(lastFrame()).toContain('Template #1');
    expect(lastFrame()).toContain('Page 1');
    stdin.write('n');
    await new Promise(r => setTimeout(r, 150));
    expect(lastFrame()).toContain('Template #11');
    expect(lastFrame()).toContain('Page 2');
  });

  it('navigates to last page and back to previous', async () => {
    const { lastFrame, stdin } = render(<TemplatesList />);
    await new Promise(r => setTimeout(r, 150));
    // Go to last page
    for (let i = 0; i < 3; i++) {
      stdin.write('n');
      await new Promise(r => setTimeout(r, 150));
    }
    expect(lastFrame()).toContain('Template #31');
    expect(lastFrame()).toContain('Page 4');
    stdin.write('p');
    await new Promise(r => setTimeout(r, 150));
    expect(lastFrame()).toContain('Template #21');
    expect(lastFrame()).toContain('Page 3');
  });

  it('shows correct navigation hints at start and end', async () => {
    const { lastFrame, stdin } = render(<TemplatesList />);
    await new Promise(r => setTimeout(r, 150));
    expect(lastFrame()).toContain('[n] Next');
    expect(lastFrame()).not.toContain('[p] Prev');
    // Go to last page
    for (let i = 0; i < 3; i++) {
      stdin.write('n');
      await new Promise(r => setTimeout(r, 150));
    }
    expect(lastFrame()).not.toContain('[n] Next');
    expect(lastFrame()).toContain('[p] Prev');
  });
});
