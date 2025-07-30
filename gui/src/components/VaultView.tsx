import React, { useState } from 'react';
import { Lock, Unlock, Key, Eye, EyeOff, Shield, AlertCircle, Copy, Check } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface Secret {
  id: string;
  name: string;
  type: 'password' | 'key' | 'token' | 'note';
  encrypted: string;
  decrypted?: string;
  lastAccessed: number;
  accessCount: number;
}

export const VaultView: React.FC = () => {
  const [isUnlocked, setIsUnlocked] = useState(false);
  const [passphrase, setPassphrase] = useState('');
  const [isUnlocking, setIsUnlocking] = useState(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [visibleSecrets, setVisibleSecrets] = useState<Set<string>>(new Set());
  
  const [secrets] = useState<Secret[]>([
    {
      id: '1',
      name: 'Relay Master Key',
      type: 'key',
      encrypted: 'AES256:7f4a9b2c8e1d6f3a9b2c8e1d6f3a9b2c...',
      decrypted: 'sk_live_51H7YvF2eZvKYlo2C0tqzqWQJxvKs2aGT...',
      lastAccessed: Date.now() - 3600000,
      accessCount: 12
    },
    {
      id: '2',
      name: 'Database Password',
      type: 'password',
      encrypted: 'AES256:9c3f1a8d4b2e7c5f8a1d4b2e7c5f8a1d...',
      decrypted: 'Tr0ub4dor&3_Secure!',
      lastAccessed: Date.now() - 7200000,
      accessCount: 5
    },
    {
      id: '3',
      name: 'API Token',
      type: 'token',
      encrypted: 'AES256:2e8b5c9f1a7d3e8b5c9f1a7d3e8b5c9f...',
      decrypted: 'ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx',
      lastAccessed: Date.now() - 1800000,
      accessCount: 28
    },
    {
      id: '4',
      name: 'Recovery Seed',
      type: 'note',
      encrypted: 'AES256:6d2a9f4c7e1b8d2a9f4c7e1b8d2a9f4c...',
      decrypted: 'witch collapse practice feed shame open despair creek road again ice least',
      lastAccessed: Date.now() - 86400000,
      accessCount: 2
    }
  ]);

  const handleUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!passphrase.trim()) return;
    
    setIsUnlocking(true);
    
    // Simulate decryption process
    await new Promise(resolve => setTimeout(resolve, 1500));
    
    // Simple passphrase check for demo
    if (passphrase === 'demo' || passphrase === 'nønos') {
      setIsUnlocked(true);
      setPassphrase('');
    } else {
      alert('Invalid passphrase. Try "demo" or "nønos"');
    }
    
    setIsUnlocking(false);
  };

  const handleLock = () => {
    setIsUnlocked(false);
    setVisibleSecrets(new Set());
    setCopiedId(null);
  };

  const toggleSecretVisibility = (secretId: string) => {
    const newVisible = new Set(visibleSecrets);
    if (newVisible.has(secretId)) {
      newVisible.delete(secretId);
    } else {
      newVisible.add(secretId);
    }
    setVisibleSecrets(newVisible);
  };

  const copyToClipboard = async (secret: Secret) => {
    if (!secret.decrypted) return;
    
    try {
      await navigator.clipboard.writeText(secret.decrypted);
      setCopiedId(secret.id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  };

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'key': return <Key className="w-3 h-3" />;
      case 'password': return <Lock className="w-3 h-3" />;
      case 'token': return <Shield className="w-3 h-3" />;
      case 'note': return <AlertCircle className="w-3 h-3" />;
      default: return <Lock className="w-3 h-3" />;
    }
  };

  const getTypeColor = (type: string) => {
    switch (type) {
      case 'key': return 'bg-primary/20 text-primary border-primary/30';
      case 'password': return 'bg-accent/20 text-accent border-accent/30';
      case 'token': return 'bg-warning/20 text-warning border-warning/30';
      case 'note': return 'bg-muted/20 text-muted-foreground border-muted/30';
      default: return 'bg-muted/20 text-muted-foreground border-muted/30';
    }
  };

  const formatLastAccessed = (timestamp: number) => {
    const diff = Date.now() - timestamp;
    const hours = Math.floor(diff / (1000 * 60 * 60));
    const days = Math.floor(hours / 24);
    
    if (days > 0) return `${days}d ago`;
    if (hours > 0) return `${hours}h ago`;
    return 'Just now';
  };

  if (!isUnlocked) {
    return (
      <Card className="h-full bg-card/50 border-terminal-border terminal-glow">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-sm">
            <Lock className="w-4 h-4 text-warning" />
            Encrypted Vault
            <Badge variant="secondary" className="ml-auto bg-warning/20 text-warning">
              LOCKED
            </Badge>
          </CardTitle>
        </CardHeader>
        
        <CardContent className="flex flex-col items-center justify-center space-y-6 py-8">
          <div className="text-center space-y-2">
            <Shield className="w-12 h-12 text-warning mx-auto" />
            <h3 className="text-lg font-semibold">Vault Access Required</h3>
            <p className="text-sm text-muted-foreground">
              Enter your master passphrase to decrypt stored secrets
            </p>
          </div>
          
          <form onSubmit={handleUnlock} className="w-full max-w-xs space-y-4">
            <Input
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="Master passphrase..."
              className="text-center"
              disabled={isUnlocking}
            />
            
            <Button 
              type="submit" 
              className="w-full" 
              disabled={isUnlocking || !passphrase.trim()}
            >
              {isUnlocking ? (
                <>
                  <Key className="w-4 h-4 mr-2 animate-spin" />
                  Decrypting...
                </>
              ) : (
                <>
                  <Unlock className="w-4 h-4 mr-2" />
                  Unlock Vault
                </>
              )}
            </Button>
          </form>
          
          <div className="text-xs text-muted-foreground text-center">
            <p>Demo: Use "demo" or "nønos" as passphrase</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="h-full bg-card/50 border-terminal-border terminal-glow">
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Unlock className="w-4 h-4 text-success" />
          Encrypted Vault
          <Badge variant="secondary" className="ml-auto bg-success/20 text-success">
            UNLOCKED
          </Badge>
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted-foreground">
            {secrets.length} secrets available
          </span>
          <Button 
            onClick={handleLock} 
            size="sm" 
            variant="outline"
            className="h-6 px-2 text-xs"
          >
            <Lock className="w-3 h-3 mr-1" />
            Lock
          </Button>
        </div>
        
        <div className="space-y-2 max-h-80 overflow-y-auto">
          {secrets.map((secret) => (
            <div 
              key={secret.id}
              className="p-3 bg-muted/20 border border-border/30 rounded hover:bg-muted/30 transition-colors"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <Badge className={`h-5 px-1.5 ${getTypeColor(secret.type)}`}>
                    {getTypeIcon(secret.type)}
                    <span className="ml-1 text-xs">{secret.type}</span>
                  </Badge>
                  <span className="text-sm font-medium">{secret.name}</span>
                </div>
                
                <div className="flex items-center gap-1">
                  <Button
                    onClick={() => toggleSecretVisibility(secret.id)}
                    size="sm"
                    variant="ghost"
                    className="h-6 w-6 p-0"
                  >
                    {visibleSecrets.has(secret.id) ? (
                      <EyeOff className="w-3 h-3" />
                    ) : (
                      <Eye className="w-3 h-3" />
                    )}
                  </Button>
                  
                  <Button
                    onClick={() => copyToClipboard(secret)}
                    size="sm"
                    variant="ghost"
                    className="h-6 w-6 p-0"
                  >
                    {copiedId === secret.id ? (
                      <Check className="w-3 h-3 text-success" />
                    ) : (
                      <Copy className="w-3 h-3" />
                    )}
                  </Button>
                </div>
              </div>
              
              <div className="text-xs space-y-1">
                <div className="font-mono p-2 bg-terminal-bg border border-terminal-border rounded">
                  {visibleSecrets.has(secret.id) ? (
                    <span className="text-primary break-all">
                      {secret.decrypted}
                    </span>
                  ) : (
                    <span className="text-muted-foreground">
                      {secret.encrypted}
                    </span>
                  )}
                </div>
                
                <div className="flex justify-between text-muted-foreground">
                  <span>Last accessed: {formatLastAccessed(secret.lastAccessed)}</span>
                  <span>Used {secret.accessCount} times</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
};