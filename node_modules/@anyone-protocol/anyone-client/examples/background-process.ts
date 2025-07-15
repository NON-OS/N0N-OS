import { Control } from '../src/control';
import { ExtendCircuitOptions, StreamEvent, EventType, Flag } from '../src/models';
import { Process } from "../src/process";
import { VPNConfig } from '../src/models';

const config: VPNConfig = {
    routings: [
        { targetAddress: 'ip-api.com', exitCountries: ['de'] },
        { targetAddress: 'ipinfo.io', exitCountries: ['nl'] },
    ]
};

class AnonRunner {
    private control!: Control;
    private anon: Process;
    private shuttingDown = false;
    private routingMap: Record<string, number> = {};

    constructor() {
        this.anon = new Process({ displayLog: true, socksPort: 9050, controlPort: 9051 });
        process.on('SIGINT', this.shutdown.bind(this));
        process.on('SIGTERM', this.shutdown.bind(this));
    }

    async run() {
        try {
            await this.anon.start();
            this.control = new Control();
            console.log('Anon started');
            console.log('Starting Anon SDK process...');

            await this.control.authenticate();

            const relays = await this.control.getRelays();

            console.log('Relays:', relays.length);

            let exits = this.control.filterRelaysByFlags(relays, Flag.Exit, Flag.Stable, Flag.Running, Flag.Fast);
            exits = exits.filter((exit) => {
                return !exit.flags.includes(Flag.BadExit);
            });

            console.log('Exits all:', exits.length);

            // populate country field
            await this.control.populateCountries(exits)

            const guards = this.control.filterRelaysByFlags(relays, Flag.Guard, Flag.Stable, Flag.Running, Flag.Fast);
            console.log('Guards:', guards.length);

            for (const route of config.routings) {
                const exitsByCountry = exits.filter((exit) => {
                    return route.exitCountries
                        .map(country => country.toLowerCase())
                        .some((country) => exit.country === country);
                });

                console.log('Exits filtered:', exitsByCountry.length);

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

                const circuitId = await this.control.extendCircuit(options);
                this.routingMap[route.targetAddress] = circuitId;
            }

            await this.control.disableStreamAttachment();

            // add event listener
            const eventListener = async (event: StreamEvent) => {
                if (event.status === 'NEW') {
                    const targetAddress = event.target.split(':')[0];
                    const circuitId = this.routingMap[targetAddress];

                    if (circuitId && (event.circId === '0' || event.circId === undefined)) {
                        await this.control.attachStream(event.streamId, circuitId);
                    }
                }
            };

            await this.control.addEventListener(eventListener, EventType.STREAM);

            // keep process alive
            console.log('Anon VPN routing is active. Press Ctrl+C to quit.');
            await new Promise<void>((resolve) => {
                this._exitPromiseResolver = resolve;
            });
        } catch (error) {
            console.error('Error:', error);
            await this.shutdown();
        }
    }

    private _exitPromiseResolver?: () => void;

    async shutdown() {
        if (this.shuttingDown) return;
        this.shuttingDown = true;

        console.log('Shutting down Anon SDK process...');

        try {
            if (this.control) await this.control.end();
            console.log('Anon SDK process shut down');
        } catch (e) {
            console.warn('Control shutdown error:', e);
        }

        try {
            await this.anon.stop();
            console.log('Anon stopped');
        } catch (e) {
            console.warn('Anon shutdown error:', e);
        }

        if (this._exitPromiseResolver) this._exitPromiseResolver();
    }

}

async function main() {
    new AnonRunner().run();
    console.log('Anon SDK process started');

    process.on('uncaughtException', (err) => {
        console.error('Uncaught Exception:', err);
        process.exit(1);
    });

    process.on('unhandledRejection', (reason) => {
        console.error('Unhandled Rejection:', reason);
        process.exit(1);
    });
}

main();