import React, { useState, useEffect, useRef } from 'react';
import { FileText, Trash2, Search, Filter, AlertCircle, Info, AlertTriangle, CheckCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface LogEntry {
  id: string;
  timestamp: number;
  level: 'info' | 'warning' | 'error' | 'debug';
  module: string;
  message: string;
  details?: string;
}

export const SystemLogs: React.FC = () => {
  const [logs, setLogs] = useState<LogEntry[]>([
    {
      id: '1',
      timestamp: Date.now() - 1000,
      level: 'info',
      module: 'core',
      message: 'System initialization completed successfully',
      details: 'All core modules loaded and operational'
    },
    {
      id: '2',
      timestamp: Date.now() - 5000,
      level: 'info',
      module: 'network',
      message: 'Relay connection established',
      details: 'Connected to relay-001.nønos (latency: 15ms)'
    },
    {
      id: '3',
      timestamp: Date.now() - 12000,
      level: 'warning',
      module: 'crypto',
      message: 'Key rotation scheduled',
      details: 'Master key rotation will occur in 24 hours'
    },
    {
      id: '4',
      timestamp: Date.now() - 18000,
      level: 'debug',
      module: 'vault',
      message: 'Vault access attempt',
      details: 'Authentication successful for user: anonymous'
    },
    {
      id: '5',
      timestamp: Date.now() - 25000,
      level: 'error',
      module: 'network',
      message: 'Peer connection timeout',
      details: 'Failed to connect to relay-007.nønos after 3 attempts'
    },
    {
      id: '6',
      timestamp: Date.now() - 35000,
      level: 'info',
      module: 'terminal',
      message: 'Terminal session started',
      details: 'User shell initialized with PID 1337'
    }
  ]);

  const [filteredLogs, setFilteredLogs] = useState<LogEntry[]>(logs);
  const [searchTerm, setSearchTerm] = useState('');
  const [levelFilter, setLevelFilter] = useState<string>('all');
  const [moduleFilter, setModuleFilter] = useState<string>('all');
  const [autoScroll, setAutoScroll] = useState(true);
  
  const scrollRef = useRef<HTMLDivElement>(null);

  // Simulate new log entries
  useEffect(() => {
    const interval = setInterval(() => {
      const newLog: LogEntry = {
        id: Date.now().toString(),
        timestamp: Date.now(),
        level: Math.random() > 0.8 ? 'warning' : Math.random() > 0.9 ? 'error' : 'info',
        module: ['core', 'network', 'crypto', 'vault', 'terminal'][Math.floor(Math.random() * 5)],
        message: [
          'Heartbeat received from relay network',
          'Memory usage within normal parameters',
          'Periodic security scan completed',
          'Cache optimization routine executed',
          'Network latency measured and logged'
        ][Math.floor(Math.random() * 5)]
      };
      
      setLogs(prev => [newLog, ...prev].slice(0, 100)); // Keep last 100 logs
    }, 8000);

    return () => clearInterval(interval);
  }, []);

  // Filter logs based on search and filters
  useEffect(() => {
    let filtered = logs;

    if (searchTerm) {
      filtered = filtered.filter(log => 
        log.message.toLowerCase().includes(searchTerm.toLowerCase()) ||
        log.module.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (log.details && log.details.toLowerCase().includes(searchTerm.toLowerCase()))
      );
    }

    if (levelFilter !== 'all') {
      filtered = filtered.filter(log => log.level === levelFilter);
    }

    if (moduleFilter !== 'all') {
      filtered = filtered.filter(log => log.module === moduleFilter);
    }

    setFilteredLogs(filtered);
  }, [logs, searchTerm, levelFilter, moduleFilter]);

  // Auto-scroll to top when new logs arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTo({ top: 0, behavior: 'smooth' });
    }
  }, [filteredLogs, autoScroll]);

  const handlePurgeLogs = () => {
    if (confirm('Are you sure you want to purge all system logs? This action cannot be undone.')) {
      setLogs([]);
      setFilteredLogs([]);
    }
  };

  const getLevelIcon = (level: string) => {
    switch (level) {
      case 'error': return <AlertCircle className="w-3 h-3" />;
      case 'warning': return <AlertTriangle className="w-3 h-3" />;
      case 'info': return <Info className="w-3 h-3" />;
      case 'debug': return <CheckCircle className="w-3 h-3" />;
      default: return <Info className="w-3 h-3" />;
    }
  };

  const getLevelColor = (level: string) => {
    switch (level) {
      case 'error': return 'bg-destructive/20 text-destructive border-destructive/30';
      case 'warning': return 'bg-warning/20 text-warning border-warning/30';
      case 'info': return 'bg-primary/20 text-primary border-primary/30';
      case 'debug': return 'bg-muted/20 text-muted-foreground border-muted/30';
      default: return 'bg-muted/20 text-muted-foreground border-muted/30';
    }
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', { 
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
  };

  const getLogCounts = () => {
    const counts = {
      total: logs.length,
      error: logs.filter(l => l.level === 'error').length,
      warning: logs.filter(l => l.level === 'warning').length,
      info: logs.filter(l => l.level === 'info').length,
      debug: logs.filter(l => l.level === 'debug').length
    };
    return counts;
  };

  const counts = getLogCounts();
  const uniqueModules = [...new Set(logs.map(log => log.module))];

  return (
    <Card className="h-full bg-card/50 border-terminal-border terminal-glow">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          <FileText className="w-4 h-4 text-primary" />
          System Logs
          <Badge variant="secondary" className="ml-auto">
            {counts.total} ENTRIES
          </Badge>
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-3">
        {/* Controls */}
        <div className="space-y-2">
          <div className="flex gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-2 top-1/2 transform -translate-y-1/2 w-3 h-3 text-muted-foreground" />
              <Input
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                placeholder="Search logs..."
                className="pl-7 h-7 text-xs"
              />
            </div>
            <Button
              onClick={handlePurgeLogs}
              size="sm"
              variant="outline"
              className="h-7 px-2"
            >
              <Trash2 className="w-3 h-3" />
            </Button>
          </div>
          
          <div className="flex gap-2">
            <select
              value={levelFilter}
              onChange={(e) => setLevelFilter(e.target.value)}
              className="h-7 px-2 text-xs bg-input border border-border rounded"
            >
              <option value="all">All Levels</option>
              <option value="error">Error ({counts.error})</option>
              <option value="warning">Warning ({counts.warning})</option>
              <option value="info">Info ({counts.info})</option>
              <option value="debug">Debug ({counts.debug})</option>
            </select>
            
            <select
              value={moduleFilter}
              onChange={(e) => setModuleFilter(e.target.value)}
              className="h-7 px-2 text-xs bg-input border border-border rounded"
            >
              <option value="all">All Modules</option>
              {uniqueModules.map(module => (
                <option key={module} value={module}>{module}</option>
              ))}
            </select>
          </div>
        </div>

        {/* Log Level Summary */}
        <div className="grid grid-cols-4 gap-1 text-xs">
          <div className="text-center p-1 bg-destructive/10 rounded">
            <div className="text-destructive font-mono">{counts.error}</div>
            <div className="text-muted-foreground">ERR</div>
          </div>
          <div className="text-center p-1 bg-warning/10 rounded">
            <div className="text-warning font-mono">{counts.warning}</div>
            <div className="text-muted-foreground">WARN</div>
          </div>
          <div className="text-center p-1 bg-primary/10 rounded">
            <div className="text-primary font-mono">{counts.info}</div>
            <div className="text-muted-foreground">INFO</div>
          </div>
          <div className="text-center p-1 bg-muted/10 rounded">
            <div className="text-muted-foreground font-mono">{counts.debug}</div>
            <div className="text-muted-foreground">DBG</div>
          </div>
        </div>

        {/* Log Entries */}
        <div 
          ref={scrollRef}
          className="space-y-1 max-h-80 overflow-y-auto"
        >
          {filteredLogs.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <FileText className="w-8 h-8 mx-auto mb-2 opacity-50" />
              <p className="text-sm">No log entries found</p>
            </div>
          ) : (
            filteredLogs.map((log) => (
              <div 
                key={log.id}
                className="p-2 bg-muted/20 border border-border/30 rounded hover:bg-muted/30 transition-colors animate-fade-in"
              >
                <div className="flex items-start gap-2">
                  <Badge className={`h-5 px-1.5 shrink-0 ${getLevelColor(log.level)}`}>
                    {getLevelIcon(log.level)}
                    <span className="ml-1 text-xs">{log.level.toUpperCase()}</span>
                  </Badge>
                  
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 text-xs mb-1">
                      <span className="font-mono text-muted-foreground">
                        {formatTimestamp(log.timestamp)}
                      </span>
                      <Badge variant="outline" className="h-4 px-1 text-xs">
                        {log.module}
                      </Badge>
                    </div>
                    
                    <p className="text-sm text-foreground break-words">
                      {log.message}
                    </p>
                    
                    {log.details && (
                      <p className="text-xs text-muted-foreground mt-1">
                        {log.details}
                      </p>
                    )}
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
};