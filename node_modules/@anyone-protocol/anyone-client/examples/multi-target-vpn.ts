import { Process } from "../src/process";
import { Socks } from "../src/socks";

import { Control } from '../src/control';
import { ExtendCircuitOptions, StreamEvent, EventType } from '../src/models';
import { VPNConfig, Flag } from '../src/models';

const config: VPNConfig = {
    routings: [
        { targetAddress: 'ip-api.com', exitCountries: ['de'] },
        { targetAddress: 'ipinfo.io', exitCountries: ['nl'] },
        { targetAddress: 'api.ipify.org', exitCountries: ['us'] },
    ]
};

async function main() {
    const anon = new Process({ displayLog: true, socksPort: 9050, controlPort: 9051 });

    try {
        await anon.start();
        const control = new Control();
        console.log('Anon started');
        console.log('Starting Anon SDK process...');

        await control.authenticate();

        const relays = await control.getRelays();

        console.log('Relays:', relays.length);

        const routingMap: Record<string, number> = {};

        let exits = control.filterRelaysByFlags(relays, Flag.Exit);
        exits = exits.filter((exit) => {
            return !exit.flags.includes(Flag.BadExit);
        });

        console.log('Exits all:', exits.length);

        // populate country field
        await control.populateCountries(exits)

        const guards = control.filterRelaysByFlags(relays, Flag.Guard);
        console.log('Guards:', guards.length);

        for (const route of config.routings) {
            const exitsByCountry = exits.filter((exit) => {
                return route.exitCountries.some((country) => exit.country === country);
            });

            const exit = exitsByCountry[Math.floor(Math.random() * exitsByCountry.length)];
            const guard = guards[Math.floor(Math.random() * guards.length)];
            const path = [guard.fingerprint, exit.fingerprint];
            console.log('Path:', path);

            const options: ExtendCircuitOptions = {
                circuitId: 0,
                serverSpecs: path,
                purpose: "general",
                awaitBuild: true
            };

            const circuitId = await control.extendCircuit(options);
            const circ = await control.getCircuit(circuitId);
            console.log('Circuit:', circ);
            routingMap[route.targetAddress] = circuitId;
        }

        await control.disableStreamAttachment();

        // add event listener
        const eventListener = async (event: StreamEvent) => {
            console.log('Event:', event);
            if (event.status === 'NEW') {
                const targetAddress = event.target.split(':')[0];
                const circuitId = routingMap[targetAddress];

                if (circuitId && (event.circId === '0' || event.circId === undefined)) {
                    await control.attachStream(event.streamId, circuitId);
                }
            }
        };

        await control.addEventListener(eventListener, EventType.STREAM);


        // Make a request through the established circuits
        const socks = new Socks(anon);
        const response = await socks.get('https://api.ipify.org?format=json');
        console.log('Response:', response.data);
    } catch (error) {
        console.error('Error:', error);
    } finally {
        await anon.stop();
        console.log('Anon stopped');
    }
}

main();
