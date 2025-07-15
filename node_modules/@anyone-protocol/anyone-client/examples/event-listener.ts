import { Control } from '../src/control';
import { Process } from "../src/process";
import { Socks } from "../src/socks";
import { EventType } from '../src/models';

async function main() {
    console.log('Starting Anon...');
    // create Anon instance
    const anon = new Process({ displayLog: true, socksPort: 9050, controlPort: 9051 });
    const socks = new Socks(anon);
    try {
        // start Anon
        await anon.start();
        console.log('Anon started');
        // connect to Control
        const control = new Control();

        // authenticate
        await control.authenticate();

        // add event listener
        const eventListener = (event: any) => {
            console.log('Event received:', event);
        };

        control.addEventListener(eventListener, EventType.INFO, EventType.NOTICE, EventType.WARN);

        // sleep for a while to allow events to be received
        await new Promise(resolve => setTimeout(resolve, 1000));

        const randomlyCreatedCircuitId = await control.extendCircuit();
        console.log('Randomly created circuit id:', randomlyCreatedCircuitId);

        const response = await socks.get('http://ip-api.com/json');
        console.log('Response:', response.data);

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

