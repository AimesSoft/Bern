use bern_script::{
    CliCall, CliDispatcher, CliError, CliInput, CliPolicy, CliTargetKind, Context, Evaluation,
    Value, split_line,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn result(eval: Evaluation) -> std::result::Result<Value, bern_script::Error> {
    let start = Instant::now();
    loop {
        if let Some(result) = eval.try_result() {
            return result;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "script never completed"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn cli_policy_and_line_splitting_rules() {
    assert_eq!(
        split_line(r#"media seek --player-id "main video" --note 'a "b"' --empty "" --esc a\ b"#)
            .unwrap(),
        vec![
            "media",
            "seek",
            "--player-id",
            "main video",
            "--note",
            r#"a "b""#,
            "--empty",
            "",
            "--esc",
            "a b"
        ]
    );
    assert!(split_line("media seek --title 'unterminated").is_err());

    let default_policy = CliPolicy::default();
    assert!(default_policy.allows_target("media.seek", CliTargetKind::MediaCommand));
    assert!(default_policy.allows_target("ui.activate", CliTargetKind::SessionCommand));
    assert!(default_policy.allows_target("state", CliTargetKind::State));

    let blacklist = CliPolicy::allow_all().deny("account.*").deny("trace.*");
    assert!(blacklist.allows_target("media.seek", CliTargetKind::MediaCommand));
    assert!(!blacklist.allows_target("account.logout", CliTargetKind::EngineCommand));
    assert!(!blacklist.allows_target("trace.start", CliTargetKind::SessionCommand));

    let whitelist = CliPolicy::allow_only(["media.*", "@queries", "state"]).deny("media.bind");
    assert!(whitelist.allows_target("media.seek", CliTargetKind::MediaCommand));
    assert!(!whitelist.allows_target("media.bind", CliTargetKind::MediaCommand));
    assert!(whitelist.allows_target("library.search", CliTargetKind::EngineQuery));
    assert!(!whitelist.allows_target("library.delete", CliTargetKind::EngineCommand));
    assert!(whitelist.allows_target("state", CliTargetKind::State));
    assert!(!whitelist.allows_target("ui.activate", CliTargetKind::SessionCommand));

    let state_policy = CliPolicy::allow_all()
        .allow_state_only(["player.*", "volume"])
        .deny_state("player.secret");
    assert!(state_policy.allows_state_field("player.position"));
    assert!(state_policy.allows_state_field("volume"));
    assert!(!state_policy.allows_state_field("player.secret"));
    assert!(!state_policy.allows_state_field("account.token"));
}

#[cfg(feature = "boa")]
static BOA_CLI_LOCK: Mutex<()> = Mutex::new(());

#[test]
#[cfg(feature = "boa")]
fn persistent_contexts_values_promises_errors_and_limits() {
    let first = Context::new().unwrap();
    let second = Context::new().unwrap();
    assert_eq!(result(first.eval("var count = 1; ++count")).unwrap(), 2);
    assert_eq!(result(first.eval("++count")).unwrap(), 3);
    assert_eq!(result(second.eval("typeof count")).unwrap(), "undefined");
    assert_eq!(
        result(first.eval("Promise.resolve({items: [1, true, null]})")).unwrap(),
        serde_json::json!({"items": [1, true, null]})
    );
    assert!(
        result(first.eval("throw new Error('broken')"))
            .unwrap_err()
            .0
            .contains("broken")
    );
    assert!(
        result(first.eval("Promise.reject('rejected')"))
            .unwrap_err()
            .0
            .contains("rejected")
    );
    assert!(result(first.eval("while(true) {}")).is_err());
    assert_eq!(result(first.eval("6 * 7")).unwrap(), 42);
}

#[cfg(feature = "boa")]
struct MockCli;

#[cfg(feature = "boa")]
impl CliDispatcher for MockCli {
    fn dispatch(
        &self,
        input: CliInput,
        policy: Arc<CliPolicy>,
        done: Box<dyn FnOnce(std::result::Result<Value, CliError>) + Send>,
    ) {
        let (target, kind, params, session) = match input {
            CliInput::Line { line } => {
                let tokens = match split_line(&line) {
                    Ok(tokens) => tokens,
                    Err(error) => {
                        done(Err(error));
                        return;
                    }
                };
                let target = tokens.first().cloned().unwrap_or_default();
                let kind = if target == "state" {
                    CliTargetKind::State
                } else {
                    CliTargetKind::EngineCommand
                };
                let mut map = serde_json::Map::new();
                map.insert("tokens".into(), serde_json::json!(tokens));
                (target, kind, map, None)
            }
            CliInput::Args { args } => {
                let target = args.first().cloned().unwrap_or_default();
                let mut map = serde_json::Map::new();
                map.insert("args".into(), serde_json::json!(args));
                (target, CliTargetKind::EngineCommand, map, None)
            }
            CliInput::Command {
                target,
                params,
                session,
            } => (target, CliTargetKind::EngineCommand, params, session),
        };
        if target == "hang" {
            // Deliberately never answer so timeout handling can be tested.
            std::mem::forget(done);
            return;
        }
        let call = CliCall {
            target: target.clone(),
            kind,
            params: params.clone(),
            session: session.clone(),
            label: policy.label().map(str::to_owned),
        };
        if let Err(error) = policy.check(&call) {
            done(Err(error));
            return;
        }
        let payload = serde_json::json!({
            "target": target,
            "params": params,
            "session": session,
            "label": policy.label(),
        });
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            done(Ok(payload));
        });
    }
}

#[test]
#[cfg(feature = "boa")]
fn boa_bern_cli_supports_forms_policies_dynamic_updates_async_listeners_and_timeouts() {
    let _guard = BOA_CLI_LOCK.lock().unwrap();
    let _reg = bern_script::host::install_cli(Arc::new(MockCli));

    // Default policy allows everything.
    let open = Context::new().unwrap();
    let answer = result(open.eval(
        r#"(async () => {
            const a = await bern.cli("state");
            const b = await bern.cli(["echo.run", "--text", "hello world"]);
            const c = await bern.cli("echo.obj", { count: 3 }, { session: "s1" });
            return { a: a.target, b: b.params.args, c: c.params.count, s: c.session };
        })()"#,
    ))
    .unwrap();
    assert_eq!(
        answer,
        serde_json::json!({
            "a": "state",
            "b": ["echo.run", "--text", "hello world"],
            "c": 3,
            "s": "s1"
        })
    );

    // Allowlist + denylist + dynamic filter + runtime policy update.
    let scoped = Context::with_policy(
        CliPolicy::allow_only(["echo.*", "state"])
            .deny("echo.secret")
            .with_label("plugin.alpha")
            .with_filter(|call| {
                if call.params.get("blocked").and_then(Value::as_bool) == Some(true) {
                    Err("blocked by custom filter".into())
                } else {
                    Ok(())
                }
            }),
    )
    .unwrap();
    let checked = result(scoped.eval(
        r#"(async () => {
            const ok = await bern.cli("echo.ok", { v: 1 });
            let deniedCode = null;
            try { await bern.cli("echo.secret", {}); } catch (e) { deniedCode = e instanceof Error ? e.code : "not-error"; }
            let unlistedCode = null;
            try { await bern.cli("other.cmd", {}); } catch (e) { unlistedCode = e.code; }
            let filterMsg = null;
            try { await bern.cli("echo.ok", { blocked: true }); } catch (e) { filterMsg = e.message; }
            return { label: ok.label, deniedCode, unlistedCode, filterMsg };
        })()"#,
    ))
    .unwrap();
    assert_eq!(
        checked,
        serde_json::json!({
            "label": "plugin.alpha",
            "deniedCode": "permission_denied",
            "unlistedCode": "permission_denied",
            "filterMsg": "blocked by custom filter"
        })
    );

    // Updating policy at runtime takes effect immediately on the same context.
    scoped.set_policy(CliPolicy::allow_all().with_label("plugin.alpha"));
    let after_update =
        result(scoped.eval(r#"(async () => (await bern.cli("echo.secret", {})).target)()"#))
            .unwrap();
    assert_eq!(after_update, "echo.secret");

    // Background async listener triggered inside a sync eval still completes.
    assert_eq!(
        result(open.eval(
            r#"var bgDone = null;
            (async () => {
                const r = await bern.cli("echo.bg", { n: 9 });
                bgDone = r.params.n;
            })();
            "started""#,
        ))
        .unwrap(),
        "started"
    );
    let start = Instant::now();
    loop {
        if result(open.eval("bgDone")).unwrap() == 9 {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "background async CLI listener never completed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    // Timeout produces `cli_timeout`.
    let fast_timeout = Context::with_policy(CliPolicy::allow_all().with_timeout_ms(50)).unwrap();
    let timed = result(fast_timeout.eval(
        r#"(async () => {
            try { await bern.cli("hang", {}); return "ok"; }
            catch (e) { return e.code; }
        })()"#,
    ))
    .unwrap();
    assert_eq!(timed, "cli_timeout");
}

#[cfg(not(feature = "boa"))]
static HOST_LOCK: Mutex<()> = Mutex::new(());

#[test]
#[cfg(not(feature = "boa"))]
fn host_routing_disposal_and_shutdown() {
    use bern_script::host;
    let _guard = HOST_LOCK.lock().unwrap();
    host::stop();
    assert!(Context::new().is_err());
    host::start();
    let context = Context::new().unwrap();
    let pending = context.eval("6 * 7");
    let batch: Value = serde_json::from_str(&host::poll(0.0)).unwrap();
    host::complete(&serde_json::json!({"id": batch[0]["id"], "value": 42}).to_string());
    assert_eq!(result(pending).unwrap(), 42);
    let pending = context.eval("Promise.resolve(1)");
    drop(context);
    assert!(result(pending).unwrap_err().0.contains("disposed"));
    let batch: Value = serde_json::from_str(&host::poll(0.0)).unwrap();
    assert!(batch[0]["source"].is_null());
    let context = Context::new().unwrap();
    let pending = context.eval("1");
    host::poll(1.0);
    host::poll(30_001.0);
    assert!(result(pending).unwrap_err().0.contains("timed out"));
    let pending = context.eval("2");
    host::stop();
    assert!(result(pending).unwrap_err().0.contains("stopped"));
    assert!(Context::new().is_err());
}

#[cfg(not(feature = "boa"))]
struct HostMockCli;

#[cfg(not(feature = "boa"))]
impl CliDispatcher for HostMockCli {
    fn dispatch(
        &self,
        input: CliInput,
        policy: Arc<CliPolicy>,
        done: Box<dyn FnOnce(std::result::Result<Value, CliError>) + Send>,
    ) {
        let CliInput::Command { target, params, .. } = input else {
            done(Err(CliError::invalid_params("expected command")));
            return;
        };
        if target == "hang" {
            std::mem::forget(done);
            return;
        }
        let call = CliCall {
            target: target.clone(),
            kind: CliTargetKind::EngineCommand,
            params: params.clone(),
            session: None,
            label: policy.label().map(str::to_owned),
        };
        if let Err(error) = policy.check(&call) {
            done(Err(error));
            return;
        }
        done(Ok(serde_json::json!({ "ok": target, "params": params })));
    }
}

#[test]
#[cfg(not(feature = "boa"))]
fn host_cli_routing_policy_and_timeout() {
    use bern_script::host;
    let _guard = HOST_LOCK.lock().unwrap();
    let _reg = host::install_cli(Arc::new(HostMockCli));
    host::stop();
    host::start();

    let context =
        Context::with_policy(CliPolicy::allow_only(["demo.ping"]).with_timeout_ms(100)).unwrap();
    let _eval = context.eval("1");
    let batch: Value = serde_json::from_str(&host::poll(0.0)).unwrap();
    let ctx_id = batch[0]["context"].as_str().unwrap().to_owned();

    // Allowed command replies cleanly.
    host::cli(
        &serde_json::json!({
            "context": ctx_id,
            "call_id": "c1",
            "input": { "kind": "command", "target": "demo.ping", "params": { "x": 1 } }
        })
        .to_string(),
    );
    let replies: Value = serde_json::from_str(&host::poll(10.0)).unwrap();
    assert_eq!(replies[0]["cli_reply"]["call_id"], "c1");
    assert_eq!(replies[0]["cli_reply"]["value"]["ok"], "demo.ping");

    // Denied command returns permission_denied.
    host::cli(
        &serde_json::json!({
            "context": ctx_id,
            "call_id": "c2",
            "input": { "kind": "command", "target": "demo.drop", "params": {} }
        })
        .to_string(),
    );
    let replies: Value = serde_json::from_str(&host::poll(20.0)).unwrap();
    assert_eq!(
        replies[0]["cli_reply"]["error"]["code"],
        "permission_denied"
    );

    // Hanging command times out in poll().
    host::cli(
        &serde_json::json!({
            "context": ctx_id,
            "call_id": "c3",
            "input": { "kind": "command", "target": "hang", "params": {} }
        })
        .to_string(),
    );
    assert!(host::poll(50.0).is_empty());
    let replies: Value = serde_json::from_str(&host::poll(151.0)).unwrap();
    assert_eq!(replies[0]["cli_reply"]["call_id"], "c3");
    assert_eq!(replies[0]["cli_reply"]["error"]["code"], "cli_timeout");

    host::stop();
}
