import { Control } from "../src/control";
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

        // get router status (all relays in consensus)
        const relays = await control.getRelays();

        console.log('Available relays:', relays.length);

        // filter relays based on a specific criterion
        const filtered = await control.filterRelaysByCountries(relays, 'us', 'ca', 'gb');
        
        console.log('Filtered relays:', filtered.length);

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