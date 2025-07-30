import { Process } from "../../src/process";
import { Socks } from "../../src/socks";

async function main() {
    try {
        // connect to Control
        const anon = new Process({ displayLog: true, socksPort: 9050, controlPort: 9051 });
        const socks = new Socks(anon);
        await anon.start();
        
        // make a request
        const response = await socks.get('https://api.ipify.org?format=json');
        console.log('Response:', response.data);

        // stop Anon
        await anon.stop();

    } catch (error) {
        console.error('Error:', error);
    }
}

main();