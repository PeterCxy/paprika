addEventListener('fetch', event => {
  event.respondWith(handleRequest(event, event.request))
})

/**
 * Fetch and log a request
 * @param {Request} request
 */
async function handleRequest(ev, request) {
  const rust = await import("./pkg/index");
  return await rust.handle_request_rs(ev, request);
}
