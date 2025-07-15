import { Event, StreamEvent, AddrMapEvent, EventType } from './models';
import { CircuitStatus, Relay, RelayInfo, ExtendCircuitOptions, Purpose, Flag } from './models';
import * as net from 'net';
import { AsyncQueue, AsyncEvent } from './queue';
import { Buffer } from 'buffer';

export class Control {
    private client: net.Socket;
    private isAuthenticated: boolean = false;
    private eventListeners: Map<EventType, Function[]> = new Map();

    private msgLock = new AsyncQueue<void>();
    private replyQueue = new AsyncQueue<string>();
    private eventQueue = new AsyncQueue<string>();
    private eventNotice = new AsyncEvent();

    private readerLoopTask: Promise<void> | null = null;
    private eventLoopTask: Promise<void> | null = null;

    constructor(host = '127.0.0.1', port = 9051) {
        console.log('Connecting to Anon Control Port at', host, port);

        this.client = net.createConnection({ host, port }, () => {
            console.log('Successfully connected to Anon Control Port');
        });

        this.createLoopTasks();
    }

    async authenticate(password: string = 'password'): Promise<void> {
        const response = await this.msg(`AUTHENTICATE "${password}"`);

        if (response.startsWith('250 OK')) {
            this.isAuthenticated = true;
            console.log('Authenticated to Anon Control Port');
        } else if (response.startsWith('515')) {
            throw new Error('Authentication failed');
        } else {
            throw new Error(`Unexpected response: ${response}`);
        }
    }

    async setEvents(events: EventType[]): Promise<boolean> {
        const command = `SETEVENTS ${events.join(' ')}`;

        const response = await this.msg(command);

        if (response.startsWith('250 OK')) {
            return true;
        } else {
            console.error('Error: ', response);
            return false;
        }
    }

    async circuitStatus(): Promise<CircuitStatus[]> {
        return this.msg('GETINFO circuit-status').then(response => {

            if (!response.startsWith('250+circuit-status=') && !response.startsWith('250 OK')) {
                throw new Error('Invalid response format');
            }

            const cleanedResponse = response
                .replace(/^250\+circuit-status=/, '')
                .replace(/250 OK$/, '')

            const circuits: CircuitStatus[] = [];
            const lines = cleanedResponse.split('\n').filter(line => line.trim() !== '');

            for (const line of lines) {
                const trimmedLine = line.trim();
                const parts = trimmedLine.split(' ');

                if (parts.length < 4 || isNaN(parseInt(parts[0], 10))) {
                    continue;
                }

                const state = parts[1];
                const circuitId = parseInt(parts[0], 10);
                const relaysPart = parts.find(part => part.startsWith('$'))?.split(',') || [];
                const relays: Relay[] = relaysPart.map(relay => {
                    const [fingerprint, nickname] = relay.split('~');
                    return {
                        fingerprint: fingerprint.replace(/^\$/, ''),
                        nickname: nickname
                    };
                });

                const buildFlags = parts.find(part => part.startsWith('BUILD_FLAGS='))
                    ?.split('=')[1]?.split(',') || [];
                const purpose = parts.find(part => part.startsWith('PURPOSE='))
                    ?.split('=')[1] || '';
                const timeCreated = new Date(parts.find(part => part.startsWith('TIME_CREATED='))
                    ?.split('=')[1] + 'Z' || ''); // Add Z to make it ISO 8601 compliant

                const circuit: CircuitStatus = {
                    circuitId,
                    state,
                    relays,
                    buildFlags,
                    purpose,
                    timeCreated
                };

                circuits.push(circuit);
            }

            return circuits;
        });
    }

    async getCircuit(circuitId: number): Promise<CircuitStatus> {
        const circuits = await this.circuitStatus();
        const circuit = circuits.find(c => c.circuitId === circuitId);
        if (!circuit) {
            console.error(`Circuit with ID ${circuitId} not found`);
            throw new Error(`Circuit with ID ${circuitId} not found`);
        }
        return circuit;
    }

    async msg(message: string): Promise<string> {
        // Acquire message lock
        await this.msgLock.push();

        try {
            // Flush any old responses/errors in the reply queue
            while (!this.replyQueue.isEmpty) {
                const response = await this.replyQueue.pop();

                if (response.includes('SocketClosed')) {
                    // This is expected sometimes
                    continue;
                } else if (response.includes('ProtocolError')) {
                    console.info('Tor provided a malformed message:', response);
                } else if (response.includes('ControllerError')) {
                    console.info('Socket experienced a problem:', response);
                } else {
                    console.info('Failed to deliver a response:', response);
                }
            }

            // Send the message
            this.client.write(`${message}\r\n`);

            // Wait for reply
            const response = await this.replyQueue.pop();

            // In a real implementation, you'd parse response objects here
            if (response.startsWith('5')) {
                console.error('Error: ', response.substring(0, 100));
            }

            return response;
        } catch (err) {
            if (!this.client || this.client.destroyed) {
                this.end();
                throw new Error('SocketClosed');
            }

            throw err;
        } finally {
            await this.msgLock.pop(); // Release lock
        }
    }

    async resolve(hostname: string): Promise<void> {
        await this.msg(`RESOLVE ${hostname}`);
    }

    async extendCircuit(options: ExtendCircuitOptions = {}): Promise<number> {
        const circuitId: number = options.circuitId ?? 0;
        const serverSpecs: string[] = options.serverSpecs ?? [];
        const purpose: Purpose = options.purpose ?? 'general';
        const awaitBuild: boolean = options.awaitBuild ?? false;
        
        var queue;
        var eventListener: Function | null = null;
        if (awaitBuild) {
            queue = new AsyncQueue<string>();

            eventListener = (event: Event) => {
                if (event.type === EventType.CIRC) {
                    queue.push(event.data!);
                }
            };
            await this.addEventListener(eventListener, EventType.CIRC);
        }

        let command = `EXTENDCIRCUIT ${circuitId}`;

        if (serverSpecs.length > 0) {
            command += ` ${serverSpecs.join(',')}`;
        }

        if (purpose) {
            command += ` purpose=${purpose}`;
        }

        const response = await this.msg(command);

        if (!response.startsWith('250 EXTENDED')) {
            throw new Error('Failed to extend circuit');
        }

        const circId = response.split(' ')[2];

        if (awaitBuild) {
            var received = false;

            let numb = 0;

            while (!received) {
                const event = await queue!.pop();
                const id = event.split(' ')[0];

                if (id === circId) {
                    console.log('Received event', event);
                    numb++;
                    if (numb >= serverSpecs.length) { // todo - fix this (we recevie event on each extended hop) 
                        received = true;
                    }
                }
            }

            await this.removeEventListener(eventListener!);
        }

        return parseInt(circId, 10); // circuitId
    }

    async closeCircuit(circuitId: number): Promise<void> {
        const command = `CLOSECIRCUIT ${circuitId}`;

        const response = await this.msg(command);

        if (!response.startsWith('250')) {
            throw new Error(`Failed to close circuit: ${response}`);
        }
    }

    async getRelayInfo(fingerprint: string): Promise<RelayInfo> {
        const command = `GETINFO ns/id/$${fingerprint}`;
        const response = await this.msg(command);

        if (!response.startsWith('250+ns/id/')) {
            throw new Error(`Failed to get relay address: ${response}`);
        }

        const lines = response.split('\n').map(line => line.trim());

        let flags: Flag[] = [];
        let ip: string = '';
        let orPort: number = 0;
        let bandwidth: number = 0;
        let nickname: string = '';

        for (const line of lines) {
            // Extract flags from the line starting with 's '
            if (line.startsWith('s ')) {
                flags = line.substring(2).trim().split(' ').map(flag => Flag[flag as keyof typeof Flag]);
            }

            // Extract IP and ORPort from the line starting with 'r '
            if (line.startsWith('r ')) {
                const parts = line.split(' ');

                if (parts.length >= 7) {
                    nickname = parts[1];
                    ip = parts[6];
                    orPort = parseInt(parts[7], 10);
                }
            }

            if (line.startsWith('w ')) {
                bandwidth = parseInt(line.split('=')[1], 10);
            }
        }

        return { fingerprint, nickname, ip, orPort, flags, bandwidth };
    }

    end() {
        this.client.write('QUIT\r\n');
        this.client.end();
    }

    async disableStreamAttachment(): Promise<void> {
        await this.setConf('__LeaveStreamsUnattached', '1');
    }

    async enableStreamAttachment(): Promise<void> {
        await this.resetConf('__LeaveStreamsUnattached');
    }

    async disablePredictedCircuits(): Promise<void> {
        await this.setConf('__DisablePredictedCircuits', '1');
    }

    async enablePredictedCircuits(): Promise<void> {
        await this.resetConf('__DisablePredictedCircuits');
    }

    async setConf(param: string, value: string | string[]): Promise<void> {
        await this.setOptions({ [param]: value }, false);
    }

    async resetConf(...params: string[]): Promise<void> {
        const resetOptions: Record<string, null> = {};
        for (const param of params) {
            resetOptions[param] = null;
        }
        await this.setOptions(resetOptions, true);
    }

    private async setOptions(
        options: Record<string, string | string[] | null>,
        reset: boolean
    ): Promise<void> {
        const commandParts: string[] = [reset ? 'RESETCONF' : 'SETCONF'];

        for (const [key, val] of Object.entries(options)) {
            if (val === null || val === undefined) {
                commandParts.push(key); // RESETCONF-style nulling
            } else if (typeof val === 'string') {
                commandParts.push(`${key}="${val.trim()}"`);
            } else if (Array.isArray(val)) {
                for (const item of val) {
                    commandParts.push(`${key}="${item.trim()}"`);
                }
            } else {
                throw new Error(`Invalid config value for ${key}: ${val}`);
            }
        }

        const command = commandParts.join(' ');
        const response = await this.msg(command);

        if (!response.startsWith('250 OK')) {
            throw new Error(`SETCONF/RESETCONF failed: ${response}`);
        }
    }

    async attachStream(streamId: number, circuitId: number, exitingHop?: number): Promise<void> {
        let command = `ATTACHSTREAM ${streamId} ${circuitId}`;

        if (exitingHop !== undefined) {
            command += ` HOP=${exitingHop}`;
        }

        const response = await this.msg(command);

        if (!response.startsWith('250')) {
            if (response.startsWith('552')) {
                throw new Error(`InvalidRequest: ${response}`);
            } else if (response.startsWith('551')) {
                throw new Error(`OperationFailed: ${response}`);
            } else if (response.startsWith('555')) {
                throw new Error(`UnsatisfiableRequest: ${response}`);
            } else {
                throw new Error(`ProtocolError: Unexpected ATTACHSTREAM response: ${response}`);
            }
        }
    }

    private async attachListeners(): Promise<[EventType[], EventType[]]> {
        const setEvents: EventType[] = [];
        const failedEvents: EventType[] = [];

        if (!this.isAuthenticated || !this.client || this.client.destroyed) {
            return [setEvents, failedEvents];
        }

        const eventTypes = Array.from(this.eventListeners?.keys() || []);

        try {
            var isOk = await this.setEvents(eventTypes);
            if (isOk) {
                setEvents.push(...eventTypes);
            } else {
                for (const eventType of eventTypes) {
                    isOk = await this.setEvents([eventType]);
                    if (isOk) {
                        setEvents.push(eventType);
                    } else {
                        failedEvents.push(eventType);
                    }
                }
            }
        } catch (err) {
            console.error('Failed to attach listeners:', err);
            failedEvents.push(...eventTypes);
        }

        return [setEvents, failedEvents];
    }

    private async attachEventListenersOrFail() {
        if (this.eventListeners.size === 0) {
            return;
        }

        const [, failedEventTypes] = await this.attachListeners();

        if (failedEventTypes.length > 0) {
            console.error('Failed to set events:', failedEventTypes);
            for (const event of failedEventTypes) {
                const callbacks = this.eventListeners.get(event);
                if (callbacks) {
                    this.eventListeners.delete(event);
                }
            }

            throw new Error(`Failed to set events: ${failedEventTypes}`);
        }
    }

    async addEventListener(callback: Function, ...eventTypes: EventType[]): Promise<void> {
        for (const eventType of eventTypes) {
            var callbacks: Function[] = this.eventListeners.get(eventType) || [];
            callbacks.push(callback);
            this.eventListeners.set(eventType, callbacks);
        }

        await this.attachEventListenersOrFail();
    }

    async removeEventListener(callback: Function): Promise<void> {
        var eventTypesChanged = false;

        for (const [eventType, callbacks] of this.eventListeners.entries()) {
            const index = callbacks.indexOf(callback);
            if (index !== -1) {
                callbacks.splice(index, 1);
            }

            if (callbacks.length === 0) {
                eventTypesChanged = true;
                this.eventListeners.delete(eventType);
            }
        }

        if (eventTypesChanged) {
            await this.attachEventListenersOrFail();
        }
    }

    private recv(): Promise<string> {
        return new Promise((resolve, reject) => {
            let buffer = '';
            let rawLines: string[] = [];
            let statusCode: string | null = null;
            let divider: string | null = null;
            let inDataBlock = false;

            const onData = (data: Buffer) => {
                buffer += data.toString();
                let lines = buffer.split('\r\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (!statusCode) {
                        if (!/^\d{3}[ +\-]/.test(line)) {
                            cleanup();
                            return reject(new Error(`Malformed initial line: '${line}'`));
                        }
                        statusCode = line.substring(0, 3);
                        divider = line.charAt(3);
                    }

                    if (line.startsWith('..')) {
                        rawLines.push(line.slice(1));
                    } else {
                        rawLines.push(line);
                    }

                    if (line.startsWith(statusCode + ' ')) {
                        cleanup();
                        return resolve(rawLines.join('\r\n'));
                    }

                    if (inDataBlock) {
                        if (line === '.') {
                            inDataBlock = false;
                            continue;
                        }
                    } else {
                        switch (divider) {
                            case ' ':
                                cleanup();
                                return resolve(rawLines.join('\r\n'));

                            case '+':
                                inDataBlock = true;
                                break;

                            case '-':
                                continue

                            default:
                                cleanup();
                                return reject(new Error(`Unknown divider: '${divider}' in line: ${line}`));
                        }
                    }
                }
            };

            const onError = (err: Error) => {
                cleanup();
                reject(err);
            };

            const cleanup = () => {
                this.client.off('data', onData);
                this.client.off('error', onError);
            };

            this.client.on('data', onData);
            this.client.once('error', onError);
        });
    }

    private createLoopTasks(): void {
        if (!this.readerLoopTask) {
            this.readerLoopTask = this.readerLoop();
        }

        if (!this.eventLoopTask) {
            this.eventLoopTask = this.eventLoop();
        }
    }

    private async readerLoop(): Promise<void> {
        while (this.client && !this.client.destroyed) {
            try {
                const message = await this.recv();

                if (message.startsWith('650')) {
                    // Asynchronous event
                    this.eventQueue.push(message.substring(4));
                    this.eventNotice.set();
                } else {
                    // Synchronous reply
                    this.replyQueue.push(message);
                }
            } catch (err: any) {
                this.replyQueue.push(err.toString());
            }
        }
    }

    private convertToEvent(eventMessage: string): Event {
        const parts = eventMessage.split(' ');
        const eventType = EventType[parts[0] as keyof typeof EventType];
        const eventData = parts.slice(1).join(' ');

        // Example parsing logic
        // You can customize this based on the actual event format

        switch (eventType) {
            case EventType.STREAM:
                const [streamId, status, circId, target, ...rest] = parts.slice(1);

                const keywordArgs: Record<string, string> = {};
                for (const arg of rest) {
                    const [key, value] = arg.split('=');
                    if (key && value !== undefined) {
                        keywordArgs[key.toUpperCase()] = value;
                    }
                }

                const event: StreamEvent = {
                    type: eventType,
                    streamId: parseInt(streamId, 10),
                    status,
                    circId,
                    target,
                    sourceAddr: keywordArgs['SOURCE_ADDR'] || null,
                    purpose: keywordArgs['PURPOSE'] || null,
                    reason: keywordArgs['REASON'] || null,
                    remoteReason: keywordArgs['REMOTE_REASON'] || null,
                    source: keywordArgs['SOURCE'] || null
                };

                return event;

            case EventType.ADDRMAP:
                const [address, mappedAddress, expires] = parts.slice(1);
                const addrMapEvent: AddrMapEvent = {
                    type: eventType,
                    address,
                    mappedAddress,
                    expires: expires ? new Date(expires) : undefined
                };
                return addrMapEvent;

            default:
                return {
                    type: eventType,
                    data: eventData,
                };
        }
    }

    private async handleEvent(eventMessage: string): Promise<void> {
        let event: any = null;
        let eventType: EventType;

        try {
            event = this.convertToEvent(eventMessage);  // you’ll implement this parser
            eventType = event.type;
        } catch (err) {
            event = eventMessage;
            eventType = EventType.UNKNOWN;
            console.error(`Tor sent a malformed event (${err}):`, eventMessage);
        }

        // Dispatch to listeners
        const listeners = this.eventListeners.get(eventType);
        if (listeners) {
            for (const listener of listeners) {
                try {
                    const result = listener(event);
                    if (result instanceof Promise) {
                        await result;
                    }
                } catch (err) {
                    console.warn(`Event listener for ${eventType} raised an error:`, err);
                }
            }
        }
    }

    private async eventLoop(): Promise<void> {
        let socketClosedAt: number | null = null;

        while (true) {
            try {
                const eventMessage = await this.eventQueue.pop();

                await this.handleEvent(eventMessage);

                if (!this.client || this.client.destroyed) {
                    if (!socketClosedAt) {
                        socketClosedAt = Date.now();
                    } else if (Date.now() - socketClosedAt > 100) {
                        break;
                    }
                }
            } catch (err) {
                if (!this.client || this.client.destroyed) break;

                try {
                    await Promise.race([
                        this.eventNotice.wait(),
                        new Promise(resolve => setTimeout(resolve, 50)),
                    ]);
                } catch { }
                this.eventNotice.clear();
            }
        }
    }

    async getRelays(): Promise<RelayInfo[]> {
        const response = await this.msg('GETINFO ns/all');

        if (!response.startsWith('250+ns/all=')) {
            throw new Error('Invalid response format');
        }

        const cleanedResponse = response
            .replace(/^250\+ns\/all=/, '')
            .replace(/250 OK$/, '')
            .trim();

        const relays: RelayInfo[] = [];
        const lines = cleanedResponse.split('\n');

        let current: Partial<RelayInfo> = {};

        for (const line of lines) {
            const trimmedLine = line.trim();

            if (trimmedLine.startsWith('r ')) {
                if (current.fingerprint) {
                    relays.push(current as RelayInfo);
                    current = {};
                }
                const [, nickname, fingerprint, , date, time, ip, orPort, dirPort] = trimmedLine.split(' ');

                current.nickname = nickname;
                current.fingerprint = this.base64ToHex(fingerprint);
                current.published = new Date(`${date}T${time}Z`);
                current.ip = ip;
                current.orPort = parseInt(orPort, 10);
                current.dirPort = parseInt(dirPort, 10);
                current.flags = [];
                current.bandwidth = 0;
            } else if (trimmedLine.startsWith('s ')) {
                current.flags = trimmedLine.substring(2).split(' ').map(flag => Flag[flag as keyof typeof Flag]);
            } else if (trimmedLine.startsWith('w ')) {
                const match = trimmedLine.match(/Bandwidth=(\d+)/);
                if (match) {
                    current.bandwidth = parseInt(match[1], 10);
                }
            }
        }

        if (current.fingerprint) {
            relays.push(current as RelayInfo);
        }

        return relays;
    }

    async findFirstByCountry(relays: RelayInfo[], firstCount: number, ...countries: string[]): Promise<RelayInfo[]> {
        const result: RelayInfo[] = [];

        for (const relay of relays) {
            if (firstCount > 0 && result.length >= firstCount) {
                break;
            }

            try {
                const country = await this.getCountry(relay.ip);
                if (countries.includes(country)) {
                    result.push(relay);
                }
            } catch (err) {
                console.warn(`Failed to get country for ${relay.ip}:`, err);
            }
        }

        return result;
    }

    async getRelaysByCountries(...countries: string[]): Promise<RelayInfo[]> {
        const relays = await this.getRelays();
        const result: RelayInfo[] = [];

        for (const relay of relays) {
            try {
                const country = await this.getCountry(relay.ip);
                if (countries.includes(country)) {
                    result.push(relay);
                }
            } catch (err) {
                console.warn(`Failed to get country for ${relay.ip}:`, err);
            }
        }

        return result;
    }

    async populateCountries(relays: RelayInfo[]): Promise<void> {
        for (const relay of relays) {
            if (relay.country) {
                continue;
            }

            try {
                const country = await this.getCountry(relay.ip);
                relay.country = country;
            } catch (err) {
                console.warn(`Failed to get country for ${relay.ip}:`, err);
            }
        }
    }

    async filterRelaysByCountries(relays: RelayInfo[], ...countries: string[]): Promise<RelayInfo[]> {
        countries = countries.map(country => country.toLowerCase());
        const result: RelayInfo[] = [];

        for (const relay of relays) {
            try {
                const country = await this.getCountry(relay.ip);
                if (countries.includes(country)) {
                    result.push(relay);
                }
            } catch (err) {
                console.warn(`Failed to get country for ${relay.ip}:`, err);
            }
        }

        return result;
    }

    filterRelaysByFlags(relays: RelayInfo[], ...flags: Flag[]): RelayInfo[] {
        return relays.filter(relay => {
            return flags.every(flag => relay.flags.includes(flag));
        });
    }

    async getCountry(address: string, timeoutMs: number = 1000): Promise<string> {
        const msgPromise = this.msg(`GETINFO ip-to-country/${address}`);
        const timeout = new Promise<string>((_, reject) =>
            setTimeout(() => reject(new Error('getCountry timeout')), timeoutMs)
        );

        const response = await Promise.race([msgPromise, timeout]);

        if (!response.startsWith('250-ip-to-country/')) {
            throw new Error('Invalid response format');
        }

        const cleanedResponse = response
            .replace(/^250-ip-to-country\//, '')
            .replace(/250 OK$/, '')
            .trim();

        const parts = cleanedResponse.split('=');
        if (parts.length < 2) {
            throw new Error('Invalid response format');
        }

        return parts[1];
    }

    private base64ToHex(identity: string, checkIfFingerprint: boolean = true): string {
        let decoded: Buffer;

        try {
            decoded = Buffer.from(identity, 'base64');
        } catch (err) {
            throw new Error(`Unable to decode identity string '${identity}'`);
        }

        const hex = decoded.toString('hex').toUpperCase();

        if (checkIfFingerprint && !this.isValidFingerprint(hex)) {
            throw new Error(`Decoded '${identity}' to '${hex}', which isn't a valid fingerprint`);
        }

        return hex;
    }

    private isValidFingerprint(hex: string): boolean {
        return /^[A-F0-9]{40}$/.test(hex);
    }
}
