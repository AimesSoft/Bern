// Bern runtime for WinUI 3: the media executor and the `Video` view with
// its standard, command-driven control bar. Generated build input; do not
// edit.
//
// The executor plays each player with one Windows.Media.Playback.MediaPlayer
// and reports what it does as observations, never as commands. Views
// present a player with a MediaPlayerElement; the control bar sends the
// same media commands every other entry uses, and shows the hub's merged
// state.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using global::Windows.Media.Playback;

namespace Bern;

/// The standard media commands declarations use on players, by their
/// index in the media catalog.
public enum MediaOp : uint
{
    Open = 2,
    Play = 3,
    Pause = 4,
    Seek = 5,
    SetVolume = 6,
    SetMuted = 7,
}

/// Playback phases, by index.
static class Phase
{
    public const uint Idle = 0, Loading = 1, Ready = 2, Playing = 3, Paused = 4, Buffering = 5, Ended = 6,
        Failed = 7, Unavailable = 8;

    public static readonly string[] Names =
        { "Idle", "Loading", "Ready", "Playing", "Paused", "Buffering", "Ended", "Failed", "Unavailable" };
}

/// The merged state of one player, from the hub.
sealed record PlayerState(string? Source, uint Phase, long PositionMs, long? DurationMs, bool Seekable,
    double Volume, bool Muted, string? Error)
{
    public static (string Id, PlayerState State)? From(Value value)
    {
        if (value is not RecordValue { Fields: var fields }) return null;
        Value Field(int index) => index < fields.Length ? fields[index] : Value.Unit;
        var source = Field(3) is OptionalValue { Inner: RecordValue { Fields: [TextValue url, ..] } } ? url.V : null;
        var phase = Field(5) is EnumValue e ? e.Variant : Bern.Phase.Unavailable;
        long? duration = Field(7) is OptionalValue { Inner: IntValue d } ? d.V : null;
        var error = Field(11) is OptionalValue { Inner: TextValue t } ? t.V : null;
        return (R.AsText(Field(0)), new PlayerState(source, phase, Field(6) is IntValue p ? p.V : 0, duration,
            R.Truthy(Field(8)), Field(9) is FloatValue or IntValue ? R.AsDouble(Field(9)) : 1, R.Truthy(Field(10)), error));
    }

    public bool Is(uint phase) => Phase == phase;
    public bool IsPlaying => Is(Bern.Phase.Playing) || Is(Bern.Phase.Buffering);
    public bool Playable => Source is not null && !Is(Bern.Phase.Idle) && !Is(Bern.Phase.Failed) && !Is(Bern.Phase.Unavailable);
    public bool CanSeek => Playable && Seekable && DurationMs is not null;
    public bool Controllable => !Is(Bern.Phase.Unavailable);
    public bool ShowsFrames => Phase is Bern.Phase.Ready or Bern.Phase.Playing or Bern.Phase.Paused or Bern.Phase.Buffering or Bern.Phase.Ended;

    public string Describe()
    {
        var name = Phase < Bern.Phase.Names.Length ? Bern.Phase.Names[Phase] : "Unavailable";
        var duration = DurationMs is { } d ? Media.Time(d) : "--:--";
        return $"{name} {Media.Time(PositionMs)} / {duration}";
    }
}

/// One player of the executor.
sealed class MediaStream
{
    readonly string id;
    internal readonly MediaPlayer Player = new() { AutoPlay = false };
    ulong version;
    bool loaded, started, opened, ended, released;
    string? error;
    DateTime lastReport = DateTime.MinValue;
    TimeSpan lastPosition;
    readonly List<Action> seeks = new();

    public MediaStream(string id)
    {
        this.id = id;
        // Commands come through the hub; the system media controls would
        // bypass it.
        Player.CommandManager.IsEnabled = false;
        var session = Player.PlaybackSession;
        Player.MediaOpened += (_, _) => Host.Post(() => { opened = true; Report(); });
        Player.MediaFailed += (_, args) => Host.Post(() =>
        {
            error = Describe(args);
            Console.Error.WriteLine($"bern: media {id}: {args.Error}: {args.ErrorMessage}");
            Report();
        });
        Player.MediaEnded += (_, _) => Host.Post(() => { ended = true; Report(); });
        Player.VolumeChanged += (_, _) => Host.Post(Report);
        Player.IsMutedChanged += (_, _) => Host.Post(Report);
        session.PlaybackStateChanged += (_, _) => Host.Post(Report);
        session.NaturalDurationChanged += (_, _) => Host.Post(Report);
        session.SeekCompleted += (_, _) => Host.Post(() =>
        {
            Report();
            var done = seeks.ToList();
            seeks.Clear();
            foreach (var complete in done) complete();
        });
        session.PositionChanged += (_, _) => Host.Post(() =>
        {
            // Playback reports twice a second; a jump reports at once.
            var position = Player.PlaybackSession.Position;
            var jumped = Math.Abs((position - lastPosition).TotalMilliseconds) > 750;
            if (jumped || DateTime.UtcNow - lastReport >= TimeSpan.FromMilliseconds(500)) Report();
        });
    }

    /// What went wrong, in the system's words when it has some.
    static string Describe(MediaPlayerFailedEventArgs args)
    {
        if (!string.IsNullOrWhiteSpace(args.ErrorMessage)) return Summary(args.ErrorMessage);
        var text = args.Error switch
        {
            MediaPlayerError.SourceNotSupported => "The file is damaged or its format is not supported",
            MediaPlayerError.DecodingError => "The media could not be decoded",
            MediaPlayerError.NetworkError => "The media could not be loaded from the network",
            MediaPlayerError.Aborted => "Loading the media was aborted",
            _ => "The media could not be played",
        };
        return args.ExtendedErrorCode is { HResult: var code } && code != 0 ? $"{text} (0x{code:X8})" : text;
    }

    /// The first line of a media error; the rest is detail for the log.
    static string Summary(string message) =>
        message.Split('\n').Select(line => line.Trim()).FirstOrDefault(line => line.Length > 0) ?? message;

    (uint Phase, string? Error) CurrentPhase()
    {
        if (!loaded) return (Bern.Phase.Idle, null);
        if (error is not null) return (Bern.Phase.Failed, error);
        var state = Player.PlaybackSession.PlaybackState;
        if (!opened) return (started ? Bern.Phase.Buffering : Bern.Phase.Loading, null);
        if (ended) return (Bern.Phase.Ended, null);
        return state switch
        {
            MediaPlaybackState.Playing => (Bern.Phase.Playing, null),
            MediaPlaybackState.Buffering or MediaPlaybackState.Opening => (started ? Bern.Phase.Buffering : Bern.Phase.Loading, null),
            _ => (started ? Bern.Phase.Paused : Bern.Phase.Ready, null),
        };
    }

    internal unsafe void Report()
    {
        // Events queued before the player was released arrive afterwards.
        if (Host.Handle == 0 || released) return;
        var session = Player.PlaybackSession;
        lastReport = DateTime.UtcNow;
        lastPosition = session.Position;
        var (phase, failure) = CurrentPhase();
        var duration = session.NaturalDuration;
        var known = opened && duration > TimeSpan.Zero;
        var writer = new Writer();
        writer.Text(id);
        writer.U64(version);
        writer.U32(phase);
        writer.I64((long)Math.Max(0, session.Position.TotalMilliseconds));
        if (known)
        {
            writer.U8(1);
            writer.I64((long)duration.TotalMilliseconds);
        }
        else writer.U8(0);
        writer.U8(known && session.CanSeek ? (byte)1 : (byte)0);
        writer.F64(Math.Round(Player.Volume * 1000) / 1000);
        writer.U8(Player.IsMuted ? (byte)1 : (byte)0);
        if (failure is not null)
        {
            writer.U8(1);
            writer.Text(failure);
        }
        else writer.U8(0);
        Native.With(writer.ToArray(), (pointer, len) => Native.bern_media_event(Host.Handle, (byte*)pointer, len));
    }

    internal async void Load(string url, ulong next)
    {
        version = next;
        started = false;
        opened = false;
        ended = false;
        error = null;
        loaded = true;
        Player.Pause();
        Player.Source = null;
        Report();
        try
        {
            var uri = new Uri(url);
            global::Windows.Media.Core.MediaSource source;
            if (uri.IsFile)
            {
                var file = await global::Windows.Storage.StorageFile.GetFileFromPathAsync(uri.LocalPath);
                source = global::Windows.Media.Core.MediaSource.CreateFromStorageFile(file);
            }
            else source = global::Windows.Media.Core.MediaSource.CreateFromUri(uri);
            if (version != next || released) return;
            Player.Source = source;
        }
        catch (Exception failure)
        {
            if (version != next) return;
            error = failure.Message.Split('\n')[0].Trim();
            Report();
        }
    }

    internal void Unload(ulong next)
    {
        version = next;
        started = opened = ended = loaded = false;
        error = null;
        Player.Pause();
        Player.Source = null;
        Report();
    }

    internal ulong Version => version;

    internal void Play()
    {
        if (ended)
        {
            ended = false;
            Player.PlaybackSession.Position = TimeSpan.Zero;
        }
        started = true;
        Player.Play();
        Report();
    }

    internal void Pause()
    {
        Player.Pause();
        Report();
    }

    internal void Seek(long positionMs, Action done)
    {
        ended = false;
        seeks.Add(done);
        Player.PlaybackSession.Position = TimeSpan.FromMilliseconds(Math.Max(0, positionMs));
        // A seek the session finishes at once reports no completion.
        Host.After(TimeSpan.FromSeconds(3), () =>
        {
            if (!seeks.Remove(done)) return;
            Report();
            done();
        });
    }

    internal void Release()
    {
        if (released) return;
        released = true;
        seeks.Clear();
        Player.Pause();
        Player.Source = null;
        Player.Dispose();
    }
}

public static unsafe class Media
{
    static readonly Dictionary<string, MediaStream> streams = new();
    static readonly Cell<Dictionary<string, PlayerState>> players = new(new());
    static ulong revision;
    static ulong views;

    internal static MediaStream StreamOf(string id)
    {
        if (!streams.TryGetValue(id, out var stream)) streams[id] = stream = new MediaStream(id);
        return stream;
    }

    internal static string Time(long milliseconds)
    {
        var seconds = Math.Max(0, milliseconds) / 1000;
        var (hours, minutes, rest) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
        return hours > 0
            ? string.Format(CultureInfo.InvariantCulture, "{0}:{1:00}:{2:00}", hours, minutes, rest)
            : string.Format(CultureInfo.InvariantCulture, "{0}:{1:00}", minutes, rest);
    }

    static byte[] Call(delegate*<nint, byte*, nuint, BernBuffer> function, byte[] bytes)
    {
        fixed (byte* pointer = bytes) return Native.Take(function(Host.Handle, pointer, (nuint)bytes.Length));
    }

    static void Post(delegate*<nint, byte*, nuint, void> function, byte[] bytes)
    {
        fixed (byte* pointer = bytes) function(Host.Handle, pointer, (nuint)bytes.Length);
    }

    /// Attach this frontend's executor to the hub and take the current state.
    internal static void AttachExecutor()
    {
        var writer = new Writer();
        writer.Text("windows-mediaplayer");
        writer.Text("1");
        writer.U8(1); // keeps playing without a view
        writer.U8(1); // several views may show one player
        var reply = Call(&Native.bern_media_executor_attached, writer.ToArray());
        if (Host.Status(reply).Failure is { } failure) Console.Error.WriteLine($"bern: media executor: {failure.Message}");
        ReceiveState(Native.Take(Native.bern_media_snapshot(Host.Handle)));
    }

    internal static void DetachExecutor()
    {
        if (Host.Handle != 0) Native.bern_media_executor_detached(Host.Handle);
        foreach (var stream in streams.Values) stream.Release();
        streams.Clear();
    }

    /// Merged player states: revision, count, then `MediaPlayer` records.
    internal static void ReceiveState(byte[] bytes)
    {
        try
        {
            var reader = new Reader(bytes);
            var next = reader.U64();
            var count = reader.U32();
            if (next < revision) return;
            revision = next;
            var states = new Dictionary<string, PlayerState>();
            var type = BType.Record("MediaPlayer");
            for (var i = 0; i < count; i++)
                if (PlayerState.From(Codec.Decode(type, Host.App.Types, reader)) is { } entry)
                    states[entry.Id] = entry.State;
            players.Set(states);
        }
        catch (DecodeException error)
        {
            Console.Error.WriteLine($"bern: media state: {error.Message}");
        }
        Snapshot.Schedule();
    }

    static void Complete(ulong request, Failure? failure)
    {
        var writer = new Writer();
        writer.U64(request);
        if (failure is null) writer.U8(0);
        else
        {
            writer.U8(1);
            writer.Text(failure.Code);
            writer.Text(failure.Message);
        }
        Post(&Native.bern_media_complete, writer.ToArray());
    }

    /// Perform one hub request.
    internal static void Request(byte[] bytes)
    {
        ulong request;
        string player;
        ulong sourceVersion;
        uint op;
        Reader reader;
        try
        {
            reader = new Reader(bytes);
            request = reader.U64();
            player = reader.Text();
            sourceVersion = reader.U64();
            op = reader.U32();
        }
        catch (DecodeException)
        {
            return;
        }
        switch (op)
        {
            case 0:
                StreamOf(player).Load(reader.Text(), sourceVersion);
                Complete(request, null);
                return;
            case 1:
                StreamOf(player).Unload(sourceVersion);
                Complete(request, null);
                return;
            case 7:
                if (streams.Remove(player, out var removed)) removed.Release();
                Complete(request, null);
                return;
        }
        var stream = StreamOf(player);
        if (stream.Version != sourceVersion)
        {
            Complete(request, new Failure("stale_source", "a newer source is loaded"));
            return;
        }
        switch (op)
        {
            case 2:
                stream.Play();
                Complete(request, null);
                break;
            case 3:
                stream.Pause();
                Complete(request, null);
                break;
            case 4:
                stream.Seek(reader.I64(), () => Complete(request, null));
                break;
            case 5:
                stream.Player.Volume = Math.Clamp(reader.F64(), 0, 1);
                Complete(request, null);
                break;
            case 6:
                stream.Player.IsMuted = reader.U8() != 0;
                Complete(request, null);
                break;
            default:
                Complete(request, new Failure("unknown_command", "unknown media operation"));
                break;
        }
    }

    static readonly BType[][] parameters =
    {
        Array.Empty<BType>(),
        new[] { BType.Text },
        new[] { BType.Text, BType.Record("MediaSource") },
        new[] { BType.Text },
        new[] { BType.Text },
        new[] { BType.Text, BType.Int },
        new[] { BType.Text, BType.Float },
        new[] { BType.Text, BType.Bool },
    };

    /// Dispatch a standard media command on `player` with its arguments
    /// after the player id.
    public static void Send(Ctx ctx, MediaOp op, string player, params Value[] args)
    {
        var types = parameters[(int)op];
        var writer = new Writer();
        var values = new[] { Value.Text(player) }.Concat(args).ToArray();
        for (var i = 0; i < values.Length && i < types.Length; i++) Codec.Encode(values[i], types[i], Host.App.Types, writer);
        var token = Host.Token(ctx.Win, null);
        Native.With(writer.ToArray(), (pointer, len) => Native.bern_media_dispatch(Host.Handle, (uint)op, (byte*)pointer, len, token));
    }

    /// `players.<id>.open(source)`: `source` is a `MediaSource` value.
    public static void Open(Ctx ctx, string player, Value source) => Send(ctx, MediaOp.Open, player, source);

    internal static Failure? Create(string player, string owner)
    {
        var writer = new Writer();
        writer.Text(player);
        writer.Text(owner);
        return Host.Status(Call(&Native.bern_media_create, writer.ToArray())).Failure;
    }

    internal static Failure? Bind(string player, string? url)
    {
        var writer = new Writer();
        writer.Text(player);
        if (url is null) writer.U8(0);
        else
        {
            writer.U8(1);
            writer.Text(url);
        }
        return Host.Status(Call(&Native.bern_media_bind, writer.ToArray())).Failure;
    }

    internal static void Release(string player)
    {
        var writer = new Writer();
        writer.Text(player);
        Post(&Native.bern_media_release, writer.ToArray());
    }

    internal static string AttachView(string player, string window)
    {
        var token = $"view-{++views}";
        var writer = new Writer();
        writer.Text(token);
        writer.U8(1);
        writer.Text(player);
        writer.Text(window);
        Post(&Native.bern_media_view, writer.ToArray());
        return token;
    }

    internal static void DetachView(string token)
    {
        var writer = new Writer();
        writer.Text(token);
        writer.U8(0);
        Post(&Native.bern_media_view, writer.ToArray());
    }

    /// The hub's state of `player`, recording a dependency.
    internal static PlayerState? State(string player) => players.Get().GetValueOrDefault(player);
    internal static PlayerState? Peek(string player) => players.Peek().GetValueOrDefault(player);
}

public static partial class B
{
    static string? UrlOf(Value value)
    {
        if (value is OptionalValue optional)
        {
            if (optional.Inner is null) return null;
            value = optional.Inner;
        }
        return value is RecordValue { Fields: [TextValue url, ..] } ? url.V : null;
    }

    /// A slider that previews while dragged and commits on release;
    /// keyboard changes commit at once.
    sealed class Scrubber
    {
        public readonly Slider Slider = new() { IsThumbToolTipEnabled = false };
        public bool Updating;
        public bool Dragging;

        public Scrubber(Action<double> commit, Action<double>? preview)
        {
            Slider.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) => Dragging = true), true);
            void Release()
            {
                if (!Dragging) return;
                Dragging = false;
                commit(Slider.Value);
            }
            Slider.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler((_, _) => Release()), true);
            Slider.AddHandler(UIElement.PointerCaptureLostEvent, new PointerEventHandler((_, _) => Release()), true);
            Slider.ValueChanged += (_, args) =>
            {
                if (Updating) return;
                if (Dragging) preview?.Invoke(args.NewValue);
                else commit(args.NewValue);
            };
        }

        public void Show(double value)
        {
            if (Dragging) return;
            Updating = true;
            Slider.Value = value;
            Updating = false;
        }
    }

    /// A video of a declaration's own player (`source`) or of an application
    /// player, with the standard command-driven control bar.
    public static void Video(Parent p, Ctx ctx, string id, Get? source = null, string? player = null, Str? label = null,
        bool controls = false, string? fit = null)
    {
        var win = ctx.Win;
        Func<string> labelText = label is null ? () => Bern.Text.Tr("bern.media.video") : () => label(ctx);
        var playerId = player ?? $"{win.Id}/{ctx.Target(id)}";
        if (source is not null)
        {
            if (Media.Create(playerId, win.Id) is { } failure) win.Fail(failure);
            else
            {
                Reactive.OnCleanup(() => Media.Release(playerId));
                var bound = false;
                string? boundUrl = null;
                Reactive.Effect(() =>
                {
                    var url = UrlOf(source(ctx));
                    if (bound && boundUrl == url) return;
                    bound = true;
                    boundUrl = url;
                    if (Reactive.Untrack(() => Media.Bind(playerId, url)) is { } bindFailure) win.Fail(bindFailure);
                });
            }
        }

        var element = new MediaPlayerElement
        {
            AreTransportControlsEnabled = false,
            Stretch = fit switch { "cover" => Stretch.UniformToFill, "fill" => Stretch.Fill, _ => Stretch.Uniform },
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
        };
        element.SetMediaPlayer(Media.StreamOf(playerId).Player);
        var frame = new Grid { MinHeight = 120, CornerRadius = new CornerRadius(4) };
        frame.SetValue(Panel.BackgroundProperty, Application.Current.Resources["ControlFillColorTertiaryBrush"]);
        frame.Children.Add(element);
        var status = new StackPanel { Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 420 };
        frame.Children.Add(status);
        // The picture area keeps 16:9 at the width it gets.
        frame.SizeChanged += (_, args) =>
        {
            var height = Math.Max(120, Math.Round(args.NewSize.Width * 9 / 16));
            if (double.IsNaN(frame.Height) || Math.Abs(frame.Height - height) > 0.5) frame.Height = height;
        };
        var token = Media.AttachView(playerId, win.Id);
        Reactive.OnCleanup(() =>
        {
            Media.DetachView(token);
            try { element.SetMediaPlayer(null); }
            catch (Exception) { }
        });
        Reactive.Effect(() =>
        {
            var state = Media.State(playerId);
            element.Opacity = state?.ShowsFrames == true ? 1 : 0;
            status.Children.Clear();
            void Add(string icon, string title, string? detail)
            {
                var symbol = Icons.Icon(icon, 32);
                symbol.HorizontalAlignment = HorizontalAlignment.Center;
                status.Children.Add(symbol);
                var heading = new TextBlock { Text = title, HorizontalAlignment = HorizontalAlignment.Center };
                if (Styles.Named("BodyStrongTextBlockStyle") is { } style) heading.Style = style;
                status.Children.Add(heading);
                if (detail is not null)
                {
                    var note = new TextBlock
                    {
                        Text = detail,
                        TextWrapping = TextWrapping.Wrap,
                        TextAlignment = TextAlignment.Center,
                        MaxLines = 3,
                        TextTrimming = TextTrimming.CharacterEllipsis,
                    };
                    Secondary(note);
                    status.Children.Add(note);
                }
            }
            switch (state)
            {
                case null:
                case { Phase: Phase.Unavailable }:
                    Add("film", Bern.Text.Tr("bern.media.unavailable"), null);
                    break;
                case { Phase: Phase.Idle }:
                    Add("film", Bern.Text.Tr("bern.media.empty"), null);
                    break;
                case { Phase: Phase.Loading }:
                    status.Children.Add(new ProgressRing { IsActive = true, Width = 32, Height = 32 });
                    break;
                case { Phase: Phase.Failed }:
                    Add("exclamationmark.triangle", Bern.Text.Tr("bern.media.failed"), state.Error);
                    break;
            }
            AutomationProperties.SetName(element, labelText());
            AutomationProperties.SetHelpText(element, state?.Describe() ?? "");
        });
        Controls.Register(ctx, id, "video", labelText, null, new Actions { Value = () => Media.Peek(playerId)?.Describe() });
        var container = new StackPanel { Spacing = 6 };
        container.Children.Add(frame);
        if (controls) container.Children.Add(ControlBar(ctx, id, playerId));
        if (p.IsList) p.Add(Card(container));
        else p.Add(container);
    }

    static FrameworkElement ControlBar(Ctx ctx, string id, string player)
    {
        void Send(MediaOp op, params Value[] args) => Media.Send(ctx, op, player, args);
        void Toggle() => Send(Media.Peek(player)?.IsPlaying == true ? MediaOp.Pause : MediaOp.Play);
        void Seek(double ms) => Send(MediaOp.Seek, Value.Int((long)Math.Round(ms)));
        void SetMuted(bool muted) => Send(MediaOp.SetMuted, Value.Bool(muted));
        void SetVolume(double volume) => Send(MediaOp.SetVolume, Value.Float(Math.Clamp(volume, 0, 1)));
        void ToggleMute() => SetMuted(Media.Peek(player)?.Muted != true);

        var playIcon = Icons.Icon("play.fill");
        var play = new Button { Content = playIcon };
        Styles.MakeSubtle(play);
        play.Click += (_, _) => Toggle();
        var positionText = new TextBlock { Text = "0:00", VerticalAlignment = VerticalAlignment.Center };
        var durationText = new TextBlock { Text = "--:--", VerticalAlignment = VerticalAlignment.Center };
        foreach (var text in new[] { positionText, durationText })
            if (Styles.Named("CaptionTextBlockStyle") is { } caption) text.Style = caption;
        var position = new Scrubber(Seek, ms => positionText.Text = Media.Time((long)ms));
        position.Slider.VerticalAlignment = VerticalAlignment.Center;
        var muteIcon = Icons.Icon("speaker.wave.3.fill");
        var mute = new Button { Content = muteIcon };
        Styles.MakeSubtle(mute);
        mute.Click += (_, _) => ToggleMute();
        var volume = new Scrubber(SetVolume, null);
        volume.Slider.Minimum = 0;
        volume.Slider.Maximum = 1;
        volume.Slider.StepFrequency = 0.05;
        volume.Slider.SmallChange = 0.05;
        volume.Slider.LargeChange = 0.1;
        volume.Slider.Width = 96;
        volume.Slider.VerticalAlignment = VerticalAlignment.Center;

        Reactive.Effect(() =>
        {
            var state = Media.State(player);
            var playing = state?.IsPlaying == true;
            var playLabel = Bern.Text.Tr(playing ? "bern.media.pause" : "bern.media.play");
            playIcon.Glyph = Icons.Glyph(playing ? "pause.fill" : "play.fill");
            ToolTipService.SetToolTip(play, playLabel);
            AutomationProperties.SetName(play, playLabel);
            play.IsEnabled = state?.Playable == true;
            var duration = state?.DurationMs ?? 0;
            var at = state?.PositionMs ?? 0;
            if (!position.Dragging)
            {
                position.Updating = true;
                position.Slider.Maximum = Math.Max(1, duration);
                position.Slider.StepFrequency = 1000;
                position.Slider.SmallChange = 5000;
                position.Slider.LargeChange = Math.Max(5000, duration / 10.0);
                position.Updating = false;
                position.Show(Math.Min(at, duration));
                positionText.Text = Media.Time(at);
            }
            position.Slider.IsEnabled = state?.CanSeek == true;
            AutomationProperties.SetName(position.Slider, Bern.Text.Tr("bern.media.position"));
            durationText.Text = state switch
            {
                { DurationMs: not null } => Media.Time(duration),
                { Playable: true } => Bern.Text.Tr("bern.media.live"),
                _ => "--:--",
            };
            var muted = state?.Muted == true;
            var muteLabel = Bern.Text.Tr(muted ? "bern.media.unmute" : "bern.media.mute");
            muteIcon.Glyph = Icons.Glyph(muted ? "speaker.slash.fill" : "speaker.wave.3.fill");
            ToolTipService.SetToolTip(mute, muteLabel);
            AutomationProperties.SetName(mute, muteLabel);
            var controllable = state?.Controllable == true;
            mute.IsEnabled = controllable;
            volume.Slider.IsEnabled = controllable;
            volume.Show(state?.Volume ?? 1);
            AutomationProperties.SetName(volume.Slider, Bern.Text.Tr("bern.media.volume"));
        });

        string Target(string name) => $"{id}.{name}";
        Controls.Register(ctx, Target("play_pause"), "button",
            () => Bern.Text.Tr(Media.State(player)?.IsPlaying == true ? "bern.media.pause" : "bern.media.play"),
            () => Media.State(player)?.Playable == true,
            new Actions { Activate = Toggle });
        Controls.Register(ctx, Target("position"), "slider", () => Bern.Text.Tr("bern.media.position"),
            () => Media.State(player)?.CanSeek == true,
            new Actions { SetNumber = Seek, Value = () => R.Number(Media.Peek(player)?.PositionMs ?? 0) });
        Controls.Register(ctx, Target("mute"), "switch", () => Bern.Text.Tr("bern.media.mute"),
            () => Media.State(player)?.Controllable == true,
            new Actions
            {
                Activate = ToggleMute,
                SetBool = SetMuted,
                Value = () => Media.Peek(player)?.Muted == true ? "true" : "false",
            });
        Controls.Register(ctx, Target("volume"), "slider", () => Bern.Text.Tr("bern.media.volume"),
            () => Media.State(player)?.Controllable == true,
            new Actions { SetNumber = SetVolume, Value = () => R.Number(Media.Peek(player)?.Volume ?? 1) });

        var bar = new Grid { ColumnSpacing = 6 };
        UIElement[] parts = { play, positionText, position.Slider, durationText, mute, volume.Slider };
        for (var i = 0; i < parts.Length; i++)
        {
            bar.ColumnDefinitions.Add(new ColumnDefinition
            {
                Width = i == 2 ? new GridLength(1, GridUnitType.Star) : GridLength.Auto,
            });
            Grid.SetColumn((FrameworkElement)parts[i], i);
            bar.Children.Add(parts[i]);
        }
        return bar;
    }
}
