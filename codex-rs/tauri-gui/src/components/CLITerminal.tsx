// CLITerminal.tsx - 組み込みターミナルエミュレータ
// CLI自動補完、実行履歴、3D可視化連携

import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../styles/CLITerminal.css';

export interface CLITerminalProps {
  onCommandExecute?: (cmd: string, result: string) => void;
}

interface HistoryEntry {
  command: string;
  output: string;
  timestamp: string;
}

export default function CLITerminal({ onCommandExecute }: CLITerminalProps) {
  const [input, setInput] = useState<string>('');
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number>(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const terminalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
    }
  }, [history]);

  const executeCommand = async () => {
    if (!input.trim()) return;

    const cmd = input.trim();
    setInput('');
    setHistoryIndex(-1);

    try {
      const result = await invoke<string>('execute_cli_command', {
        cmd: cmd.split(' ')[0],
        args: cmd.split(' ').slice(1),
      });

      const entry: HistoryEntry = {
        command: cmd,
        output: result,
        timestamp: new Date().toISOString(),
      };

      setHistory((prev) => [...prev, entry]);

      if (onCommandExecute) {
        onCommandExecute(cmd, result);
      }
    } catch (error) {
      const entry: HistoryEntry = {
        command: cmd,
        output: `Error: ${error}`,
        timestamp: new Date().toISOString(),
      };
      setHistory((prev) => [...prev, entry]);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      executeCommand();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      // TODO: 履歴ナビゲーション実装
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      // TODO: 履歴ナビゲーション実装
    }
  };

  return (
    <div className="cli-terminal">
      <div className="terminal-header">
        <span className="terminal-title">Codex CLI</span>
        <button className="btn-clear" onClick={() => setHistory([])}>Clear</button>
      </div>

      <div className="terminal-output" ref={terminalRef}>
        {history.map((entry, index) => (
          <div key={index} className="terminal-entry">
            <div className="terminal-command">
              <span className="prompt">$</span>
              <span className="command-text">{entry.command}</span>
            </div>
            <div className="terminal-result">{entry.output}</div>
          </div>
        ))}
      </div>

      <div className="terminal-input">
        <span className="prompt">$</span>
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="codex [command] [args]"
          autoFocus
        />
      </div>
    </div>
  );
}

























