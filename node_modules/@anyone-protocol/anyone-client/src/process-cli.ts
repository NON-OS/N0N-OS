#!/usr/bin/env node

import { parseArgs } from "util";
import { Process } from "./process";
import { Config } from "./config";



const args = parseArgs({
  options: {
    socksPort: {
      type: 'string',
      short: 's',
    },
    orPort: {
      type: 'string',
      short: 'o',
    },
    controlPort: {
      type: 'string',
      short: 'c',
    },
    verbose: {
      type: 'boolean',
      short: 'v',
    },
    config : {
      type: 'string',
      short: 'f',
    },
    binaryPath: {
      type: 'string',
      short: 'b',
    },
    agree: {
      type: 'boolean',
    },
    termsFilePath: {
      type: 'string',
      short: 't',
    },
  }
});

const config: Config = {
  displayLog: args.values.verbose === true,
  socksPort: 9050,
  orPort: 9001,
  controlPort: 9051,
  binaryPath: args.values.binaryPath,
  autoTermsAgreement: args.values.agree === true,
  termsFilePath: args.values.termsFilePath,
};

function parsePort(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  const parsed = parseInt(value, 10);
  return isFinite(parsed) ? parsed : undefined;
}

const socksPort = parsePort(args.values.socksPort);
if (socksPort !== undefined) config.socksPort = socksPort;

const orPort = parsePort(args.values.orPort);
if (orPort !== undefined) config.orPort = orPort;

const controlPort = parsePort(args.values.controlPort);
if (controlPort !== undefined) config.controlPort = controlPort;

const configFile = args.values.config;
if (configFile !== undefined) config.configFile = configFile;

const termsFilePath = args.values.termsFilePath;
if (termsFilePath !== undefined) config.termsFilePath = termsFilePath;

const anon = new Process(config);

(async () => {
  await anon.start();
})();

function gracefulShutdown() {
  anon.stop();
}

process.on('SIGTERM', gracefulShutdown);
process.on('SIGINT', gracefulShutdown);
