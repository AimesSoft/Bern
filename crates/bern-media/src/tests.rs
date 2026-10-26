use super::*;
use bern_protocol::{CommandError, Value, check_external_names};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Records every request; tests decide when and how it completes.
#[derive(Default)]
struct Executor {
    requests: Mutex<Vec<ExecRequest>>,
}

impl MediaExecutor for Executor {
    fn execute(&self, request: ExecRequest) {
        self.requests.lock().unwrap().push(request);
    }
}

impl Executor {
    fn take(&self) -> Vec<ExecRequest> {
        std::mem::take(&mut self.requests.lock().unwrap())
    }
}

fn capabilities() -> MediaCapabilities {
    MediaCapabilities {
        backend: "test".into(),
        backend_version: "1".into(),
        background_playback: true,
        multi_view: true,
    }
}

fn hub() -> (MediaHub, Arc<Executor>) {
    let hub = MediaHub::new(&["preview".to_string()]);
    let executor = Arc::new(Executor::default());
    hub.attach_executor(executor.clone(), capabilities());
    (hub, executor)
}

fn source(url: &str) -> Value {
    Value::Record(vec![Value::Text(url.into())])
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

/// Dispatch and capture the reply, which may arrive later.
fn dispatch(
    hub: &MediaHub,
    command: MediaCommand,
    params: Vec<Value>,
) -> Arc<Mutex<Option<Result<Value, CommandError>>>> {
    let slot = Arc::new(Mutex::new(None));
    let target = Arc::clone(&slot);
    hub.dispatch(command, params, move |result| {
        *target.lock().unwrap() = Some(result);
    });
    slot
}

fn field<'a>(player: &'a Value, name: &str) -> &'a Value {
    let index = PLAYER_FIELDS
        .iter()
        .position(|(field, _, _)| *field == name)
        .unwrap();
    let Value::Record(fields) = player else {
        panic!("not a record")
    };
    &fields[index]
}

fn phase(hub: &MediaHub, id: &str) -> Phase {
    match field(&hub.player(id).unwrap(), "phase") {
        Value::Enum { variant, .. } => Phase::from_index(*variant).unwrap(),
        other => panic!("{other:?}"),
    }
}

fn playing(position_ms: i64) -> Observation {
    Observation {
        phase: Phase::Playing,
        position_ms,
        duration_ms: Some(8_000),
        seekable: true,
        volume: 1.0,
        muted: false,
        error: None,
    }
}

#[test]
fn catalog_and_state_schema_are_consistent() {
    let (types, commands) = media_catalog();
    types.validate().unwrap();
    check_external_names(commands.iter()).unwrap();
    assert_eq!(commands.len(), MediaCommand::ALL.len());
    for (index, command) in MediaCommand::ALL.into_iter().enumerate() {
        assert_eq!(commands[index].id, command.id());
        assert_eq!(MediaCommand::from_id(command.id()), Some(command));
        assert_eq!(MediaCommand::from_index(command.index()), Some(command));
    }
    let play = &commands[MediaCommand::Play.index() as usize];
    assert_eq!(play.external.cli, vec!["media", "play"]);
    assert_eq!(play.external.mcp, "media_play");
    // Every published player value matches the declared record.
    let (hub, _) = hub();
    for player in hub.snapshot().1 {
        player
            .check(
                &bern_protocol::Type::Record {
                    name: "MediaPlayer".into(),
                },
                &types,
            )
            .unwrap();
    }
}

#[test]
fn executor_protocol_round_trips() {
    for op in [
        ExecOp::Load {
            url: "file:///a.mp4".into(),
        },
        ExecOp::Unload,
        ExecOp::Play,
        ExecOp::Seek { position_ms: 1_500 },
        ExecOp::SetVolume { volume: 0.25 },
        ExecOp::SetMuted { muted: true },
        ExecOp::Release,
    ] {
        let request = ExecRequest {
            request: 7,
            player: "main#1/page.video".into(),
            source_version: 3,
            op,
        };
        assert_eq!(ExecRequest::decode(&request.encode()).unwrap(), request);
    }
    let observation = Observation {
        duration_ms: None,
        error: Some("bad".into()),
        ..playing(10)
    };
    let (player, version, decoded) =
        Observation::decode(&observation.encode("preview", 4)).unwrap();
    assert_eq!(
        (player.as_str(), version, decoded),
        ("preview", 4, observation)
    );
}

#[test]
fn commands_route_to_the_executor_and_wait_for_execution() {
    let (hub, executor) = hub();
    let reply = dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///clip.mp4")],
    );
    let requests = executor.take();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].op,
        ExecOp::Load {
            url: "file:///clip.mp4".into()
        }
    );
    assert_eq!(requests[0].source_version, 1);
    // Accepted but not executed yet: no reply, request status accepted.
    assert!(reply.lock().unwrap().is_none());
    let player = hub.player("preview").unwrap();
    assert_eq!(phase(&hub, "preview"), Phase::Loading);
    let Value::Optional(Some(request)) = field(&player, "request") else {
        panic!()
    };
    assert_eq!(
        **request,
        Value::Record(vec![
            text("media.open"),
            Value::unit_variant(0),
            Value::Int(1),
            Value::none()
        ])
    );
    hub.complete(requests[0].request, Ok(()));
    assert_eq!(reply.lock().unwrap().clone(), Some(Ok(Value::Int(1))));
    // Executed is not playing: the phase only follows observations.
    assert_eq!(phase(&hub, "preview"), Phase::Loading);
    assert!(hub.observe("preview", 1, playing(0)));
    assert_eq!(phase(&hub, "preview"), Phase::Playing);
}

#[test]
fn reports_never_dispatch_commands() {
    let (hub, executor) = hub();
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///clip.mp4")],
    );
    let load = executor.take().remove(0);
    hub.complete(load.request, Ok(()));
    dispatch(&hub, MediaCommand::Play, vec![text("preview")]);
    let play = executor.take();
    assert_eq!(play.len(), 1);
    hub.complete(play[0].request, Ok(()));
    // The backend reports facts — playing, pausing on its own, ending — and
    // none of them turns into another request.
    for (position, phase_now) in [
        (0, Phase::Playing),
        (500, Phase::Playing),
        (900, Phase::Paused),
        (8_000, Phase::Ended),
    ] {
        hub.observe(
            "preview",
            1,
            Observation {
                phase: phase_now,
                ..playing(position)
            },
        );
    }
    assert!(executor.take().is_empty());
    assert_eq!(phase(&hub, "preview"), Phase::Ended);
}

#[test]
fn reports_from_an_older_source_are_ignored() {
    let hub = MediaHub::new(&[]);
    let executor = Arc::new(Executor::default());
    hub.attach_executor(executor.clone(), capabilities());
    hub.create("main#1/page.video", "main#1").unwrap();
    assert_eq!(
        hub.bind("main#1/page.video", Some("file:///a.mp4".into())),
        Ok(1)
    );
    assert_eq!(
        hub.bind("main#1/page.video", Some("file:///b.mp4".into())),
        Ok(2)
    );
    let loads = executor.take();
    assert_eq!(loads.len(), 2);
    assert_eq!(loads[1].source_version, 2);
    // A late report for source 1 must not overwrite source 2.
    assert!(!hub.observe(
        "main#1/page.video",
        1,
        Observation {
            phase: Phase::Failed,
            error: Some("old".into()),
            ..playing(0)
        }
    ));
    assert_eq!(hub.ignored_reports(), 1);
    assert_eq!(phase(&hub, "main#1/page.video"), Phase::Loading);
    assert!(hub.observe("main#1/page.video", 2, playing(100)));
    assert_eq!(phase(&hub, "main#1/page.video"), Phase::Playing);
    // A late completion of the old load does not change the newer request.
    hub.complete(loads[0].request, Err(CommandError::new("stale_source", "")));
    let player = hub.player("main#1/page.video").unwrap();
    let Value::Optional(Some(request)) = field(&player, "request") else {
        panic!()
    };
    let Value::Record(request) = &**request else {
        panic!()
    };
    assert_eq!(request[2], Value::Int(2));
    assert_eq!(request[1], Value::unit_variant(0));
}

#[test]
fn bound_sources_load_once_and_refuse_external_replacement() {
    let (hub, executor) = hub();
    hub.create("main#1/page.video", "main#1").unwrap();
    for _ in 0..3 {
        // Re-evaluating the same binding, as redraws do, never reloads.
        assert_eq!(
            hub.bind("main#1/page.video", Some("file:///a.mp4".into())),
            Ok(1)
        );
    }
    assert_eq!(executor.take().len(), 1);
    let reply = dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("main#1/page.video"), source("file:///other.mp4")],
    );
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "source_bound"
    );
    // Playback commands still reach the same player.
    hub.observe(
        "main#1/page.video",
        1,
        Observation {
            phase: Phase::Ready,
            ..playing(0)
        },
    );
    dispatch(&hub, MediaCommand::Play, vec![text("main#1/page.video")]);
    assert_eq!(executor.take()[0].op, ExecOp::Play);
    // Application players are not bound and cannot be bound.
    assert_eq!(
        hub.bind("preview", Some("file:///a.mp4".into()))
            .unwrap_err()
            .code,
        "not_source_bound"
    );
    assert_eq!(
        hub.create("main#1/page.video", "main#1").unwrap_err().code,
        "player_exists"
    );
}

#[test]
fn unplayable_media_is_reported_and_refused() {
    let (hub, executor) = hub();
    let reply = dispatch(&hub, MediaCommand::Play, vec![text("preview")]);
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "no_source"
    );
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///live.m3u8")],
    );
    let load = executor.take().remove(0);
    hub.complete(load.request, Ok(()));
    // Live media: playing, but the duration is unknown and it cannot seek.
    hub.observe(
        "preview",
        1,
        Observation {
            duration_ms: None,
            seekable: false,
            ..playing(4_000)
        },
    );
    let player = hub.player("preview").unwrap();
    assert_eq!(*field(&player, "duration_ms"), Value::none());
    let reply = dispatch(
        &hub,
        MediaCommand::Seek,
        vec![text("preview"), Value::Int(1_000)],
    );
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "not_seekable"
    );
    // Seekable media rejects positions outside its duration.
    hub.observe("preview", 1, playing(0));
    let reply = dispatch(
        &hub,
        MediaCommand::Seek,
        vec![text("preview"), Value::Int(9_000)],
    );
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "out_of_range"
    );
    let reply = dispatch(
        &hub,
        MediaCommand::SetVolume,
        vec![text("preview"), Value::Float(1.5)],
    );
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "out_of_range"
    );
    // A source that fails to load refuses playback with its error.
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///broken.mp4")],
    );
    let load = executor.take().remove(0);
    hub.complete(load.request, Ok(()));
    hub.observe(
        "preview",
        2,
        Observation {
            phase: Phase::Failed,
            error: Some("The file is damaged.".into()),
            ..Observation::idle()
        },
    );
    let reply = dispatch(&hub, MediaCommand::Play, vec![text("preview")]);
    let error = reply.lock().unwrap().clone().unwrap().unwrap_err();
    assert_eq!(error.code, "media_failed");
    assert_eq!(error.message, "The file is damaged.");
    assert!(executor.take().is_empty());
}

#[test]
fn executor_failures_and_timeouts_are_explicit() {
    let (hub, executor) = hub();
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///clip.mp4")],
    );
    let load = executor.take().remove(0);
    hub.complete(load.request, Ok(()));
    hub.observe(
        "preview",
        1,
        Observation {
            phase: Phase::Ready,
            ..playing(0)
        },
    );
    let reply = dispatch(&hub, MediaCommand::Play, vec![text("preview")]);
    let play = executor.take().remove(0);
    hub.complete(
        play.request,
        Err(CommandError::new(
            "stale_source",
            "a newer source is loaded",
        )),
    );
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "stale_source"
    );
    let player = hub.player("preview").unwrap();
    let Value::Optional(Some(request)) = field(&player, "request") else {
        panic!()
    };
    assert_eq!(
        **request,
        Value::Record(vec![
            text("media.play"),
            Value::unit_variant(2),
            Value::Int(1),
            Value::some(text("stale_source"))
        ])
    );
    // No answer within the timeout: the outcome is unknown, not success.
    let error = hub
        .dispatch_blocking(
            MediaCommand::Pause,
            vec![text("preview")],
            Duration::from_millis(20),
        )
        .unwrap_err();
    assert_eq!(error.code, "media_timeout");
    let pause = executor.take().remove(0);
    hub.complete(pause.request, Ok(()));
}

#[test]
fn a_detached_executor_makes_players_unavailable() {
    let (hub, executor) = hub();
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///clip.mp4")],
    );
    let load = executor.take().remove(0);
    hub.complete(load.request, Ok(()));
    hub.observe("preview", 1, playing(1_000));
    let waiting = dispatch(&hub, MediaCommand::Pause, vec![text("preview")]);
    hub.detach_executor();
    assert_eq!(
        waiting.lock().unwrap().clone().unwrap().unwrap_err().code,
        "executor_unavailable"
    );
    // The confirmed "playing" state is no longer shown.
    assert_eq!(phase(&hub, "preview"), Phase::Unavailable);
    assert!(!hub.observe("preview", 1, playing(2_000)));
    let reply = dispatch(&hub, MediaCommand::Play, vec![text("preview")]);
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "executor_unavailable"
    );
    // A new executor reloads the source under a new version.
    let next = Arc::new(Executor::default());
    hub.attach_executor(next.clone(), capabilities());
    let reload = next.take();
    assert_eq!(reload.len(), 1);
    assert_eq!(reload[0].source_version, 2);
    assert_eq!(phase(&hub, "preview"), Phase::Loading);
}

#[test]
fn component_players_end_with_their_owner_and_application_players_stay() {
    let (hub, executor) = hub();
    hub.create("main#1/page.video", "main#1").unwrap();
    hub.bind("main#1/page.video", Some("file:///a.mp4".into()))
        .unwrap();
    dispatch(
        &hub,
        MediaCommand::Open,
        vec![text("preview"), source("file:///b.mp4")],
    );
    executor.take();
    hub.attach_view("a", "preview", "main#1");
    hub.attach_view("b", "main#1/page.video", "main#1");
    // Leaving the page detaches its views and releases its own player.
    hub.detach_view("a");
    hub.detach_view("b");
    hub.release("main#1/page.video");
    let release = executor.take();
    assert_eq!(release.len(), 1);
    assert_eq!(release[0].op, ExecOp::Release);
    assert!(hub.player("main#1/page.video").is_none());
    // The application player keeps its source and has no views.
    let preview = hub.player("preview").unwrap();
    assert_eq!(
        *field(&preview, "source"),
        Value::some(source("file:///b.mp4"))
    );
    assert_eq!(*field(&preview, "views"), Value::Int(0));
    let reply = dispatch(&hub, MediaCommand::State, vec![text("main#1/page.video")]);
    assert_eq!(
        reply.lock().unwrap().clone().unwrap().unwrap_err().code,
        "player_not_found"
    );
}

#[test]
fn listeners_receive_complete_versioned_snapshots() {
    let (hub, _) = hub();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let subscription = hub.subscribe(move |update| {
        sink.lock().unwrap().push(update.clone());
    });
    hub.attach_view("a", "preview", "main#1");
    hub.detach_view("a");
    drop(subscription);
    hub.attach_view("b", "preview", "main#1");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].revision < seen[1].revision);
    assert_eq!(seen[1].players.len(), 1);
}

#[test]
fn closing_a_window_releases_what_it_owns() {
    let (hub, executor) = hub();
    hub.create("main#1/page.video", "main#1").unwrap();
    hub.bind("main#1/page.video", Some("file:///a.mp4".into()))
        .unwrap();
    hub.create("settings#1/page.video", "settings#1").unwrap();
    hub.attach_view("a", "main#1/page.video", "main#1");
    hub.attach_view("b", "preview", "main#1");
    hub.attach_view("c", "preview", "settings#1");
    executor.take();
    hub.release_window("main#1");
    // The window's own player is released; its views no longer count.
    let released = executor.take();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].player, "main#1/page.video");
    assert_eq!(released[0].op, ExecOp::Release);
    assert!(hub.player("main#1/page.video").is_none());
    assert!(hub.player("settings#1/page.video").is_some());
    assert_eq!(
        *field(&hub.player("preview").unwrap(), "views"),
        Value::Int(1)
    );
    // A view the toolkit tears down later changes nothing.
    hub.detach_view("b");
    assert_eq!(
        *field(&hub.player("preview").unwrap(), "views"),
        Value::Int(1)
    );
}

#[test]
fn frame_sinks_reach_the_kernel_while_their_views_keep_them() {
    use crate::kernel::{self, FrameSink, SurfaceHandle, SurfaceKind, VideoFrame};
    #[derive(Default)]
    struct Sink(Mutex<Vec<(u32, u32)>>);
    impl FrameSink for Sink {
        fn present(&self, frame: VideoFrame) {
            self.0.lock().unwrap().push((frame.width, frame.height));
        }
    }
    let sink = Arc::new(Sink::default());
    let handle = kernel::register_frame_sink(sink.clone());
    assert_eq!(handle.kind(), SurfaceKind::FrameSink);
    assert_eq!(
        SurfaceKind::from_code(SurfaceKind::FrameSink.code()),
        Some(SurfaceKind::FrameSink)
    );
    kernel::frame_sink(handle)
        .expect("registered")
        .present(VideoFrame {
            width: 2,
            height: 1,
            pixels: vec![0; 8],
        });
    assert_eq!(*sink.0.lock().unwrap(), [(2, 1)]);
    // Other surfaces name no sink, and a view's sink is gone once it lets
    // go of it; the next view gets a sink of its own.
    assert!(kernel::frame_sink(SurfaceHandle::MetalLayer(1)).is_none());
    kernel::unregister_frame_sink(handle);
    assert!(kernel::frame_sink(handle).is_none());
    let next = kernel::register_frame_sink(sink);
    assert_ne!(next, handle);
    kernel::unregister_frame_sink(next);
}
