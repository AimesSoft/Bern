use bern_script::{Context, Evaluation, Value};
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
#[test]
#[cfg(not(feature = "boa"))]
fn host_routing_disposal_and_shutdown() {
    use bern_script::host;
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
