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
        const controlClient = new Control();

        // authenticate
        await controlClient.authenticate();

        // get circuit status before
        const circuits = await controlClient.circuitStatus();

        console.log('Get info about relay from circuit:', circuits);

        // create new circuit with random servers
        const randomlyCreatedCircuitId = await controlClient.extendCircuit(
            {
                serverSpecs: [
                    circuits[0].relays[0].fingerprint, 
                    circuits[1].relays[0].fingerprint, 
                    circuits[2].relays[0].fingerprint
                ],
                awaitBuild: true,
            }
        );
        console.log('Randomly created circuit id:', randomlyCreatedCircuitId);

        // close connection
        controlClient.end();
    } catch (error) {
        console.error('Error:', error);
    } finally {
        // stop Anon
        await anon.stop();
        console.log('Anon stopped');
    }
}

main();
