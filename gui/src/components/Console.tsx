import React, { useState, useEffect, useRef } from 'react';
import { Terminal, Send } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useSocket, SocketMessage } from '@/hooks/useSocket';


interface ConsoleEntry {
  id: string;
  type: 'command' | 'output' | 'error';
  content: string;
  timestamp: number;
}

export const Console: React.FC = () => {
  const [command, setCommand] = useState('');
  const [history, setHistory] = useState<ConsoleEntry[]>([
    {
      id: '1',
      type: 'output',
      content: 'NØNOS Zero-Trust Operating System v1.0.0',
      timestamp: Date.now() - 1000
    },
    {
      id: '2',
      type: 'output',
      content: 'Type "help" for available commands',
      timestamp: Date.now()
    }
  ]);
  const [commandHistory, setCommandHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  
  const { sendMessage, lastMessage, connectionStatus } = useSocket('ws://localhost:8081');

  useEffect(() => {
    if (lastMessage && lastMessage.type === 'output') {
      const newEntry: ConsoleEntry = {
        id: Date.now().toString(),
        type: lastMessage.type,
        content: lastMessage.data,
        timestamp: lastMessage.timestamp
      };
      setHistory(prev => [...prev, newEntry]);
    }
  }, [lastMessage]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
  }, [history]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!command.trim()) return;

    // Add command to history
    const commandEntry: ConsoleEntry = {
      id: Date.now().toString(),
      type: 'command',
      content: command,
      timestamp: Date.now()
    };
    
    setHistory(prev => [...prev, commandEntry]);
    setCommandHistory(prev => [...prev, command]);
    
    // Send to backend via WebSocket
    const message: SocketMessage = {
      type: 'command',
      data: command,
      timestamp: Date.now()
    };
    sendMessage(message);
    
    // Handle local commands for demo
    handleLocalCommand(command);
    
    setCommand('');
    setHistoryIndex(-1);
  };

  const handleLocalCommand = (cmd: string) => {
    const lowerCmd = cmd.toLowerCase().trim();
    let response = '';
    
    // Handle module installation commands
    if (lowerCmd.startsWith('nonos_module_')) {
      const moduleMap: { [key: string]: string } = {
        'nonos_module_browser': 'Firefox Browser installed! Check desktop for new icon.',
        'nonos_module_mail': 'Thunderbird Mail installed! Check desktop for new icon.',
        'nonos_module_calc': 'Calculator installed! Check desktop for new icon.',
        'nonos_module_music': 'Music Player installed! Check desktop for new icon.',
        'nonos_module_notes': 'Notes app installed! Check desktop for new icon.',
        'nonos_module_photos': 'Photos app installed! Check desktop for new icon.'
      };
      
      response = moduleMap[lowerCmd] || `Module '${lowerCmd}' not found. Available modules: browser, mail, calc, music, notes, photos`;
      
      // Trigger module installation event
      if (moduleMap[lowerCmd]) {
        const moduleEvent = new CustomEvent('moduleInstalled', { 
          detail: { command: lowerCmd } 
        });
        window.dispatchEvent(moduleEvent);
      }
    } else {
      switch (lowerCmd) {
        case 'help':
          response = `Available commands:
help          - Show this help message
status        - Show system status
modules       - List active modules
vault         - Access encrypted vault
logs          - View system logs
network       - Show network status
clear         - Clear console
whoami        - Show current user context
Module Installation:
nonos_module_browser  - Install Firefox browser
nonos_module_mail     - Install Thunderbird mail
nonos_module_calc     - Install calculator
nonos_module_music    - Install music player
nonos_module_notes    - Install notes app
nonos_module_photos   - Install photos app`;
          break;
        case 'status':
          response = `System Status: OPERATIONAL
Encryption: AES-256-GCM
Zero-Trust Mode: ENABLED
Relay Network: CONNECTED
Active Modules: 3`;
          break;
        case 'modules':
          response = `Active Modules:
└── core.nonosmod (v1.0.0) - System Core
└── crypto.nonosmod (v1.2.1) - Cryptographic Functions  
└── network.nonosmod (v1.1.0) - Network Management`;
          break;
        case 'vault':
          response = 'Vault access requires authentication. Use biometric or hardware key.';
          break;
        case 'logs':
          response = `Recent System Logs:
[INFO] System initialized successfully
[DEBUG] Network relay established
[INFO] Vault locked and secured`;
          break;
        case 'network':
          response = `Network Status:
Relay: CONNECTED (2 peers)
Latency: 15ms
Encryption: Active
Traffic: 1.2KB/s`;
          break;
        case 'clear':
          setHistory([]);
          return;
        case 'whoami':
          response = 'User: anonymous@nønos-terminal';
          break;
        default:
          response = `Command not found: ${cmd}. Type "help" for available commands.`;
      }
    }
    
    setTimeout(() => {
      const responseEntry: ConsoleEntry = {
        id: (Date.now() + 1).toString(),
        type: 'output',
        content: response,
        timestamp: Date.now()
      };
      setHistory(prev => [...prev, responseEntry]);
    }, 100);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (historyIndex < commandHistory.length - 1) {
        const newIndex = historyIndex + 1;
        setHistoryIndex(newIndex);
        setCommand(commandHistory[commandHistory.length - 1 - newIndex]);
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (historyIndex > 0) {
        const newIndex = historyIndex - 1;
        setHistoryIndex(newIndex);
        setCommand(commandHistory[commandHistory.length - 1 - newIndex]);
      } else if (historyIndex === 0) {
        setHistoryIndex(-1);
        setCommand('');
      }
    }
  };

  const getStatusColor = () => {
    switch (connectionStatus) {
      case 'connected': return 'text-success';
      case 'connecting': return 'text-warning';
      case 'error': return 'text-destructive';
      default: return 'text-muted-foreground';
    }
  };

  return (
    <div className="h-full flex flex-col bg-terminal-bg border border-terminal-border terminal-glow rounded-lg">
      <div className="flex items-center justify-between p-3 border-b border-terminal-border">
        <div className="flex items-center gap-2">
          <Terminal className="w-4 h-4 text-primary" />
          <span className="text-sm font-semibold">NØNOS Terminal</span>
        </div>
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${getStatusColor()}`}>
            <div className={`w-full h-full rounded-full animate-glow-pulse`} />
          </div>
          <span className={`text-xs ${getStatusColor()}`}>
            {connectionStatus.toUpperCase()}
          </span>
        </div>
      </div>
      
      <div 
        ref={scrollRef}
        className="flex-1 p-4 overflow-y-auto font-mono text-sm space-y-1"
      >
        {history.map((entry) => (
          <div key={entry.id} className="animate-fade-in">
            {entry.type === 'command' ? (
              <div className="flex items-start gap-2">
                <span className="text-terminal-prompt shrink-0">nønos@localhost:~$</span>
                <span className="text-primary">{entry.content}</span>
              </div>
            ) : (
              <div className={`pl-4 ${entry.type === 'error' ? 'text-destructive' : 'text-foreground'}`}>
                {entry.content.split('\n').map((line, i) => (
                  <div key={i}>{line}</div>
                ))}
              </div>
            )}
          </div>
        ))}
        <div className="flex items-center gap-2">
          <span className="text-terminal-prompt">nønos@localhost:~$</span>
          <div className="w-2 h-4 bg-primary animate-terminal-blink" />
        </div>
      </div>
      
      <form onSubmit={handleSubmit} className="p-3 border-t border-terminal-border">
        <div className="flex gap-2">
          <div className="flex items-center gap-2 flex-1">
            <span className="text-terminal-prompt text-sm shrink-0">nønos@localhost:~$</span>
            <Input
              ref={inputRef}
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Enter command..."
              className="bg-transparent border-none focus:ring-0 font-mono text-sm p-0 h-auto"
              autoFocus
            />
          </div>
          <Button 
            type="submit" 
            size="sm" 
            variant="outline"
            className="shrink-0"
          >
            <Send className="w-3 h-3" />
          </Button>
        </div>
      </form>
    </div>
  );
};