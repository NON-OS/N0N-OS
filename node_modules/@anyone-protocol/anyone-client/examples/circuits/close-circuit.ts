import { Control } from '../../src/control';
import { Process } from "../../src/process";

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

        // get circuit status before
        const circuits = await control.circuitStatus();

        // close circuit by id
        await control.closeCircuit(circuits[0].circuitId);
        console.log('Closed circuit id:', circuits[0].circuitId);

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
