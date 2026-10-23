// Opt-in local relay. The engine and all command execution stay in WASM.
export function connectControl(host) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(location.hostname)) return;
  const key = 'bern:control-token';
  const supplied = new URLSearchParams(location.hash.slice(1)).get('bern-bridge');
  let token = supplied;
  try {
    if (supplied) sessionStorage.setItem(key, supplied);
    else token = sessionStorage.getItem(key);
  } catch { /* A fragment can still enable a session without storage. */ }
  if (!token) return;
  let stopped = false, page = null;
  const state = value => { document.documentElement.dataset.bernControl = value; };
  async function post(action, data, keepalive = false) {
    const response = await fetch(`/__bern_bridge/${action}`, {
      method: 'POST', headers: {'Content-Type':'application/json', 'Authorization':`Bearer ${token}`},
      body: JSON.stringify(data), cache:'no-store', keepalive,
    });
    if (!response.ok) {
      const error = new Error(`Browser command bridge: HTTP ${response.status}`);
      error.status = response.status; throw error;
    }
    return response.json();
  }
  async function execute(task, owner) {
    let message;
    try { message = await host.control({entry:task.entry, message:task.message}); }
    catch (error) {
      message = {type:'error', request_id:task.message.request_id,
        error:{code:'browser_command_failed',message:String(error.message)}};
    }
    // A disconnected/expired request is never queued or retried.
    try { await post('reply', {page:owner, id:task.id, message}); } catch { /* caller has disconnected */ }
  }
  async function run() {
    let delay = 250;
    while (!stopped) {
      try {
        if (!page) {
          state('connecting');
          page = (await post('register', {engine:host.engineID})).page;
          document.documentElement.dataset.bernEngine = host.engineID;
          state('connected');
        }
        const owner = page;
        const response = await post('next', {page:owner});
        delay = 250;
        for (const task of response.tasks) if (!stopped) void execute(task, owner);
      } catch (error) {
        page = null; state('disconnected');
        if ([401,403].includes(error.status)) { stopped = true; break; }
        await new Promise(resolve=>setTimeout(resolve, delay));
        delay = Math.min(delay*2, 10000);
      }
    }
  }
  addEventListener('pagehide', () => {
    stopped = true; state('disconnected');
    if (page) void post('disconnect', {page}, true).catch(()=>{});
  }, {once:true});
  void run();
}
