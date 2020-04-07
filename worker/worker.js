const { handle_request_rs } = wasm_bindgen;

addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
})

/**
 * Fetch and log a request
 * @param {Request} request
 */
async function handleRequest(request) {
    await wasm_bindgen(wasm);
    return await handle_request_rs(request);
}
