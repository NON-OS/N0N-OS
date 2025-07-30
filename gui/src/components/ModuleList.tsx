import React, { useState } from 'react';
import { Play, Pause, Square, Package, Cpu, Settings, Activity, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface Module {
  id: string;
  name: string;
  version: string;
  description: string;
  status: 'running' | 'stopped' | 'error' | 'loading';
  type: 'core' | 'security' | 'network' | 'user';
  memoryUsage: number;
  cpuUsage: number;
  uptime: number;
  lastUpdate: number;
}

export const ModuleList: React.FC = () => {
  const [modules, setModules] = useState<Module[]>([
    {
      id: 'core',
      name: 'core.nonosmod',
      version: '1.0.0',
      description: 'System core and process management',
      status: 'running',
      type: 'core',
      memoryUsage: 45.2,
      cpuUsage: 12.3,
      uptime: 8640000, // 2.4 hours
      lastUpdate: Date.now() - 3600000
    },
    {
      id: 'crypto',
      name: 'crypto.nonosmod',
      version: '1.2.1',
      description: 'Cryptographic functions and key management',
      status: 'running',
      type: 'security',
      memoryUsage: 23.8,
      cpuUsage: 8.7,
      uptime: 7200000, // 2 hours
      lastUpdate: Date.now() - 1800000
    },
    {
      id: 'network',
      name: 'network.nonosmod',
      version: '1.1.0',
      description: 'Relay network and peer communication',
      status: 'running',
      type: 'network',
      memoryUsage: 67.4,
      cpuUsage: 15.9,
      uptime: 6300000, // 1.75 hours
      lastUpdate: Date.now() - 900000
    },
    {
      id: 'terminal',
      name: 'terminal.nonosmod',
      version: '0.9.5',
      description: 'Command line interface and shell',
      status: 'running',
      type: 'user',
      memoryUsage: 18.5,
      cpuUsage: 3.2,
      uptime: 5400000, // 1.5 hours
      lastUpdate: Date.now() - 600000
    },
    {
      id: 'vault',
      name: 'vault.nonosmod',
      version: '2.0.0-beta',
      description: 'Encrypted storage and secret management',
      status: 'stopped',
      type: 'security',
      memoryUsage: 0,
      cpuUsage: 0,
      uptime: 0,
      lastUpdate: Date.now() - 86400000
    }
  ]);

  const handleModuleAction = (moduleId: string, action: 'start' | 'stop' | 'restart') => {
    setModules(prev => prev.map(module => {
      if (module.id === moduleId) {
        switch (action) {
          case 'start':
            return { 
              ...module, 
              status: 'loading' as const,
              lastUpdate: Date.now()
            };
          case 'stop':
            return { 
              ...module, 
              status: 'stopped' as const,
              memoryUsage: 0,
              cpuUsage: 0,
              uptime: 0,
              lastUpdate: Date.now()
            };
          case 'restart':
            return { 
              ...module, 
              status: 'loading' as const,
              uptime: 0,
              lastUpdate: Date.now()
            };
        }
      }
      return module;
    }));

    // Simulate module startup
    if (action === 'start' || action === 'restart') {
      setTimeout(() => {
        setModules(prev => prev.map(module => {
          if (module.id === moduleId) {
            return {
              ...module,
              status: 'running' as const,
              memoryUsage: Math.random() * 50 + 10,
              cpuUsage: Math.random() * 20 + 2,
              uptime: 1000
            };
          }
          return module;
        }));
      }, 2000);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'running': return 'bg-success/20 text-success border-success/30';
      case 'stopped': return 'bg-muted/20 text-muted-foreground border-muted/30';
      case 'error': return 'bg-destructive/20 text-destructive border-destructive/30';
      case 'loading': return 'bg-warning/20 text-warning border-warning/30';
      default: return 'bg-muted/20 text-muted-foreground border-muted/30';
    }
  };

  const getTypeColor = (type: string) => {
    switch (type) {
      case 'core': return 'bg-primary/20 text-primary border-primary/30';
      case 'security': return 'bg-accent/20 text-accent border-accent/30';
      case 'network': return 'bg-warning/20 text-warning border-warning/30';
      case 'user': return 'bg-muted/20 text-muted-foreground border-muted/30';
      default: return 'bg-muted/20 text-muted-foreground border-muted/30';
    }
  };

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'core': return <Cpu className="w-3 h-3" />;
      case 'security': return <AlertTriangle className="w-3 h-3" />;
      case 'network': return <Activity className="w-3 h-3" />;
      case 'user': return <Settings className="w-3 h-3" />;
      default: return <Package className="w-3 h-3" />;
    }
  };

  const formatUptime = (ms: number) => {
    if (ms === 0) return 'Stopped';
    const hours = Math.floor(ms / (1000 * 60 * 60));
    const minutes = Math.floor((ms % (1000 * 60 * 60)) / (1000 * 60));
    const seconds = Math.floor((ms % (1000 * 60)) / 1000);
    
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m ${seconds}s`;
    return `${seconds}s`;
  };

  const formatLastUpdate = (timestamp: number) => {
    const diff = Date.now() - timestamp;
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    
    if (hours > 0) return `${hours}h ago`;
    if (minutes > 0) return `${minutes}m ago`;
    return 'Just now';
  };

  const runningModules = modules.filter(m => m.status === 'running').length;
  const totalMemory = modules.reduce((sum, m) => sum + m.memoryUsage, 0);
  const avgCpu = modules.filter(m => m.status === 'running').reduce((sum, m) => sum + m.cpuUsage, 0) / runningModules || 0;

  return (
    <Card className="h-full bg-card/50 border-terminal-border terminal-glow">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Package className="w-4 h-4 text-primary" />
          Active Modules
          <Badge variant="secondary" className="ml-auto">
            {runningModules}/{modules.length} RUNNING
          </Badge>
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        {/* System Overview */}
        <div className="grid grid-cols-3 gap-2 text-xs">
          <div className="text-center p-2 bg-muted/20 rounded border border-border/30">
            <div className="text-muted-foreground">Memory</div>
            <div className="text-primary font-mono">{totalMemory.toFixed(1)}MB</div>
          </div>
          <div className="text-center p-2 bg-muted/20 rounded border border-border/30">
            <div className="text-muted-foreground">Avg CPU</div>
            <div className="text-accent font-mono">{avgCpu.toFixed(1)}%</div>
          </div>
          <div className="text-center p-2 bg-muted/20 rounded border border-border/30">
            <div className="text-muted-foreground">Status</div>
            <div className="text-success">STABLE</div>
          </div>
        </div>

        {/* Module List */}
        <div className="space-y-2 max-h-96 overflow-y-auto">
          {modules.map((module) => (
            <div 
              key={module.id}
              className="p-3 bg-muted/20 border border-border/30 rounded hover:bg-muted/30 transition-colors"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <Badge className={`h-5 px-1.5 ${getTypeColor(module.type)}`}>
                    {getTypeIcon(module.type)}
                    <span className="ml-1 text-xs">{module.type}</span>
                  </Badge>
                  <span className="text-sm font-mono font-medium">{module.name}</span>
                  <span className="text-xs text-muted-foreground">v{module.version}</span>
                </div>
                
                <Badge className={`h-5 px-2 ${getStatusColor(module.status)}`}>
                  {module.status.toUpperCase()}
                </Badge>
              </div>
              
              <p className="text-xs text-muted-foreground mb-3">{module.description}</p>
              
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4 text-xs">
                  {module.status === 'running' && (
                    <>
                      <span className="text-muted-foreground">
                        MEM: <span className="text-primary font-mono">{module.memoryUsage.toFixed(1)}MB</span>
                      </span>
                      <span className="text-muted-foreground">
                        CPU: <span className="text-accent font-mono">{module.cpuUsage.toFixed(1)}%</span>
                      </span>
                      <span className="text-muted-foreground">
                        UP: <span className="text-success">{formatUptime(module.uptime)}</span>
                      </span>
                    </>
                  )}
                  {module.status === 'stopped' && (
                    <span className="text-muted-foreground">
                      Last updated: {formatLastUpdate(module.lastUpdate)}
                    </span>
                  )}
                </div>
                
                <div className="flex items-center gap-1">
                  {module.status === 'running' ? (
                    <>
                      <Button
                        onClick={() => handleModuleAction(module.id, 'restart')}
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0"
                        title="Restart"
                      >
                        <Activity className="w-3 h-3" />
                      </Button>
                      <Button
                        onClick={() => handleModuleAction(module.id, 'stop')}
                        size="sm"
                        variant="ghost"
                        className="h-6 w-6 p-0"
                        title="Stop"
                      >
                        <Square className="w-3 h-3" />
                      </Button>
                    </>
                  ) : module.status === 'stopped' ? (
                    <Button
                      onClick={() => handleModuleAction(module.id, 'start')}
                      size="sm"
                      variant="ghost"
                      className="h-6 w-6 p-0"
                      title="Start"
                    >
                      <Play className="w-3 h-3" />
                    </Button>
                  ) : (
                    <div className="w-6 h-6 flex items-center justify-center">
                      <div className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin" />
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
};