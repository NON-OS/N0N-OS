export interface VPNConfig {
    routings: VPNRouting[];
}

export interface VPNRouting {
    targetAddress: string;
    exitCountries: string[];
}

export interface CircuitStatus {
    circuitId: number;
    state: string;
    relays: Relay[];
    buildFlags: string[];
    purpose: string;
    timeCreated: Date;
}

export interface Relay {
    fingerprint: string;
    nickname: string;
}

export type Purpose = 'general' | 'controller';

export enum EventType {
    ADDRMAP = 'ADDRMAP',
    BUILDTIMEOUT_SET = 'BUILDTIMEOUT_SET',
    BW = 'BW',
    CELL_STATS = 'CELL_STATS',
    CIRC = 'CIRC',
    CIRC_BW = 'CIRC_BW',
    CIRC_MINOR = 'CIRC_MINOR',
    CONF_CHANGED = 'CONF_CHANGED',
    CONN_BW = 'CONN_BW',
    CLIENTS_SEEN = 'CLIENTS_SEEN',
    DEBUG = 'DEBUG',
    DESCCHANGED = 'DESCCHANGED',
    ERR = 'ERR',
    GUARD = 'GUARD',
    HS_DESC = 'HS_DESC',
    HS_DESC_CONTENT = 'HS_DESC_CONTENT',
    INFO = 'INFO',
    NETWORK_LIVENESS = 'NETWORK_LIVENESS',
    NEWCONSENSUS = 'NEWCONSENSUS',
    NEWDESC = 'NEWDESC',
    NOTICE = 'NOTICE',
    NS = 'NS',
    ORCONN = 'ORCONN',
    SIGNAL = 'SIGNAL',
    STATUS_CLIENT = 'STATUS_CLIENT',
    STATUS_GENERAL = 'STATUS_GENERAL',
    STATUS_SERVER = 'STATUS_SERVER',
    STREAM = 'STREAM',
    STREAM_BW = 'STREAM_BW',
    TRANSPORT_LAUNCHED = 'TRANSPORT_LAUNCHED',
    WARN = 'WARN',
    UNKNOWN = 'UNKNOWN'
}

export interface ExtendCircuitOptions {
    circuitId?: number;
    serverSpecs?: string[];
    purpose?: Purpose;
    awaitBuild?: boolean;
}

export enum Flag {
    Authority = 'Authority',
    BadExit = 'BadExit',
    BadDirectory = 'BadDirectory',
    Exit = 'Exit',
    Fast = 'Fast',
    Guard = 'Guard',
    HSDir = 'HSDir',
    Named = 'Named',
    NoEdConsensus = 'NoEdConsensus',
    Running = 'Running',
    Stable = 'Stable',
    StaleDesc = 'StaleDesc',
    Unnamed = 'Unnamed',
    Valid = 'Valid',
    V2Dir = 'V2Dir',
    V3Dir = 'V3Dir'
}

export interface RelayInfo {
    fingerprint: string;
    nickname: string;
    ip: string;
    orPort: number;
    flags: Flag[];
    bandwidth: number;
    published?: Date;
    dirPort?: number;
    country?: string;
}

export interface ControlMessage {
    code: string;
    divider: string;
    content: string;
    raw: string;
    arrivedAt?: number;
}

export interface Event {
    type: EventType;
    data?: string;
}

export interface StreamEvent extends Event {
    type: EventType.STREAM;
    streamId: number;
    status: string;
    circId: string;
    target: string;
    sourceAddr: string | null;
    purpose: string | null;
    reason: string | null;
    remoteReason: string | null;
    source: string | null;
}

export interface AddrMapEvent {
    type: EventType.ADDRMAP;
    address: string;
    mappedAddress: string;
    expires?: Date;
}
