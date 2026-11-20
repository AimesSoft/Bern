// One document per persistent context. The parent only routes requests.
(() => {
  const contexts = new Map();
  const child = `<!doctype html><meta charset="utf-8"><script>
  const __cliPending = new Map();
  let __cliSeq = 0;
  function __cliError(code, message) {
    const error = new Error(message);
    error.code = code;
    return error;
  }
  function __parseCliArgs(first, second, third) {
    if (typeof first === 'string') {
      if (second === undefined || second === null) return { kind: 'line', line: first };
      if (typeof second !== 'object' || Array.isArray(second)) {
        throw __cliError('invalid_params', 'second argument to bern.cli must be a parameter object');
      }
      let session = null;
      if (third !== undefined && third !== null) {
        if (typeof third === 'string') session = third || null;
        else if (typeof third === 'object' && !Array.isArray(third)) {
          if (third.session !== undefined && third.session !== null) {
            if (typeof third.session !== 'string') throw __cliError('invalid_params', 'options.session must be a string');
            session = third.session || null;
          }
        } else {
          throw __cliError('invalid_params', 'third argument to bern.cli must be an options object or session id');
        }
      }
      return { kind: 'command', target: first, params: JSON.parse(JSON.stringify(second)), session };
    }
    if (Array.isArray(first)) {
      if (second !== undefined && second !== null) {
        throw __cliError('invalid_params', 'array form of bern.cli does not take a second argument');
      }
      const args = first.map(item => {
        if (typeof item === 'string') return item;
        if (typeof item === 'number' || typeof item === 'boolean') return String(item);
        throw __cliError('invalid_params', 'bern.cli argument array elements must be strings, numbers or booleans');
      });
      return { kind: 'args', args };
    }
    throw __cliError('invalid_params', 'bern.cli expects a command string or an argument array');
  }
  globalThis.bern = Object.freeze({
    cli(first, second, third) {
      let input;
      try { input = __parseCliArgs(first, second, third); }
      catch (error) { return Promise.reject(error); }
      const callId = String(++__cliSeq);
      return new Promise((resolve, reject) => {
        __cliPending.set(callId, { resolve, reject });
        parent.postMessage({ __bernCli: true, callId, input }, '*');
      });
    }
  });
  addEventListener('message', async event => {
    if (event.source !== parent) return;
    const data = event.data;
    if (data && data.__bernCliReply) {
      const entry = __cliPending.get(data.callId);
      if (!entry) return;
      __cliPending.delete(data.callId);
      if (data.error) entry.reject(__cliError(data.error.code || 'cli_failed', data.error.message || 'CLI call failed'));
      else entry.resolve(data.value === undefined ? null : data.value);
      return;
    }
    const {id, source} = data;
    try {
      const value = await (0, eval)(source);
      parent.postMessage(JSON.stringify({id, value: value === undefined ? null : value}), '*');
    } catch (error) {
      parent.postMessage(JSON.stringify({id, error: String(error)}), '*');
    }
  });<\/script>`;
  addEventListener('message', event => {
    for (const [contextId, context] of contexts.entries()) {
      if (context.frame.contentWindow === event.source) {
        if (event.data && typeof event.data === 'object' && event.data.__bernCli) {
          if (typeof globalThis.bernScriptCli === 'function') {
            globalThis.bernScriptCli(JSON.stringify({ context: contextId, call_id: event.data.callId, input: event.data.input }));
          } else {
            context.frame.contentWindow.postMessage({
              __bernCliReply: true,
              callId: event.data.callId,
              error: { code: 'cli_unavailable', message: 'no application CLI host is attached to bern.cli' },
            }, '*');
          }
        } else {
          bernScriptComplete(event.data);
        }
        return;
      }
    }
  });
  globalThis.bernScriptDispatch = requests => {
    for (const request of requests) {
      if (request.cli_reply) {
        const reply = request.cli_reply;
        const target = contexts.get(reply.context);
        if (target) {
          const message = { __bernCliReply: true, callId: reply.call_id, value: reply.value, error: reply.error };
          if (target.ready) target.frame.contentWindow.postMessage(message, '*');
          else target.queue.push(message);
        }
        continue;
      }
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
