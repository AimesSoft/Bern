// Bern runtime for WinUI 3: session commands of external entries, run
// through the same closures the user's input runs, in the order of the
// framework session catalog. Generated build input; do not edit.

using System;
using System.Linq;
using Windows.Graphics.Imaging;

namespace Bern;

static class Session
{
    static unsafe void Send(ulong request, byte[] bytes)
    {
        Native.With(bytes, (pointer, len) => Native.bern_session_complete(Host.Handle, request, (byte*)pointer, len));
        Snapshot.Schedule();
    }

    static void Complete(ulong request, uint index, Value result)
    {
        var writer = new Writer();
        writer.U8(0);
        writer.U32(index);
        if (index < Host.App.SessionResults.Length)
            Codec.Encode(result, Host.App.SessionResults[index], Host.App.Types, writer);
        Send(request, writer.ToArray());
    }

    static void Fail(ulong request, string code, string message)
    {
        var writer = new Writer();
        writer.U8(1);
        writer.Text(code);
        writer.Text(message);
        Send(request, writer.ToArray());
    }

    static ControlEntry Find(string target)
    {
        var matches = Controls.All.Where(entry => entry.Target == target).ToList();
        if (matches.Count == 0) throw new BernException("target_not_found", $"no mounted control `{target}`; see ui.inspect");
        if (matches.Count > 1) throw new BernException("target_ambiguous", $"`{target}` is mounted in several places");
        var entry = matches[0];
        if (entry.Window.TryGetTarget(out var win) && win.IsBlocked)
            throw new BernException("target_blocked", $"`{target}` is blocked by the close confirmation of `{win.Id}`; decide it with window.confirm_close");
        if (entry.Enabled is { } enabled && !Reactive.Untrack(enabled))
            throw new BernException("target_unavailable", $"`{target}` is disabled");
        return entry;
    }

    static T Need<T>(ControlEntry entry, T? action, string name) where T : class =>
        action ?? throw new BernException("target_unavailable", $"`{entry.Target}` ({entry.Role}) does not support {name}");

    static Value Inspect()
    {
        static Value Optional(string? text) => text is null ? Value.None : Value.Some(Value.Text(text));
        return Value.List(Controls.All.Select(entry =>
        {
            entry.Window.TryGetTarget(out var win);
            var enabled = entry.Enabled is null || Reactive.Untrack(entry.Enabled);
            var label = entry.Label is null ? "" : Reactive.Untrack(entry.Label);
            var value = entry.Actions.Value is null ? null : Reactive.Untrack(entry.Actions.Value);
            var reason = win?.IsBlocked == true ? "blocked_by_modal" : enabled ? null : "disabled";
            return Value.Record(
                Value.Text(entry.Target), Value.Text(win?.Id ?? ""), Value.Text(entry.Role), Value.Text(label),
                Value.Bool(enabled), Optional(value), Optional(reason));
        }));
    }

    /// Handle one session request from the session runtime.
    internal static void Handle(ulong request, byte[] bytes)
    {
        var reader = new Reader(bytes);
        uint index;
        try { index = reader.U32(); }
        catch (DecodeException error) { Fail(request, "invalid_request", error.Message); return; }
        try { Run(request, index, reader); }
        catch (BernException error) { Fail(request, error.Code, error.Message); }
        catch (DecodeException error) { Fail(request, "invalid_request", error.Message); }
    }

    static void Presentation(ulong request, uint index, Win win, Action<Win, Action<Failure?>> perform) =>
        perform(win, failure =>
        {
            if (failure is null) Complete(request, index, Value.Unit);
            else Fail(request, failure.Code, failure.Message);
        });

    static void Run(ulong request, uint index, Reader reader)
    {
        void Ok(Value result) => Complete(request, index, result);
        switch (index)
        {
            case 0: Ok(Inspect()); break;
            case 1:
                {
                    var entry = Find(reader.Text());
                    Need(entry, entry.Actions.Activate, "activate")();
                    Ok(Value.Unit);
                    break;
                }
            case 2:
                {
                    var entry = Find(reader.Text());
                    var text = reader.Text();
                    Need(entry, entry.Actions.Edit, "edit")(text);
                    Ok(Value.Unit);
                    break;
                }
            case 3:
                {
                    var entry = Find(reader.Text());
                    Need(entry, entry.Actions.Submit, "submit")();
                    Ok(Value.Unit);
                    break;
                }
            case 4:
                {
                    var entry = Find(reader.Text());
                    var on = reader.U8() != 0;
                    Need(entry, entry.Actions.SetBool, "set_bool")(on);
                    Ok(Value.Unit);
                    break;
                }
            case 5:
                {
                    var entry = Find(reader.Text());
                    var number = reader.F64();
                    Need(entry, entry.Actions.SetNumber, "set_number")(number);
                    Ok(Value.Unit);
                    break;
                }
            case 6:
                {
                    var entry = Find(reader.Text());
                    var option = reader.Text();
                    if (!Need(entry, entry.Actions.Select, "select")(option))
                        throw new BernException("invalid_params", $"`{entry.Target}` has no option `{option}`");
                    Ok(Value.Unit);
                    break;
                }
            case 7:
                {
                    var entry = Find(reader.Text());
                    if (!Need(entry, entry.Actions.Back, "back")())
                        throw new BernException("target_unavailable", "nothing to go back to");
                    Ok(Value.Unit);
                    break;
                }
            case 8: Ok(Value.Text(WindowManager.Open(reader.Text()))); break;
            case 9: Ok(WindowManager.RequestClose(WindowManager.Named(reader.Text()), close: true).ToValue()); break;
            case 10:
                WindowManager.Show(WindowManager.Named(reader.Text()));
                Ok(Value.Unit);
                break;
            case 11:
                {
                    var win = WindowManager.Named(reader.Text());
                    var confirmation = reader.Text();
                    var decision = reader.U32();
                    if (decision > 2) throw new BernException("invalid_params", "unknown close decision");
                    WindowManager.ConfirmClose(win, confirmation, decision, outcome =>
                    {
                        if (outcome.Failure is { } failure) Fail(request, failure.Code, failure.Message);
                        else Complete(request, index, outcome.Result!.ToValue());
                    });
                    break;
                }
            case 12: throw new BernException("unknown_command", "window.list is answered by the runtime");
            case 13: Presentation(request, index, WindowManager.Named(reader.Text()), WindowManager.Minimize); break;
            case 14: Presentation(request, index, WindowManager.Named(reader.Text()), WindowManager.Restore); break;
            case 15: Presentation(request, index, WindowManager.Named(reader.Text()), WindowManager.Maximize); break;
            case 16:
                {
                    var win = WindowManager.Named(reader.Text());
                    var (x, y) = (reader.F64(), reader.F64());
                    Presentation(request, index, win, (w, done) => WindowManager.Move(w, x, y, done));
                    break;
                }
            case 17:
                {
                    var win = WindowManager.Named(reader.Text());
                    var (width, height) = (reader.F64(), reader.F64());
                    Presentation(request, index, win, (w, done) => WindowManager.Resize(w, width, height, done));
                    break;
                }
            case 18: Ok(Value.Text(Text.PeekLanguage)); break;
            case 19:
                {
                    var language = reader.Text();
                    if (!Text.SetLanguage(language))
                        throw new BernException("unsupported_language", $"`{language}` is not supported; supported: {string.Join(", ", Text.Supported)}");
                    Ok(Value.Unit);
                    break;
                }
            default: throw new BernException("unknown_command", $"unknown session command {index}");
        }
    }
}

/// Development aids, off by default: `BERN_APPEARANCE=light|dark`
/// overrides the system appearance, and `BERN_SNAPSHOT_DIR` makes the
/// application render each window's content to `<dir>/<window>.png` after
/// it opens and after every change, for checks without screen access.
static class Snapshot
{
    static bool queued;

    static string? Directory => Environment.GetEnvironmentVariable("BERN_SNAPSHOT_DIR");

    internal static void ApplyAppearance(Microsoft.UI.Xaml.Application application)
    {
        switch (Environment.GetEnvironmentVariable("BERN_APPEARANCE"))
        {
            case "light": application.RequestedTheme = Microsoft.UI.Xaml.ApplicationTheme.Light; break;
            case "dark": application.RequestedTheme = Microsoft.UI.Xaml.ApplicationTheme.Dark; break;
        }
    }

    internal static void Schedule()
    {
        if (Directory is null || queued) return;
        queued = true;
        Host.After(TimeSpan.FromMilliseconds(700), () =>
        {
            queued = false;
            CaptureAll();
        });
    }

    internal static void Watch(Win win)
    {
        if (Directory is null) return;
        win.Window.Activated += (_, _) => Schedule();
        Schedule();
    }

    static async void CaptureAll()
    {
        if (Directory is not { } directory) return;
        System.IO.Directory.CreateDirectory(directory);
        foreach (var win in WindowManager.All.ToList())
        {
            if (win.Window.Content is not Microsoft.UI.Xaml.UIElement content) continue;
            var name = win.Id.Replace('#', '-');
            var dpi = Interop.DpiForWindow(win.Window);
            try
            {
                await Save(content, System.IO.Path.Combine(directory, name + ".png"), dpi);
                // Dialogs and menus live in popups above the content.
                var index = 0;
                if (content.XamlRoot is { } root)
                    foreach (var popup in Microsoft.UI.Xaml.Media.VisualTreeHelper.GetOpenPopupsForXamlRoot(root))
                        if (popup.Child is { } child)
                            await Save(child, System.IO.Path.Combine(directory, $"{name}-popup{++index}.png"), dpi);
            }
            catch (Exception error)
            {
                Console.Error.WriteLine($"bern: snapshot of {win.Id}: {error.Message}");
            }
        }
    }

    static async System.Threading.Tasks.Task Save(Microsoft.UI.Xaml.UIElement element, string path, uint dpi)
    {
        var bitmap = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
        await bitmap.RenderAsync(element);
        if (bitmap.PixelWidth == 0 || bitmap.PixelHeight == 0) return;
        var pixels = System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions.ToArray(await bitmap.GetPixelsAsync());
        // The backdrop is not part of the rendering: put the content on the
        // theme's base color, as the window shows it.
        var dark = element is Microsoft.UI.Xaml.FrameworkElement { ActualTheme: Microsoft.UI.Xaml.ElementTheme.Dark };
        byte backdrop = dark ? (byte)0x20 : (byte)0xF3;
        for (var i = 0; i + 3 < pixels.Length; i += 4)
        {
            var rest = 255 - pixels[i + 3];
            for (var c = 0; c < 3; c++) pixels[i + c] = (byte)Math.Min(255, pixels[i + c] + backdrop * rest / 255);
            pixels[i + 3] = 255;
        }
        using var stream = new global::Windows.Storage.Streams.InMemoryRandomAccessStream();
        var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied,
            (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, dpi, dpi, pixels);
        await encoder.FlushAsync();
        using var file = System.IO.File.Create(path);
        stream.Seek(0);
        await System.IO.WindowsRuntimeStreamExtensions.AsStreamForRead(stream).CopyToAsync(file);
    }
}
