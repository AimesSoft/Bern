
function startScriptHost(host) {
  const contexts = new Map();
  const source = `
    const __cliPending = new Map();
    let __cliSeq = 0;
    function __cliError(code, message) {
      const error = new Error(message);
      error.code = code;
      return error;
    }
    function __parseCliArgs(first, second, third) {
      if (typeof first === "string") {
        if (second === undefined || second === null) return { kind: "line", line: first };
        if (typeof second !== "object" || Array.isArray(second)) {
          throw __cliError("invalid_params", "second argument to bern.cli must be a parameter object");
        }
        let session = null;
        if (third !== undefined && third !== null) {
          if (typeof third === "string") session = third || null;
          else if (typeof third === "object" && !Array.isArray(third)) {
            if (third.session !== undefined && third.session !== null) {
              if (typeof third.session !== "string") throw __cliError("invalid_params", "options.session must be a string");
              session = third.session || null;
            }
          } else {
            throw __cliError("invalid_params", "third argument to bern.cli must be an options object or session id");
          }
        }
        return { kind: "command", target: first, params: JSON.parse(JSON.stringify(second)), session };
      }
      if (Array.isArray(first)) {
        if (second !== undefined && second !== null) {
          throw __cliError("invalid_params", "array form of bern.cli does not take a second argument");
        }
        const args = first.map(item => {
          if (typeof item === "string") return item;
          if (typeof item === "number" || typeof item === "boolean") return String(item);
          throw __cliError("invalid_params", "bern.cli argument array elements must be strings, numbers or booleans");
        });
        return { kind: "args", args };
      }
      throw __cliError("invalid_params", "bern.cli expects a command string or an argument array");
    }
    globalThis.bern = Object.freeze({
      cli(first, second, third) {
        let input;
        try { input = __parseCliArgs(first, second, third); }
        catch (error) { return Promise.reject(error); }
        const callId = String(++__cliSeq);
        return new Promise((resolve, reject) => {
          __cliPending.set(callId, { resolve, reject });
          postMessage({ __bernCli: true, callId, input });
        });
      }
    });
    onmessage = async ({ data }) => {
      if (data && data.__bernCliReply) {
        const entry = __cliPending.get(data.callId);
        if (!entry) return;
        __cliPending.delete(data.callId);
        if (data.error) entry.reject(__cliError(data.error.code || "cli_failed", data.error.message || "CLI call failed"));
        else entry.resolve(data.value === undefined ? null : data.value);
        return;
      }
      const { id, source } = data;
      try {
        const value = await (0, eval)(source);
        postMessage(JSON.stringify({ id, value: value === undefined ? null : value }));
      } catch (error) {
        postMessage(JSON.stringify({ id, error: String(error) }));
      }
    };`;
  const url = URL.createObjectURL(new Blob([source], {type: "text/javascript"}));
  host.exports.bern_script_start();
  const complete = json => host.withBytes(new TextEncoder().encode(json), (ptr, len) => host.exports.bern_script_complete(ptr, len));
  const dispatchCli = json => host.withBytes(new TextEncoder().encode(json), (ptr, len) => host.exports.bern_script_cli(ptr, len));
  const timer = setInterval(() => {
    const batch = new TextDecoder().decode(host.buffer("bern_script_poll", performance.now()));
    if (!batch) return;
    for (const request of JSON.parse(batch)) {
      if (request.cli_reply) {
        const reply = request.cli_reply;
        contexts.get(reply.context)?.postMessage({
          __bernCliReply: true,
          callId: reply.call_id,
          value: reply.value,
          error: reply.error,
        });
        continue;
      }
      if (request.source === null) { contexts.get(request.context)?.terminate(); contexts.delete(request.context); continue; }
      try {
        let worker = contexts.get(request.context);
        if (!worker) {
          const contextId = request.context;
          worker = new Worker(url); contexts.set(contextId, worker);
          worker.onmessage = ({data}) => {
            if (data && typeof data === "object" && data.__bernCli) {
              if (host.exports.bern_script_cli) {
                dispatchCli(JSON.stringify({ context: contextId, call_id: data.callId, input: data.input }));
              } else {
                worker.postMessage({
                  __bernCliReply: true,
                  callId: data.callId,
                  error: { code: "cli_unavailable", message: "no application CLI host is attached to bern.cli" },
                });
              }
              return;
            }
            complete(data);
          };
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
