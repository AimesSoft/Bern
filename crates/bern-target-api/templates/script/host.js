
function startScriptHost(host) {
  const contexts = new Map();
  const source = `onmessage = async ({data: {id, source}}) => {
    try { const value = await (0, eval)(source); postMessage(JSON.stringify({id, value: value === undefined ? null : value})); }
    catch (error) { postMessage(JSON.stringify({id, error: String(error)})); }
  };`;
  const url = URL.createObjectURL(new Blob([source], {type: "text/javascript"}));
  host.exports.bern_script_start();
  const complete = json => host.withBytes(new TextEncoder().encode(json), (ptr, len) => host.exports.bern_script_complete(ptr, len));
  const timer = setInterval(() => {
    const batch = new TextDecoder().decode(host.buffer("bern_script_poll", performance.now()));
    if (!batch) return;
    for (const request of JSON.parse(batch)) {
      if (request.source === null) { contexts.get(request.context)?.terminate(); contexts.delete(request.context); continue; }
      try {
        let worker = contexts.get(request.context);
        if (!worker) {
          worker = new Worker(url); contexts.set(request.context, worker);
          worker.onmessage = ({data}) => complete(data);
          worker.onerror = () => stop();
        }
        worker.postMessage(request);
      } catch (error) { complete(JSON.stringify({id: request.id, error: String(error)})); }
    }
  }, 16);
  const stop = () => {
    clearInterval(timer); for (const worker of contexts.values()) worker.terminate();
    contexts.clear(); URL.revokeObjectURL(url); host.exports.bern_script_stop();
  };
  addEventListener("pagehide", event => { if (!event.persisted) stop(); });
  return stop;
}
