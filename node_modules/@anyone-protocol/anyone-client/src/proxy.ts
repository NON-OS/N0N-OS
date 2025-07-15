/**
 * THIS FEATURE IS EXPERIMENTAL AND IN BETA, USE AT YOUR OWN RISK
 */

import { ChildProcess, spawn } from 'child_process';
import { createProxyConfigFile } from './config';
import { getBinaryPath } from './utils';
import os from 'os';

/**
 * Allows to run Anon Proxy with different configuration options
 */
export class Proxy {
  private socksPort?: number;
  private process?: ChildProcess;

  public constructor(socksPort?: number) {
    this.socksPort = socksPort;
  };

  /**
   * Starts Anon Proxy
   */
  public async start(args: string[]) {
    if (this.process !== undefined) {
      throw new Error('Anon process already started');
    }

    const configPath = await createProxyConfigFile(this.socksPort);
    const osArch = os.arch();
    if (os.platform() === 'win32') {
      this.process = this.runBinary(`anon-proxy_${osArch}`, args, configPath, () => this.onStop());
    } else {
      this.process = this.runBinary('anon-proxy', args, configPath, () => this.onStop());
    }
  }

  /**
   * Stops Anon Proxy
   */
  public async stop() {
    if (this.process !== undefined) {
      this.process.kill('SIGTERM');
    }
  }

  /**
   * Allows to check if Anon Proxy is running
   * @returns true if Anon Proxy is running
   */
  public isRunning(): boolean {
    return this.process !== undefined;
  }

  private onStop() {
    this.process = undefined;
  }

  private runBinary(name: string, args: string[], configPath?: string, onStop?: VoidFunction): ChildProcess {
    const binaryPath = getBinaryPath(name);

    let proxyArgs: string[] = [];
    if (configPath !== undefined) {
      proxyArgs = ['-f', configPath]
    }

    const child = spawn(binaryPath, proxyArgs.concat(args), { detached: false });

    child.stdout.on('data', (data) => {
      console.log(`${data}`);
    });

    child.stderr.on('data', (data) => {
      console.log(`${data}`);
    });

    child.on('close', (code) => {
      if (onStop !== undefined) {
        onStop();
      }
    });

    child.on('exit', (code) => {
      if (onStop !== undefined) {
        onStop();
      }
    });

    return child;
  }
}
