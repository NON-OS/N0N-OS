import { useEffect, useRef, useState, useCallback } from 'react';

export interface SocketMessage {
  type: 'command' | 'output' | 'status' | 'error' | 'log';
  data: any;
  timestamp: number;
}

export interface UseSocketReturn {
  connected: boolean;
  sendMessage: (message: SocketMessage) => void;
  lastMessage: SocketMessage | null;
  connectionStatus: 'connecting' | 'connected' | 'disconnected' | 'error';
}

export const useSocket = (url: string): UseSocketReturn => {
  const [connected, setConnected] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected' | 'error'>('disconnected');
  const [lastMessage, setLastMessage] = useState<SocketMessage | null>(null);
  const ws = useRef<WebSocket | null>(null);
  const reconnectTimeout = useRef<NodeJS.Timeout | null>(null);

  const connect = useCallback(() => {
    try {
      setConnectionStatus('connecting');
      ws.current = new WebSocket(url);

      ws.current.onopen = () => {
        setConnected(true);
        setConnectionStatus('connected');
        console.log('[NØNOS] WebSocket connected');
      };

      ws.current.onmessage = (event) => {
        try {
          const message: SocketMessage = JSON.parse(event.data);
          setLastMessage(message);
        } catch (error) {
          console.error('[NØNOS] Failed to parse WebSocket message:', error);
        }
      };

      ws.current.onclose = () => {
        setConnected(false);
        setConnectionStatus('disconnected');
        console.log('[NØNOS] WebSocket disconnected');
        
        // Auto-reconnect after 3 seconds
        reconnectTimeout.current = setTimeout(() => {
          connect();
        }, 3000);
      };

      ws.current.onerror = (error) => {
        setConnectionStatus('error');
        console.error('[NØNOS] WebSocket error:', error);
      };
    } catch (error) {
      setConnectionStatus('error');
      console.error('[NØNOS] Failed to create WebSocket connection:', error);
    }
  }, [url]);

  const sendMessage = useCallback((message: SocketMessage) => {
    if (ws.current && ws.current.readyState === WebSocket.OPEN) {
      ws.current.send(JSON.stringify(message));
    } else {
      console.warn('[NØNOS] WebSocket not connected, cannot send message');
    }
  }, []);

  useEffect(() => {
    connect();

    return () => {
      if (reconnectTimeout.current) {
        clearTimeout(reconnectTimeout.current);
      }
      if (ws.current) {
        ws.current.close();
      }
    };
  }, [connect]);

  return {
    connected,
    sendMessage,
    lastMessage,
    connectionStatus
  };
};