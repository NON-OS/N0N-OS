import { Control } from '../src/control';
import { AddrMapEvent, EventType } from '../src/models';
import { Process } from "../src/process";

async function main() {
    console.log('Starting Anon...');
    // create Anon instance
    const anon = new Process({ displayLog: true, socksPort: 9050, controlPort: 9051 });
    try {
        // start Anon
        await anon.start();
        console.log('Anon started');
        // connect to Control
        const control = new Control();

        // authenticate
        await control.authenticate();

        // add event listener
        const eventListener = (event: AddrMapEvent) => {
            console.log('Event received:', event);
        };

        control.addEventListener(eventListener, EventType.ADDRMAP);

        // sleep for a while to allow events to be received
        await new Promise(resolve => setTimeout(resolve, 1000));

        await control.resolve('www.google.com');

        // sleep for a while to allow events to be received
        await new Promise(resolve => setTimeout(resolve, 1000));

        // close connection
        control.end();
    } catch (error) {
        console.error('Error:', error);
    } finally {
        // stop Anon
        await anon.stop();
        console.log('Anon stopped');
    }
}

main();

