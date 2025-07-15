import { Control } from '../src/control';
import { StreamEvent, EventType } from '../src/models';
import { Process } from "../src/process";
import { Socks } from "../src/socks";

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

        await control.disableStreamAttachment();
        console.log('Stream attachment disabled');

        const circId = await control.extendCircuit({awaitBuild: true});
        console.log('Created circuit with id:', circId);

        const circ = await control.getCircuit(circId);
        console.log('Circuit:', circ);

        // add event listener
        const eventListener = async (event: StreamEvent) => {
            console.log('Event received:', event);
            if (event.status === 'NEW') {
                await control.attachStream(event.streamId, circId);
                console.log('Stream attached to circuit:', circId);
            }
        };

        await control.addEventListener(eventListener, EventType.STREAM);

        // sleep for a while to allow events to be received
        await new Promise(resolve => setTimeout(resolve, 1000));

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

