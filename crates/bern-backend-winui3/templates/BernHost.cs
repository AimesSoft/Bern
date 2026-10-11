// Bern runtime for WinUI 3: the engine behind the in-process interface.
// Generated build input; do not edit.
//
// The engine, the UI session runtime and the frontend share one process
// through the same C ABI the SwiftUI target uses (`bern_app.dll`). Values
// cross it in Bern's binary layout. Callbacks arrive on engine and channel
// threads; their bytes are copied and handled on the UI thread in arrival
// order.

using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Microsoft.UI.Dispatching;

namespace Bern;

[StructLayout(LayoutKind.Sequential)]
struct BernBuffer
{
    public nint Ptr;
    public nuint Len;
    public nuint Capacity;
}

[StructLayout(LayoutKind.Sequential)]
unsafe struct BernCallbacks
{
    public nint Context;
    public delegate* unmanaged[Cdecl]<nint, byte*, nuint, void> OnState;
    public delegate* unmanaged[Cdecl]<nint, ulong, byte*, nuint, void> OnReply;
    public delegate* unmanaged[Cdecl]<nint, ulong, byte*, nuint, void> OnSessionRequest;
    public delegate* unmanaged[Cdecl]<nint, byte*, nuint, void> OnMediaRequest;
    public delegate* unmanaged[Cdecl]<nint, byte*, nuint, void> OnMediaState;
    public delegate* unmanaged[Cdecl]<nint, ulong, byte*, nuint, void> OnQuery;
}

static unsafe class Native
{
    const string Library = "bern_app";

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern nint bern_app_start(ulong fingerprint, byte* resources, nuint resourcesLen, BernCallbacks callbacks, BernBuffer* error);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_app_stop(nint app);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_app_snapshot(nint app);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_app_engine_id(nint app);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_app_dispatch(nint app, uint command, byte* parameters, nuint len, ulong token);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_window_opened(nint app, uint kind);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_window_closed(nint app, uint kind, ulong generation);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern ulong bern_window_facts(nint app, uint kind, ulong generation, byte source, byte visibility, byte size, double x, double y, double width, double height, byte focused, uint actions);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_window_request_close(nint app, uint kind, ulong generation, byte needsConfirmation, byte canSave);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_window_confirm_close(nint app, uint kind, ulong generation, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_session_set(nint app, uint kind, ulong generation, uint session, uint field, byte* value, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_session_complete(nint app, ulong request, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_query_open(nint app, uint kind, ulong generation, uint query);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_query_request(nint app, ulong subscription, byte* parameters, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_query_close(nint app, ulong subscription);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_media_executor_attached(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_executor_detached(nint app);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_media_snapshot(nint app);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_dispatch(nint app, uint command, byte* parameters, nuint len, ulong token);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_media_create(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern BernBuffer bern_media_bind(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_release(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_view(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_complete(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_media_event(nint app, byte* bytes, nuint len);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    public static extern void bern_buffer_free(BernBuffer buffer);

    /// Copy and release a buffer returned by the runtime.
    public static byte[] Take(BernBuffer buffer)
    {
        var bytes = buffer.Ptr == 0 || buffer.Len == 0 ? Array.Empty<byte>() : new byte[(int)buffer.Len];
        if (bytes.Length > 0) Marshal.Copy(buffer.Ptr, bytes, 0, bytes.Length);
        if (buffer.Ptr != 0) bern_buffer_free(buffer);
        return bytes;
    }

    /// Call with the bytes pinned for the duration of `body`.
    public static T With<T>(byte[] bytes, Func<nint, nuint, T> body)
    {
        fixed (byte* pointer = bytes) return body((nint)pointer, (nuint)bytes.Length);
    }

    public static void With(byte[] bytes, Action<nint, nuint> body)
    {
        fixed (byte* pointer = bytes) body((nint)pointer, (nuint)bytes.Length);
    }
}

public static unsafe class Host
{
    internal static AppDef App = null!;
    internal static nint Handle;
    internal static DispatcherQueue Dispatcher = null!;
    static Cell<Value>[] engine = Array.Empty<Cell<Value>>();
    static ulong revision;
    static ulong nextToken = 1;
    static readonly Dictionary<ulong, (WeakReference<Win>? Window, Action<Failure?>? Done)> replies = new();
    internal static readonly Dictionary<ulong, Action<byte[]>> Queries = new();
    internal static string EngineId = "";

    internal static Value EngineValue(int field) => field < engine.Length ? engine[field].Get() : Value.Unit;

    static byte[] Copy(byte* bytes, nuint len)
    {
        if (bytes is null || len == 0) return Array.Empty<byte>();
        var copy = new byte[(int)len];
        Marshal.Copy((nint)bytes, copy, 0, copy.Length);
        return copy;
    }

    static readonly HashSet<DispatcherQueueTimer> timers = new();

    /// Run `work` on the UI thread once `delay` has passed. The timer is
    /// kept alive until it fires.
    internal static void After(TimeSpan delay, Action work)
    {
        var timer = Dispatcher.CreateTimer();
        timer.Interval = delay;
        timer.IsRepeating = false;
        timers.Add(timer);
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            timers.Remove(timer);
            try { work(); }
            catch (Exception error) { Reactive.Report(error); }
        };
        timer.Start();
    }

    /// Run `work` on the UI thread after the current call returns.
    internal static void Post(Action work) =>
        Dispatcher.TryEnqueue(DispatcherQueuePriority.Normal, () =>
        {
            try { work(); }
            catch (Exception error) { Reactive.Report(error); }
        });

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnState(nint context, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() => ReceiveState(data));
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnReply(nint context, ulong token, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() => ReceiveReply(token, data));
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnSessionRequest(nint context, ulong request, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() => Session.Handle(request, data));
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnMediaRequest(nint context, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() => Media.Request(data));
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnMediaState(nint context, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() => Media.ReceiveState(data));
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    static void OnQuery(nint context, ulong subscription, byte* bytes, nuint len)
    {
        var data = Copy(bytes, len);
        Post(() =>
        {
            if (Queries.TryGetValue(subscription, out var receive)) receive(data);
        });
    }

    /// Start the engine for this frontend. Returns an error message or null.
    internal static string? Start(AppDef app, string resources)
    {
        App = app;
        Dispatcher = DispatcherQueue.GetForCurrentThread();
        engine = new Cell<Value>[app.State.Length];
        for (var i = 0; i < engine.Length; i++) engine[i] = new Cell<Value>(Value.Unit);
        var callbacks = new BernCallbacks
        {
            Context = 0,
            OnState = &OnState,
            OnReply = &OnReply,
            OnSessionRequest = &OnSessionRequest,
            OnMediaRequest = &OnMediaRequest,
            OnMediaState = &OnMediaState,
            OnQuery = &OnQuery,
        };
        var path = System.Text.Encoding.UTF8.GetBytes(resources);
        var error = new BernBuffer();
        nint handle;
        fixed (byte* pointer = path)
        {
            handle = Native.bern_app_start(app.Fingerprint, pointer, (nuint)path.Length, callbacks, &error);
        }
        if (handle == 0) return System.Text.Encoding.UTF8.GetString(Native.Take(error));
        Handle = handle;
        EngineId = System.Text.Encoding.UTF8.GetString(Native.Take(Native.bern_app_engine_id(handle)));
        ReceiveState(Native.Take(Native.bern_app_snapshot(handle)));
        return null;
    }

    internal static void Stop()
    {
        var handle = Handle;
        Handle = 0;
        if (handle != 0) Native.bern_app_stop(handle);
    }

    static void ReceiveState(byte[] bytes)
    {
        var reader = new Reader(bytes);
        var next = reader.U64();
        if (next < revision) return;
        revision = next;
        var count = reader.U32();
        var changes = new List<(int, Value)>();
        for (var i = 0; i < count; i++)
        {
            var field = (int)reader.U32();
            if (field >= App.State.Length) return;
            changes.Add((field, Codec.Decode(App.State[field], App.Types, reader)));
        }
        Reactive.Batch(() =>
        {
            foreach (var (field, value) in changes) engine[field].Set(value);
        });
        Snapshot.Schedule();
    }

    static void ReceiveReply(ulong token, byte[] bytes)
    {
        if (!replies.Remove(token, out var reply)) return;
        var reader = new Reader(bytes);
        var status = reader.U8();
        reader.U64();
        Failure? failure = status == 0 ? null : new Failure(reader.Text(), reader.Text());
        if (failure is not null && reply.Window is not null && reply.Window.TryGetTarget(out var window))
            window.Fail(failure);
        reply.Done?.Invoke(failure);
        Snapshot.Schedule();
    }

    /// A reply token for a command whose failure shows in `window`.
    internal static ulong Token(Win? window, Action<Failure?>? done)
    {
        var token = nextToken++;
        replies[token] = (window is null ? null : new WeakReference<Win>(window), done);
        return token;
    }

    /// `0, …` or `1, code, message`.
    internal static (Reader Reader, Failure? Failure) Status(byte[] bytes)
    {
        var reader = new Reader(bytes);
        if (bytes.Length == 0) return (reader, new Failure("invalid_reply", "empty reply"));
        return reader.U8() == 0 ? (reader, null) : (reader, new Failure(reader.Text(), reader.Text()));
    }

    /// Dispatch engine command `command` with its arguments. The outcome goes
    /// to `done`; a failure also shows in the context's window.
    public static void Dispatch(Ctx ctx, int command, Value[] args, Action<Failure?>? done = null)
    {
        if (command >= App.Commands.Length) return;
        var spec = App.Commands[command];
        var writer = new Writer();
        for (var i = 0; i < args.Length && i < spec.Params.Length; i++)
            Codec.Encode(args[i], spec.Params[i], App.Types, writer);
        var token = Token(ctx.Win, done);
        Native.With(writer.ToArray(), (pointer, len) =>
            Native.bern_app_dispatch(Handle, (uint)command, (byte*)pointer, len, token));
    }
}
