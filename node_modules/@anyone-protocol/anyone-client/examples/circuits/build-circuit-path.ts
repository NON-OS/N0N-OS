import { Control } from "../../src/control";
import { Process } from "../../src/process";
import { selectPath } from "../path-selection";

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

        const path = await selectPath(control, 2, 'ca', 'us', 'gb');

        console.log('Selected path:', path);

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