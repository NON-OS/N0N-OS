import { Flag, RelayInfo } from "../src/models";
import { Control } from "../src/control";

interface PathState {
    desiredLength: number;
    desiredExitCountries: string[];
    excludedRelays: string[];
    excludedCountries: string[];
    path: string[];
    relays: RelayInfo[];
    exit?: RelayInfo;
}

export async function selectPath(control: Control, hopCount: number, ...exitCountries: string[]): Promise<string[]> {
    const relays = await control.getRelays();

    const state: PathState = {
        desiredLength: hopCount,
        desiredExitCountries: exitCountries,
        excludedRelays: [],
        excludedCountries: [],
        relays: relays,
        path: [],
    }

    await pickExit(control, state);

    await populatePath(control, state)

    return state.path!;
}

async function populatePath(control: Control, state: PathState) {
    let r = 0;

    while (r == 0) {
        r = await extendPath(control, state);
    }
}

async function extendPath(control: Control, state: PathState): Promise<number> {
    if (state.path.length >= state.desiredLength) {
        return 1;
    }

    let relay: RelayInfo | null = null;

    if (state.path.length === 0) {
        relay = await chooseEntry(control, state);
    } else if (state.path.length === state.desiredLength - 1) {
        relay = await chooseExit(control, state);
    } else {
        relay = await chooseMiddle(control, state);
    }

    state.path.push(relay!.fingerprint);
    state.excludedRelays.push(relay!.fingerprint);
    state.excludedCountries.push(relay!.country!);

    return 0;
}

async function chooseMiddle(control: Control, state: PathState): Promise<RelayInfo> {
    let middleRelays = state.relays.filter(relay =>
        relay.flags.includes(Flag.Stable) &&
        relay.flags.includes(Flag.Running) &&
        !relay.flags.includes(Flag.Exit) && // not Exit allowed for middle
        !relay.flags.includes(Flag.Guard) // not Guard allowed for middle
    );

    await control.populateCountries(middleRelays);

    middleRelays = middleRelays.filter(relay =>
        !state.excludedRelays.includes(relay.fingerprint) &&
        !state.excludedCountries.includes(relay.country!)
    );

    return chooseRandom(middleRelays);
}

async function chooseExit(control: Control, state: PathState): Promise<RelayInfo> {
    if (state.exit) {
        return state.exit;
    }

    let exits = state.relays.filter(relay =>
        relay.flags.includes(Flag.Exit) && !relay.flags.includes(Flag.BadExit)
    );

    await control.populateCountries(exits);

    exits = exits.filter(relay =>
        !state.excludedRelays.includes(relay.fingerprint) &&
        !state.excludedCountries.includes(relay.country!)
    );

    if (state.desiredExitCountries.length > 0) {
        exits = exits.filter(relay =>
            state.desiredExitCountries.includes(relay.country!)
        );
    }

    return chooseRandom(exits);
}

async function chooseEntry(control: Control, state: PathState): Promise<RelayInfo> {
    let entries = state.relays.filter(relay =>
        relay.flags.includes(Flag.Guard) &&
        relay.flags.includes(Flag.Stable) &&
        relay.flags.includes(Flag.Running) &&
        relay.flags.includes(Flag.Fast) &&
        !relay.flags.includes(Flag.Exit) // not Exit allowed for entry
    );

    await control.populateCountries(entries);

    if (state.desiredExitCountries.length > 0) {
        entries = entries.filter(relay =>
            relay.country && !state.desiredExitCountries.includes(relay.country)
        );
    }

    entries = entries.filter(relay =>
        !state.excludedRelays.includes(relay.fingerprint) &&
        !state.excludedCountries.includes(relay.country!)
    );

    return chooseRandom(entries);
}

async function pickExit(control: Control, state: PathState) {
    state.exit = await chooseExit(control, state);

    state.excludedRelays.push(state.exit!.fingerprint);
    state.excludedCountries.push(state.exit!.country!);
}

function chooseRandom(relays: RelayInfo[]): RelayInfo {
    const totalBandwidth = relays.reduce((sum, relay) => sum + relay.bandwidth, 0);

    const weights = relays.map(relay => relay.bandwidth / totalBandwidth);
    const randomValue = Math.random();

    let cumulative = 0;
    for (let i = 0; i < relays.length; i++) {
        cumulative += weights[i];
        if (randomValue < cumulative) {
            return relays[i];
        }
    }
    return relays[relays.length - 1]; // Fallback
}