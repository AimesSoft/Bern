// One document per persistent context. The parent only routes requests.
(() => {
  const contexts = new Map();
  const child = `<!doctype html><meta charset="utf-8"><script>
  addEventListener('message', async event => {
    if (event.source !== parent) return;
    const {id, source} = event.data;
    try {
      const value = await (0, eval)(source);
      parent.postMessage(JSON.stringify({id, value: value === undefined ? null : value}), '*');
    } catch (error) {
      parent.postMessage(JSON.stringify({id, error: String(error)}), '*');
    }
  });<\/script>`;
  addEventListener('message', event => {
    if ([...contexts.values()].some(c => c.frame.contentWindow === event.source)) bernScriptComplete(event.data);
  });
  globalThis.bernScriptDispatch = requests => {
    for (const request of requests) {
      if (request.source === null) {
        contexts.get(request.context)?.frame.remove(); contexts.delete(request.context); continue;
      }
      let context = contexts.get(request.context);
      if (!context) {
        const frame = document.createElement('iframe');
        frame.hidden = true; frame.sandbox = 'allow-scripts';
        context = {frame, ready: false, queue: []};
        frame.onload = () => { context.ready = true; for (const r of context.queue) frame.contentWindow.postMessage(r, '*'); context.queue = []; };
        frame.srcdoc = child; document.body.append(frame); contexts.set(request.context, context);
      }
      if (context.ready) context.frame.contentWindow.postMessage(request, '*');
      else context.queue.push(request);
    }
  };
})();
