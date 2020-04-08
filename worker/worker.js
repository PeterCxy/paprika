const { handle_request_rs } = wasm_bindgen;
var gen = false;

addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
})

/**
 * Fetch and log a request
 * @param {Request} request
 */
async function handleRequest(request) {
  if (!gen) {
    await wasm_bindgen(wasm);
    gen = true;
  }
  return await handle_request_rs(request);
}
