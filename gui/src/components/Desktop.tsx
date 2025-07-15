import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Terminal, Shield, Lock, Package, FileText, Wifi, Settings, Power, Minimize2, Maximize2, X, Globe, Mail, Calculator, Music, Camera } from 'lucide-react';
import { Console } from './Console';
import { NetStatus } from './NetStatus';
import { VaultView } from './VaultView';
import { ModuleList } from './ModuleList';
import { SystemLogs } from './SystemLogs';

interface AppWindow {
  id: string;
  title: string;
  icon: React.ComponentType<any>;
  component: React.ComponentType<any>;
  isMinimized: boolean;
  isMaximized: boolean;
  position: { x: number; y: number };
  size: { width: number; height: number };
  zIndex: number;
}

interface DesktopIcon {
  id: string;
  title: string;
  icon: React.ComponentType<any>;
  component: React.ComponentType<any>;
}

// App components for modules
const BrowserApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Firefox Browser</h2>
      <div className="space-y-4">
        <div className="flex items-center gap-2 p-3 border rounded-lg">
          <span className="text-sm">🌐 https://example.com</span>
        </div>
        <div className="h-64 bg-muted rounded border flex items-center justify-center">
          <span className="text-muted-foreground">Web content would appear here</span>
        </div>
      </div>
    </div>
  </div>
);

const MailApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Thunderbird Mail</h2>
      <div className="text-muted-foreground">Email client interface...</div>
    </div>
  </div>
);

const CalcApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Calculator</h2>
      <div className="grid grid-cols-4 gap-2 max-w-xs">
        {['7','8','9','/','4','5','6','*','1','2','3','-','0','.','=','+'].map(btn => (
          <button key={btn} className="aspect-square bg-muted hover:bg-muted/80 rounded border">
            {btn}
          </button>
        ))}
      </div>
    </div>
  </div>
);

const MusicApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Music Player</h2>
      <div className="text-muted-foreground">Music player interface...</div>
    </div>
  </div>
);

const NotesApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Notes</h2>
      <textarea className="w-full h-32 p-2 border rounded resize-none" placeholder="Write your notes..."/>
    </div>
  </div>
);

const PhotosApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">Photos</h2>
      <div className="text-muted-foreground">Photo gallery interface...</div>
    </div>
  </div>
);

// Settings App component
const SettingsApp = () => (
  <div className="p-6 h-full bg-background">
    <div className="h-full bg-card rounded-lg border p-4">
      <h2 className="text-xl font-bold mb-4 text-primary">NØNOS Settings</h2>
      <div className="space-y-4">
        <div className="border rounded-lg p-3">
          <h3 className="font-semibold mb-2">Display</h3>
          <div className="text-sm text-muted-foreground">
            <p>• Resolution: 1920x1080</p>
            <p>• Wallpaper: NØNOS Collection</p>
            <p>• Theme: Dark Mode</p>
          </div>
        </div>
        <div className="border rounded-lg p-3">
          <h3 className="font-semibold mb-2">Security</h3>
          <div className="text-sm text-muted-foreground">
            <p>• Zero-Trust: Enabled</p>
            <p>• Encryption: AES-256-GCM</p>
            <p>• Firewall: Active</p>
          </div>
        </div>
        <div className="border rounded-lg p-3">
          <h3 className="font-semibold mb-2">System</h3>
          <div className="text-sm text-muted-foreground">
            <p>• Version: NØNOS v1.0.0</p>
            <p>• Uptime: 42 minutes</p>
            <p>• Memory: 4.2GB / 16GB</p>
          </div>
        </div>
      </div>
    </div>
  </div>
);

const defaultDesktopIcons: DesktopIcon[] = [
  { id: 'terminal', title: 'Terminal', icon: Terminal, component: Console },
  { id: 'network', title: 'Network', icon: Shield, component: NetStatus },
  { id: 'vault', title: 'Vault', icon: Lock, component: VaultView },
  { id: 'modules', title: 'Modules', icon: Package, component: ModuleList },
  { id: 'logs', title: 'System Logs', icon: FileText, component: SystemLogs },
];

const moduleApps: { [key: string]: DesktopIcon } = {
  'nonos_module_browser': { id: 'browser', title: 'Firefox', icon: Globe, component: BrowserApp },
  'nonos_module_mail': { id: 'mail', title: 'Thunderbird', icon: Mail, component: MailApp },
  'nonos_module_calc': { id: 'calculator', title: 'Calculator', icon: Calculator, component: CalcApp },
  'nonos_module_music': { id: 'music', title: 'Music', icon: Music, component: MusicApp },
  'nonos_module_notes': { id: 'notes', title: 'Notes', icon: FileText, component: NotesApp },
  'nonos_module_photos': { id: 'photos', title: 'Photos', icon: Camera, component: PhotosApp },
};

export const Desktop = () => {
  const [windows, setWindows] = useState<AppWindow[]>([]);
  const [desktopIcons, setDesktopIcons] = useState<DesktopIcon[]>(defaultDesktopIcons);
  const [nextZIndex, setNextZIndex] = useState(100);
  const [time, setTime] = useState(new Date());
  const dragRef = useRef<{ 
    isDragging: boolean; 
    windowId: string; 
    offset: { x: number; y: number };
    startPos: { x: number; y: number };
  }>({
    isDragging: false,
    windowId: '',
    offset: { x: 0, y: 0 },
    startPos: { x: 0, y: 0 }
  });

  // Listen for module installation events
  useEffect(() => {
    const handleModuleInstall = (event: CustomEvent) => {
      const command = event.detail.command;
      const moduleApp = moduleApps[command];
      
      if (moduleApp && !desktopIcons.find(icon => icon.id === moduleApp.id)) {
        setDesktopIcons(prev => [...prev, moduleApp]);
      }
    };

    window.addEventListener('moduleInstalled', handleModuleInstall as EventListener);
    return () => {
      window.removeEventListener('moduleInstalled', handleModuleInstall as EventListener);
    };
  }, [desktopIcons]);

  // Update time every second
  useEffect(() => {
    const timer = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  const openWindow = useCallback((icon: DesktopIcon) => {
    const existingWindow = windows.find(w => w.id === icon.id);
    if (existingWindow) {
      // Bring to front and unminimize
      setWindows(prev => prev.map(w => 
        w.id === existingWindow.id 
          ? { ...w, isMinimized: false, zIndex: nextZIndex }
          : w
      ));
      setNextZIndex(prev => prev + 1);
      return;
    }

    const screenWidth = window.innerWidth;
    const screenHeight = window.innerHeight;
    
    // Responsive window sizing
    let windowWidth, windowHeight;
    if (screenWidth < 640) { // Mobile
      windowWidth = screenWidth - 20;
      windowHeight = screenHeight - 120;
    } else if (screenWidth < 1024) { // Tablet
      windowWidth = Math.min(600, screenWidth - 60);
      windowHeight = Math.min(500, screenHeight - 140);
    } else { // Desktop
      windowWidth = Math.min(800, screenWidth - 100);
      windowHeight = Math.min(600, screenHeight - 150);
    }

    // Responsive positioning
    const posX = screenWidth < 640 ? 10 : Math.max(50, (screenWidth - windowWidth) / 2 + windows.length * 30);
    const posY = screenWidth < 640 ? 50 : Math.max(50, 100 + windows.length * 30);

    const newWindow: AppWindow = {
      id: icon.id,
      title: icon.title,
      icon: icon.icon,
      component: icon.component,
      isMinimized: false,
      isMaximized: false,
      position: { x: posX, y: posY },
      size: { width: windowWidth, height: windowHeight },
      zIndex: nextZIndex
    };

    setWindows(prev => [...prev, newWindow]);
    setNextZIndex(prev => prev + 1);
  }, [windows, nextZIndex]);

  const closeWindow = useCallback((windowId: string) => {
    setWindows(prev => prev.filter(w => w.id !== windowId));
  }, []);

  const toggleMinimize = useCallback((windowId: string) => {
    setWindows(prev => prev.map(w => 
      w.id === windowId ? { ...w, isMinimized: !w.isMinimized } : w
    ));
  }, []);

  const toggleMaximize = useCallback((windowId: string) => {
    setWindows(prev => prev.map(w => 
      w.id === windowId ? { 
        ...w, 
        isMaximized: !w.isMaximized,
        position: w.isMaximized ? w.position : { x: 0, y: 0 },
        size: w.isMaximized ? w.size : { width: window.innerWidth, height: window.innerHeight - 80 }
      } : w
    ));
  }, []);

  const bringToFront = useCallback((windowId: string) => {
    setWindows(prev => prev.map(w => 
      w.id === windowId ? { ...w, zIndex: nextZIndex } : w
    ));
    setNextZIndex(prev => prev + 1);
  }, [nextZIndex]);

  const startDrag = useCallback((e: React.MouseEvent, windowId: string) => {
    e.preventDefault();
    const window = windows.find(w => w.id === windowId);
    if (!window || window.isMaximized) return;

    dragRef.current = {
      isDragging: true,
      windowId,
      offset: {
        x: e.clientX - window.position.x,
        y: e.clientY - window.position.y
      },
      startPos: {
        x: e.clientX,
        y: e.clientY
      }
    };

    bringToFront(windowId);
  }, [windows, bringToFront]);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!dragRef.current.isDragging) return;

    const newX = Math.max(0, Math.min(e.clientX - dragRef.current.offset.x, window.innerWidth - 200));
    const newY = Math.max(0, Math.min(e.clientY - dragRef.current.offset.y, window.innerHeight - 100));

    setWindows(prev => prev.map(w => 
      w.id === dragRef.current.windowId 
        ? { ...w, position: { x: newX, y: newY } }
        : w
    ));
  }, []);

  const handleMouseUp = useCallback(() => {
    dragRef.current.isDragging = false;
  }, []);

  useEffect(() => {
    if (dragRef.current.isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      
      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [handleMouseMove, handleMouseUp]);

  return (
    <div className="h-screen w-screen overflow-hidden relative select-none">
      {/* NØNOS Full Desktop Wallpaper */}
      <div 
        className="absolute inset-0 bg-cover bg-center bg-no-repeat"
        style={{
          backgroundImage: `url('/nonos-uploads/31a5029d-bb4a-42b3-9005-6f5ff4ff9bfd.png')`,
          backgroundSize: 'cover',
          backgroundPosition: 'center center'
        }}
      >
        {/* Subtle overlay for better UI readability */}
        <div className="absolute inset-0 bg-black/20"></div>
        
        {/* Bottom gradient for taskbar separation */}
        <div className="absolute bottom-0 left-0 right-0 h-24 bg-gradient-to-t from-black/40 to-transparent"></div>
      </div>

      {/* Desktop Icons - Left Side */}
      <div className="absolute top-4 left-4 md:top-8 md:left-8 grid grid-cols-2 sm:grid-cols-1 gap-3 md:gap-6 z-10 max-w-[200px] sm:max-w-none">
        {desktopIcons.map((icon) => (
          <div
            key={icon.id}
            className="flex flex-col items-center cursor-pointer group transition-all duration-200"
            onDoubleClick={() => openWindow(icon)}
          >
            <div className="w-12 h-12 sm:w-14 sm:h-14 md:w-16 md:h-16 bg-gradient-to-br from-card/90 to-card/70 backdrop-blur-xl border border-primary/20 rounded-xl flex items-center justify-center mb-1 md:mb-2 group-hover:border-primary/60 group-hover:shadow-[0_0_20px_hsl(var(--primary)/0.3)] transition-all duration-300 group-hover:scale-105">
              <icon.icon className="w-6 h-6 sm:w-7 sm:h-7 md:w-8 md:h-8 text-primary group-hover:text-accent transition-colors duration-200" />
            </div>
            <span className="text-xs sm:text-xs md:text-xs text-foreground/90 text-center max-w-16 sm:max-w-20 truncate font-medium">{icon.title}</span>
          </div>
        ))}
      </div>

      {/* Settings Icon - Right Side */}
      <div className="absolute top-4 right-4 md:top-8 md:right-8 z-10">
        <div
          className="flex flex-col items-center cursor-pointer group transition-all duration-200"
          onDoubleClick={() => openWindow({ id: 'settings', title: 'Settings', icon: Settings, component: SettingsApp })}
        >
          <div className="w-12 h-12 sm:w-14 sm:h-14 md:w-16 md:h-16 bg-gradient-to-br from-card/90 to-card/70 backdrop-blur-xl border border-primary/20 rounded-xl flex items-center justify-center mb-1 md:mb-2 group-hover:border-primary/60 group-hover:shadow-[0_0_20px_hsl(var(--primary)/0.3)] transition-all duration-300 group-hover:scale-105">
            <Settings className="w-6 h-6 sm:w-7 sm:h-7 md:w-8 md:h-8 text-primary group-hover:text-accent transition-colors duration-200" />
          </div>
          <span className="text-xs sm:text-xs md:text-xs text-foreground/90 text-center max-w-16 sm:max-w-20 truncate font-medium">Settings</span>
        </div>
      </div>

      {/* Windows - Responsive */}
      {windows.map((window) => {
        if (window.isMinimized) return null;
        
        const WindowComponent = window.component;
        const isMobile = typeof globalThis !== 'undefined' && globalThis.innerWidth < 640;
        
        return (
          <div
            key={window.id}
            className={`absolute bg-gradient-to-br from-card/95 to-card/85 backdrop-blur-2xl border border-primary/20 rounded-xl shadow-[0_20px_40px_hsl(var(--background)/0.8)] overflow-hidden transition-all duration-200 ${
              isMobile ? 'inset-2' : ''
            }`}
            style={{
              left: isMobile ? 10 : window.position.x,
              top: isMobile ? 50 : window.position.y,
              width: isMobile ? 'calc(100vw - 20px)' : window.size.width,
              height: isMobile ? 'calc(100vh - 120px)' : window.size.height,
              zIndex: window.zIndex
            }}
            onClick={() => bringToFront(window.id)}
          >
            {/* Ultra-tech Window Title Bar - Responsive */}
            <div
              className="h-10 sm:h-12 bg-gradient-to-r from-muted/80 to-muted/60 backdrop-blur-xl border-b border-primary/20 flex items-center justify-between px-2 sm:px-4 cursor-move group relative overflow-hidden"
              onMouseDown={(e) => !isMobile && startDrag(e, window.id)}
            >
              {/* Animated background glow */}
              <div className="absolute inset-0 bg-gradient-to-r from-primary/5 to-accent/5 opacity-0 group-hover:opacity-100 transition-opacity duration-300"></div>
              
              <div className="flex items-center space-x-2 sm:space-x-3 relative z-10">
                <div className="w-6 h-6 sm:w-8 sm:h-8 bg-primary/20 rounded-lg border border-primary/30 flex items-center justify-center">
                  <window.icon className="w-3 h-3 sm:w-4 sm:h-4 text-primary" />
                </div>
                <span className="text-xs sm:text-sm font-semibold text-foreground truncate">{window.title}</span>
              </div>
              
              <div className="flex items-center space-x-1 sm:space-x-2 relative z-10">
                {!isMobile && (
                  <button
                    className="w-5 h-5 sm:w-6 sm:h-6 rounded-lg bg-warning/20 border border-warning/40 hover:bg-warning/30 hover:border-warning/60 transition-all duration-200 flex items-center justify-center group"
                    onClick={(e) => { e.stopPropagation(); toggleMinimize(window.id); }}
                  >
                    <Minimize2 className="w-2 h-2 sm:w-3 sm:h-3 text-warning group-hover:text-warning/80" />
                  </button>
                )}
                {!isMobile && (
                  <button
                    className="w-5 h-5 sm:w-6 sm:h-6 rounded-lg bg-success/20 border border-success/40 hover:bg-success/30 hover:border-success/60 transition-all duration-200 flex items-center justify-center group"
                    onClick={(e) => { e.stopPropagation(); toggleMaximize(window.id); }}
                  >
                    <Maximize2 className="w-2 h-2 sm:w-3 sm:h-3 text-success group-hover:text-success/80" />
                  </button>
                )}
                <button
                  className="w-5 h-5 sm:w-6 sm:h-6 rounded-lg bg-destructive/20 border border-destructive/40 hover:bg-destructive/30 hover:border-destructive/60 transition-all duration-200 flex items-center justify-center group"
                  onClick={(e) => { e.stopPropagation(); closeWindow(window.id); }}
                >
                  <X className="w-2 h-2 sm:w-3 sm:h-3 text-destructive group-hover:text-destructive/80" />
                </button>
              </div>
            </div>
            
            {/* Window Content - Responsive */}
            <div className="h-[calc(100%-2.5rem)] sm:h-[calc(100%-3rem)] overflow-auto bg-gradient-to-br from-background/50 to-terminal-bg/30">
              <WindowComponent />
            </div>
          </div>
        );
      })}

      {/* Ultra High-Tech Responsive Taskbar */}
      <div className="absolute bottom-0 left-0 right-0 h-16 sm:h-20 bg-gradient-to-t from-card/95 to-card/80 backdrop-blur-2xl border-t border-primary/20 flex items-center justify-between px-2 sm:px-6 shadow-[0_-10px_30px_hsl(var(--background)/0.5)]">
        {/* NØNOS Logo & Branding */}
        <div className="flex items-center space-x-2 sm:space-x-3">
          <div className="w-10 h-10 sm:w-12 sm:h-12 bg-gradient-to-br from-primary/30 to-accent/20 rounded-xl border border-primary/40 flex items-center justify-center shadow-[0_0_15px_hsl(var(--primary)/0.3)]">
            <span className="text-base sm:text-lg font-bold text-primary">›N</span>
          </div>
          <div className="hidden sm:block">
            <span className="text-lg font-bold bg-gradient-to-r from-primary to-accent bg-clip-text text-transparent">NØNOS</span>
            <div className="text-xs text-muted-foreground">Zero-Trust OS</div>
          </div>
        </div>

        {/* Open Windows - Center - Responsive */}
        <div className="flex items-center space-x-1 sm:space-x-3 flex-1 justify-center overflow-x-auto max-w-[50%] sm:max-w-none">
          {windows.map((window) => (
            <button
              key={window.id}
              className={`px-2 sm:px-4 py-2 rounded-lg text-xs flex items-center space-x-1 sm:space-x-2 transition-all duration-200 border shrink-0 ${
                window.isMinimized 
                  ? 'bg-muted/50 text-muted-foreground border-muted/30 hover:bg-muted/70' 
                  : 'bg-primary/20 text-primary border-primary/30 shadow-[0_0_10px_hsl(var(--primary)/0.2)] hover:bg-primary/30'
              }`}
              onClick={() => {
                if (window.isMinimized) {
                  toggleMinimize(window.id);
                }
                bringToFront(window.id);
              }}
            >
              <window.icon className="w-3 h-3 sm:w-4 sm:h-4" />
              <span className="font-medium hidden sm:inline truncate max-w-[80px]">{window.title}</span>
            </button>
          ))}
        </div>

        {/* System Tray - Responsive */}
        <div className="flex items-center space-x-2 sm:space-x-4 text-sm">
          <div className="hidden sm:flex items-center space-x-2 px-3 py-2 rounded-lg bg-success/20 border border-success/30">
            <Wifi className="w-4 h-4 text-success" />
            <span className="text-success font-medium">SECURE</span>
          </div>
          
          {/* Mobile: Show only status icon */}
          <div className="sm:hidden">
            <Wifi className="w-4 h-4 text-success" />
          </div>
          
          <div className="text-foreground font-mono text-sm sm:text-lg">
            {time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
          </div>
          
          <div className="flex items-center space-x-1 sm:space-x-2">
            <Power 
              className="w-4 h-4 sm:w-5 sm:h-5 text-muted-foreground hover:text-destructive cursor-pointer transition-colors duration-200"
              onClick={() => {
                if (confirm('Are you sure you want to shut down NØNOS?')) {
                  window.location.reload();
                }
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};
