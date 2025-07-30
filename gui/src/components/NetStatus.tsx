import React, { useState, useEffect } from 'react';
import { Wifi, WifiOff, Users, Activity, Shield, AlertTriangle } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

interface Peer {
  id: string;
  name: string;
  latency: number;
  status: 'connected' | 'syncing' | 'offline';
  trustLevel: number;
}

interface NetworkStats {
  totalPeers: number;
  activePeers: number;
  bytesIn: number;
  bytesOut: number;
  uptime: number;
}

export const NetStatus: React.FC = () => {
  const [isConnected, setIsConnected] = useState(true);
  const [peers, setPeers] = useState<Peer[]>([
    { id: '1', name: 'relay-001.nønos', latency: 15, status: 'connected', trustLevel: 95 },
    { id: '2', name: 'relay-007.nønos', latency: 32, status: 'connected', trustLevel: 87 },
    { id: '3', name: 'relay-015.nønos', latency: 156, status: 'syncing', trustLevel: 92 }
  ]);
  
  const [stats, setStats] = useState<NetworkStats>({
    totalPeers: 3,
    activePeers: 2,
    bytesIn: 1024 * 15,
    bytesOut: 1024 * 8,
    uptime: 142 * 60 * 1000 // 142 minutes in ms
  });

  // Simulate real-time updates
  useEffect(() => {
    const interval = setInterval(() => {
      setStats(prev => ({
        ...prev,
        bytesIn: prev.bytesIn + Math.random() * 500,
        bytesOut: prev.bytesOut + Math.random() * 200,
        uptime: prev.uptime + 1000
      }));
      
      // Randomly update peer latencies
      setPeers(prev => prev.map(peer => ({
        ...peer,
        latency: Math.max(5, peer.latency + (Math.random() - 0.5) * 10)
      })));
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes.toFixed(0)}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
  };

  const formatUptime = (ms: number) => {
    const hours = Math.floor(ms / (1000 * 60 * 60));
    const minutes = Math.floor((ms % (1000 * 60 * 60)) / (1000 * 60));
    return `${hours}h ${minutes}m`;
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'connected': return 'text-success';
      case 'syncing': return 'text-warning';
      case 'offline': return 'text-destructive';
      default: return 'text-muted-foreground';
    }
  };

  const getTrustColor = (level: number) => {
    if (level >= 90) return 'text-success';
    if (level >= 70) return 'text-warning';
    return 'text-destructive';
  };

  return (
    <Card className="h-full bg-card/50 border-terminal-border terminal-glow">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          {isConnected ? (
            <Wifi className="w-4 h-4 text-success" />
          ) : (
            <WifiOff className="w-4 h-4 text-destructive" />
          )}
          Relay Network
          <Badge variant="secondary" className="ml-auto">
            {stats.activePeers}/{stats.totalPeers} ACTIVE
          </Badge>
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        {/* Network Stats */}
        <div className="grid grid-cols-2 gap-3 text-xs">
          <div className="space-y-1">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Uptime:</span>
              <span className="text-success">{formatUptime(stats.uptime)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted-foreground">Data In:</span>
              <span className="text-primary">{formatBytes(stats.bytesIn)}</span>
            </div>
          </div>
          <div className="space-y-1">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Status:</span>
              <span className="text-success">SECURE</span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted-foreground">Data Out:</span>
              <span className="text-accent">{formatBytes(stats.bytesOut)}</span>
            </div>
          </div>
        </div>

        {/* Security Status */}
        <div className="flex items-center gap-2 p-2 bg-success/10 border border-success/20 rounded">
          <Shield className="w-3 h-3 text-success" />
          <span className="text-xs text-success">Zero-Trust Mode Active</span>
        </div>

        {/* Peer List */}
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Users className="w-3 h-3" />
            <span>Active Peers</span>
          </div>
          
          <div className="space-y-1 max-h-32 overflow-y-auto">
            {peers.map((peer) => (
              <div 
                key={peer.id} 
                className="flex items-center justify-between p-2 bg-muted/20 rounded border border-border/30 hover:bg-muted/30 transition-colors"
              >
                <div className="flex items-center gap-2">
                  <div className={`w-2 h-2 rounded-full ${getStatusColor(peer.status)}`}>
                    <div className="w-full h-full rounded-full animate-glow-pulse" />
                  </div>
                  <span className="text-xs font-mono">{peer.name}</span>
                </div>
                
                <div className="flex items-center gap-2 text-xs">
                  <span className="text-muted-foreground">{Math.round(peer.latency)}ms</span>
                  <div className="flex items-center gap-1">
                    <Shield className="w-3 h-3" />
                    <span className={getTrustColor(peer.trustLevel)}>
                      {peer.trustLevel}%
                    </span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Connection Health */}
        <div className="flex items-center gap-2 text-xs">
          <Activity className="w-3 h-3 text-primary" />
          <span className="text-muted-foreground">Network Health:</span>
          <span className="text-success">OPTIMAL</span>
          {stats.activePeers < 2 && (
            <AlertTriangle className="w-3 h-3 text-warning ml-auto" />
          )}
        </div>
      </CardContent>
    </Card>
  );
};