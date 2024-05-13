var module = import('../pkg/diffenator3.js');
async function init() {
    let wasm = await module;
    // console.log("Got wasm module", wasm);
    self.onmessage = async (event) => {
        // console.log("Worker received message");
        // console.log(event);
        const { beforeFont, afterFont } = event.data;
        wasm.progressive_diff(beforeFont, afterFont, (progress) => {
            self.postMessage(progress);
        });

    }
    return self;
}

init();
