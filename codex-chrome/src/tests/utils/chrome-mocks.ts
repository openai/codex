/**
 * Chrome API mocks for testing
 * Provides mock implementations of Chrome extension APIs
 */

export const chromeMock = {
  runtime: {
    sendMessage: vi.fn().mockResolvedValue(undefined),
    onMessage: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
    onInstalled: {
      addListener: vi.fn(),
    },
    getURL: vi.fn((path: string) => `chrome-extension://mock-id/${path}`),
    id: 'mock-extension-id',
  },

  tabs: {
    create: vi.fn().mockResolvedValue({ id: 1, url: 'https://example.com' }),
    remove: vi.fn().mockResolvedValue(undefined),
    update: vi.fn().mockResolvedValue({ id: 1 }),
    query: vi.fn().mockResolvedValue([]),
    get: vi.fn().mockResolvedValue({ id: 1, url: 'https://example.com' }),
    getCurrent: vi.fn().mockResolvedValue({ id: 1, url: 'https://example.com' }),
    captureVisibleTab: vi.fn().mockResolvedValue('data:image/png;base64,mock'),
    onUpdated: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
  },

  storage: {
    local: {
      get: vi.fn().mockResolvedValue({}),
      set: vi.fn().mockResolvedValue(undefined),
      remove: vi.fn().mockResolvedValue(undefined),
      clear: vi.fn().mockResolvedValue(undefined),
    },
    session: {
      get: vi.fn().mockResolvedValue({}),
      set: vi.fn().mockResolvedValue(undefined),
      remove: vi.fn().mockResolvedValue(undefined),
      clear: vi.fn().mockResolvedValue(undefined),
    },
    sync: {
      get: vi.fn().mockResolvedValue({}),
      set: vi.fn().mockResolvedValue(undefined),
      remove: vi.fn().mockResolvedValue(undefined),
      clear: vi.fn().mockResolvedValue(undefined),
    },
  },

  scripting: {
    executeScript: vi.fn().mockResolvedValue([{ result: 'mock-result' }]),
    insertCSS: vi.fn().mockResolvedValue(undefined),
    removeCSS: vi.fn().mockResolvedValue(undefined),
  },

  action: {
    setTitle: vi.fn().mockResolvedValue(undefined),
    setIcon: vi.fn().mockResolvedValue(undefined),
    setBadgeText: vi.fn().mockResolvedValue(undefined),
    setBadgeBackgroundColor: vi.fn().mockResolvedValue(undefined),
  },

  webNavigation: {
    onCompleted: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
    onBeforeNavigate: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
  },
};

// Helper to install chrome mock globally
export function installChromeMock() {
  (global as any).chrome = chromeMock;
}

// Helper to reset all mocks
export function resetChromeMocks() {
  Object.values(chromeMock).forEach(api => {
    if (typeof api === 'object') {
      Object.values(api).forEach(method => {
        if (typeof method === 'function' && 'mockClear' in method) {
          (method as any).mockClear();
        } else if (typeof method === 'object' && method !== null) {
          Object.values(method).forEach(subMethod => {
            if (typeof subMethod === 'function' && 'mockClear' in subMethod) {
              (subMethod as any).mockClear();
            }
          });
        }
      });
    }
  });
}

// Message passing mock utilities
export class MessageChannelMock {
  private listeners: Array<(message: any, sender: any, sendResponse: any) => void> = [];

  addListener(callback: (message: any, sender: any, sendResponse: any) => void) {
    this.listeners.push(callback);
  }

  removeListener(callback: (message: any, sender: any, sendResponse: any) => void) {
    const index = this.listeners.indexOf(callback);
    if (index !== -1) {
      this.listeners.splice(index, 1);
    }
  }

  async sendMessage(message: any, sender = { id: 'test-sender' }) {
    const responses = await Promise.all(
      this.listeners.map(listener =>
        new Promise(resolve => {
          const sendResponse = (response: any) => resolve(response);
          const result = listener(message, sender, sendResponse);
          if (result !== true) {
            resolve(undefined);
          }
        })
      )
    );
    return responses.find(r => r !== undefined);
  }
}