import * as os from 'os';
import path from 'path';

export function getBinaryDir(): string {
  const platform = os.platform();
  const arch = os.arch();

  let binaryDir = path.join(__dirname, '..', 'bin', platform, arch);

  return path.resolve(binaryDir);
}

export function getBinaryPath(binaryName: string): string {
  const platform = os.platform();
  const dir = getBinaryDir();

  let binaryPath = path.join(dir, binaryName);
  if (platform === 'win32') {
    binaryPath += '.exe';
  }

  return path.resolve(binaryPath);
}
