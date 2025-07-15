#!/usr/bin/env node

import os from 'os';
import fs from 'fs';
import path from 'path';
import process from 'process';
import unzipper from 'unzipper';
import axios, { AxiosResponse } from 'axios';
import { getBinaryDir } from './utils';

const owner = 'anyone-protocol';
const repo = 'ator-protocol'
const version = 'v0.4.9.11';
const releaseUrl = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${version}`;

interface Asset {
  name: string;
  browser_download_url: string;
}

interface Release {
  assets: Asset[];
}

const platformMap: { [name: string]: string } = {
  'linux': 'linux',
  'darwin': 'macos',
  'win32': 'windows-signed',
};

const archMap: { [name: string]: string } = {
  'arm64': 'arm64',
  'x64': 'amd64',
};

const geoipFiles = [
  {
    name: 'geoip',
    url: 'https://raw.githubusercontent.com/anyone-protocol/ator-protocol/refs/heads/main/src/config/geoip',
  },
  {
    name: 'geoip6',
    url: 'https://raw.githubusercontent.com/anyone-protocol/ator-protocol/refs/heads/main/src/config/geoip6',
  },
];

const downloadTextFile = async (url: string, outputPath: string) => {
  const response = await axios.get(url, { responseType: 'stream' });
  const writer = fs.createWriteStream(outputPath);
  response.data.pipe(writer);
  return new Promise<void>((resolve, reject) => {
    writer.on('finish', resolve);
    writer.on('error', reject);
  });
};

const downloadFile = async (url: string, outputPath: string) => {
  try {
    const response = await axios({
      url,
      method: 'GET',
      responseType: 'stream',
    });

    const writer = fs.createWriteStream(outputPath);

    response.data.pipe(writer);

    return new Promise<void>((resolve, reject) => {
      writer.on('finish', resolve);
      writer.on('error', reject);
    });
  } catch (error) {
    console.error('Error downloading the file', error);
    throw error;
  }
};

const unzip = async (zipFilePath: string, outputDir: string): Promise<void> => {
  return new Promise<void>((resolve, reject) => {
    fs.createReadStream(zipFilePath)
      .pipe(unzipper.Extract({ path: outputDir }))
      .on('close', () => {
        resolve();
      })
      .on('error', (err) => {
        reject(err);
      });
  });
};

const listFiles = async (dir: string): Promise<string[]> => {
  return new Promise<string[]>((resolve, reject) => {
    fs.readdir(dir, (err, files) => {
      if (err) {
        reject(err);
      } else {
        resolve(files);
      }
    });
  });
};

const makeExecutable = (file: string) => {
  return new Promise<void>((resolve, reject) => {
    fs.chmod(file, 0o755, (err) => {
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
};

(async () => {

  const resp: AxiosResponse<Release> = await axios.get<Release>(releaseUrl);

  const osPlatform = os.platform();
  if (!platformMap.hasOwnProperty(osPlatform)) {
    console.error(`Platform ${osPlatform} is not supported`);
    process.exit(1);
  }

  const platform = platformMap[osPlatform];

  const osArch = os.arch();
  if (!archMap.hasOwnProperty(osArch)) {
    console.error(`Architecture ${osArch} is not supported`);
    process.exit(1);
  }

  const arch = archMap[osArch];

  const assetName = `anon-live-${platform}-${arch}.zip`;
  const assetDir = `anon-live-${platform}-${arch}`;
  let downloadUrl = '';
  for (const asset of resp.data.assets) {
    if (asset.name === assetName) {
      downloadUrl = asset.browser_download_url;
      console.log(`Download URL: ${asset.browser_download_url}`);
      break;
    }
  }

  if (downloadUrl === '') {
    console.error(`Platform ${platform} (${arch}) is not sipported`);
    process.exit(1);
  }

  const tmpDir = os.tmpdir();
  const downloadDest = path.join(tmpDir, assetName);
  const extractDest = path.join(tmpDir, assetDir);

  console.log(`Platform: ${platform}`);
  console.log(`Arch: ${arch}`);
  console.log(`Download dest: ${downloadDest}`);

  console.log('Downloading anon...');

  await downloadFile(downloadUrl, downloadDest);
  await unzip(downloadDest, extractDest);
  const files = await listFiles(extractDest);
  console.log('Downloaded files:');
  console.log(files);

  const binaryDir = getBinaryDir();
  await fs.promises.mkdir(binaryDir, { recursive: true });

  for (const file of files) {
    await makeExecutable(path.join(extractDest, file));
    fs.promises.copyFile(path.join(extractDest, file), path.join(binaryDir, file));
  }

  // After downloading binary files
  for (const geo of geoipFiles) {
    const dest = path.join(binaryDir, geo.name);
    console.log(`Downloading ${geo.name}...`);
    await downloadTextFile(geo.url, dest);
  }

  console.log('Download complete');
})();
