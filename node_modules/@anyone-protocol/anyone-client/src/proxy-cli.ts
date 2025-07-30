#!/usr/bin/env node

/**
 * THIS FEATURE IS EXPERIMENTAL AND IN BETA, USE AT YOUR OWN RISK
 */

import { Proxy } from './proxy';

function parseArgs(): { socksPort?: number, args: string[] } {
  const args = process.argv.slice(2);
  let socksPort: number | undefined;
  const parsedArgs: string[] = [];

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--socks-port' && args[i + 1]) {
      socksPort = parseInt(args[i + 1], 10);
      if (isNaN(socksPort)) {
        throw new Error('Invalid SOCKS port value');
      }
      i++;
    } else {
      parsedArgs.push(args[i]);
    }
  }

  return { socksPort, args: parsedArgs };
}

const { socksPort, args } = parseArgs();

const proxy = new Proxy(socksPort);

(async () => {
  await proxy.start(args);
})();

function gracefulShutdown() {
  if (proxy.isRunning()) {
    proxy.stop();
  }
  process.exit(0);
}

process.on('SIGTERM', gracefulShutdown);
process.on('SIGINT', gracefulShutdown);