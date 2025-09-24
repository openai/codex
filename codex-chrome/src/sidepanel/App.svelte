<script lang="ts">
  import { onMount } from 'svelte';
  import { MessageRouter, MessageType } from '../core/MessageRouter';
  import type { Event } from '../protocol/types';
  import type { EventMsg } from '../protocol/events';

  let router: MessageRouter;
  let messages: Array<{ type: 'user' | 'agent'; content: string; timestamp: number }> = [];
  let inputText = '';
  let isConnected = false;
  let isProcessing = false;

  onMount(() => {
    // Initialize router
    router = new MessageRouter('sidepanel');

    // Setup event handlers
    router.on(MessageType.EVENT, (message) => {
      const event = message.payload as Event;
      handleEvent(event);
    });

    router.on(MessageType.STATE_UPDATE, (message) => {
      console.log('State update:', message.payload);
    });

    // Check connection
    checkConnection();

    // Periodic connection check
    const interval = setInterval(checkConnection, 5000);

    return () => {
      clearInterval(interval);
      router?.cleanup();
    };
  });

  async function checkConnection() {
    try {
      const response = await router?.send(MessageType.PING);
      isConnected = response?.type === MessageType.PONG;
    } catch {
      isConnected = false;
    }
  }

  function handleEvent(event: Event) {
    const msg = event.msg;

    switch (msg.type) {
      case 'AgentMessage':
        if ('data' in msg && msg.data && 'message' in msg.data) {
          messages = [...messages, {
            type: 'agent',
            content: msg.data.message,
            timestamp: Date.now(),
          }];
        }
        break;

      case 'TaskStarted':
        isProcessing = true;
        break;

      case 'TaskComplete':
        isProcessing = false;
        break;

      case 'Error':
        if ('data' in msg && msg.data && 'message' in msg.data) {
          messages = [...messages, {
            type: 'agent',
            content: `Error: ${msg.data.message}`,
            timestamp: Date.now(),
          }];
        }
        isProcessing = false;
        break;
    }
  }

  async function sendMessage() {
    if (!inputText.trim() || !isConnected) return;

    const text = inputText.trim();
    inputText = '';

    // Add user message
    messages = [...messages, {
      type: 'user',
      content: text,
      timestamp: Date.now(),
    }];

    // Send to agent
    try {
      await router.sendSubmission({
        id: `user_${Date.now()}`,
        op: {
          type: 'UserInput',
          items: [{ type: 'text', text }],
        },
      });
    } catch (error) {
      console.error('Failed to send message:', error);
      messages = [...messages, {
        type: 'agent',
        content: 'Failed to send message. Please try again.',
        timestamp: Date.now(),
      }];
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
    });
  }
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <div class="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
    <div class="flex items-center space-x-2">
      <h1 class="text-lg font-semibold">Codex Assistant</h1>
      {#if isProcessing}
        <div class="w-2 h-2 bg-blue-500 rounded-full animate-pulse"></div>
      {/if}
    </div>
    <div class="flex items-center space-x-2">
      <div class="w-2 h-2 rounded-full"
           class:bg-green-500={isConnected}
           class:bg-red-500={!isConnected}
           title={isConnected ? 'Connected' : 'Disconnected'}>
      </div>
    </div>
  </div>

  <!-- Messages -->
  <div class="flex-1 overflow-y-auto p-4 space-y-3">
    {#if messages.length === 0}
      <div class="text-center text-gray-500 dark:text-gray-400 mt-8">
        <p class="text-lg mb-2">Welcome to Codex</p>
        <p class="text-sm">Start a conversation or select text on the page</p>
      </div>
    {/if}

    {#each messages as message}
      <div class="flex {message.type === 'user' ? 'justify-end' : 'justify-start'}">
        <div class="max-w-[80%] rounded-lg px-4 py-2 {message.type === 'user'
             ? 'bg-blue-500 text-white'
             : 'bg-gray-100 dark:bg-gray-800'}">
          <div class="text-sm whitespace-pre-wrap">{message.content}</div>
          <div class="text-xs mt-1 opacity-70">
            {formatTime(message.timestamp)}
          </div>
        </div>
      </div>
    {/each}
  </div>

  <!-- Input -->
  <div class="border-t border-gray-200 dark:border-gray-700 p-4">
    <div class="flex space-x-2">
      <textarea
        bind:value={inputText}
        on:keydown={handleKeydown}
        placeholder="Type a message..."
        disabled={!isConnected}
        class="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600
               rounded-lg resize-none focus:outline-none focus:ring-2
               focus:ring-blue-500 dark:bg-gray-800"
        rows="2" />
      <button
        on:click={sendMessage}
        disabled={!isConnected || !inputText.trim()}
        class="px-4 py-2 bg-blue-500 text-white rounded-lg
               hover:bg-blue-600 disabled:opacity-50
               disabled:cursor-not-allowed transition-colors">
        Send
      </button>
    </div>
  </div>
</div>

<style>
  /* Component-specific styles */
  textarea {
    font-family: inherit;
  }

  .animate-pulse {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: .5;
    }
  }
</style>