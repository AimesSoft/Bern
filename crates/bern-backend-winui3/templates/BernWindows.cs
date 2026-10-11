// Bern runtime for WinUI 3: native windows, their drafts, page headers,
// the close protocol, presentation facts and the application's lifetime.
// Generated build input; do not edit.

using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.XamlTypeInfo;
using Windows.Graphics;

namespace Bern;

/// A close confirmation waiting for a decision.
sealed record PendingClose(string Id, string Message, bool CanSave, ulong DraftRevision);

/// Outcome of a close request or decision, as the session catalog's
/// `CloseResult` describes it.
sealed record CloseResult(uint Outcome, string Window, string? Confirmation, ulong DraftRevision,
    uint[] Decisions, string? Message, string? Reason)
{
    public const uint Closed = 0, Rejected = 1, ConfirmationRequired = 2;

    public Value ToValue()
    {
        static Value Text(string? text) => text is null ? Value.None : Value.Some(Value.Text(text));
        return Value.Record(
            Value.Enum(Outcome), Value.Text(Window), Text(Confirmation), Value.Int((long)DraftRevision),
            Value.List(Decisions.Select(decision => Value.Enum(decision))), Text(Message), Text(Reason));
    }
}

/// What the platform shows for a window, in the session catalog's terms.
readonly record struct Facts(byte Visibility, byte Size, double X, double Y, double Width, double Height,
    bool Focused, uint Actions);

/// A window instance.
public sealed class Win
{
    internal int Kind;
    internal ulong Generation;
    internal string Id = "";
    internal Dictionary<int, Cell<Value>[]> Cells = new();
    internal Window Window = null!;
    internal AppWindow AppWindow = null!;
    internal InfoBar Failures = null!;
    internal readonly Cell<PendingClose?> Pending = new(null);
    internal bool AllowClose;
    internal readonly Scope Scope = new();
    internal bool Command;
    internal readonly List<(Func<Facts, bool> Satisfied, Action<Failure?> Complete)> Waits = new();
    internal ContentDialog? Dialog;
    internal string? DialogConfirmation;
    bool reportQueued;
    internal bool Closed;
    internal bool Focused;
    int failuresShown;

    internal WindowDef Def => Host.App.Windows[Kind];

    internal Cell<Value> Cell(Field field) =>
        Cells.TryGetValue(field.Session, out var cells) && field.FieldIndex < cells.Length
            ? cells[field.FieldIndex]
            : new Cell<Value>(Value.Unit);

    /// Write a draft field: the session runtime first, then the cell.
    internal unsafe bool Write(Field field, Value value)
    {
        var session = Host.App.Sessions[field.Session];
        var bytes = Codec.Bytes(value, session.Fields[field.FieldIndex].Type, Host.App.Types);
        var reply = Native.With(bytes, (pointer, len) => Native.Take(
            Native.bern_session_set(Host.Handle, (uint)Kind, Generation, (uint)field.Session, (uint)field.FieldIndex, (byte*)pointer, len)));
        var (_, failure) = Host.Status(reply);
        if (failure is not null)
        {
            Console.Error.WriteLine($"bern: {failure.Code}: {failure.Message}");
            return false;
        }
        Cell(field).Set(value);
        Snapshot.Schedule();
        return true;
    }

    /// Show a failed command to the user.
    internal void Fail(Failure failure)
    {
        Failures.Title = Text.Tr("bern.alert.failed");
        Failures.Message = failure.Message;
        Failures.IsOpen = true;
        var shown = ++failuresShown;
        Host.After(TimeSpan.FromSeconds(6), () =>
        {
            if (shown == failuresShown) Failures.IsOpen = false;
        });
    }

    internal bool IsBlocked => Pending.Peek() is not null;

    double Scale => Interop.DpiForWindow(Window) / 96.0;

    internal Facts CurrentFacts()
    {
        var presenter = AppWindow.Presenter as OverlappedPresenter;
        var fullScreen = AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
        byte visibility = !AppWindow.IsVisible ? (byte)2
            : presenter?.State == OverlappedPresenterState.Minimized ? (byte)1 : (byte)0;
        byte size = fullScreen ? (byte)2 : presenter?.State == OverlappedPresenterState.Maximized ? (byte)1 : (byte)0;
        var scale = Scale;
        var position = AppWindow.Position;
        var outer = AppWindow.Size;
        uint actions = (1u << 0) | (1u << 1); // activate, close
        var resizable = presenter?.IsResizable ?? false;
        if (visibility == 0 && (presenter?.IsMinimizable ?? false)) actions |= 1u << 2;
        if (visibility == 1 || size != 0) actions |= 1u << 3;
        if (resizable && size == 0 && visibility == 0 && (presenter?.IsMaximizable ?? false)) actions |= 1u << 4;
        if (size == 0 && visibility == 0) actions |= 1u << 5;
        if (resizable && size == 0 && visibility == 0) actions |= 1u << 6;
        var frame = (position.X / scale, position.Y / scale, outer.Width / scale, outer.Height / scale);
        // A minimized window is parked off screen; its frame is the one it
        // comes back to.
        if (visibility == 1 && lastFrame is { } restored) frame = restored;
        else if (visibility == 0) lastFrame = frame;
        return new Facts(visibility, size, frame.Item1, frame.Item2, frame.Item3, frame.Item4, Focused, actions);
    }

    (double, double, double, double)? lastFrame;

    /// Report the current facts to the session runtime, then finish the
    /// presentation commands they satisfy.
    internal void Report()
    {
        if (Closed || Host.Handle == 0) return;
        var facts = CurrentFacts();
        Native.bern_window_facts(Host.Handle, (uint)Kind, Generation, Command ? (byte)0 : (byte)1,
            facts.Visibility, facts.Size, facts.X, facts.Y, facts.Width, facts.Height,
            facts.Focused ? (byte)1 : (byte)0, facts.Actions);
        var done = Waits.Where(wait => wait.Satisfied(facts)).ToList();
        foreach (var wait in done) Waits.Remove(wait);
        foreach (var wait in done) wait.Complete(null);
        if (Waits.Count == 0) Command = false;
    }

    internal void ScheduleReport()
    {
        if (reportQueued) return;
        reportQueued = true;
        Host.Dispatcher.TryEnqueue(DispatcherQueuePriority.Low, () =>
        {
            reportQueued = false;
            Report();
        });
    }

    /// Perform a presentation change and finish once the facts show it, or
    /// with `ui_timeout` after five seconds.
    internal void Perform(Action change, Func<Facts, bool> satisfied, Action<Failure?> complete)
    {
        Command = true;
        Waits.Add((satisfied, complete));
        change();
        ScheduleReport();
        Host.After(TimeSpan.FromSeconds(5), () =>
        {
            Report();
            var expired = Waits.ToList();
            Waits.Clear();
            foreach (var wait in expired)
                wait.Complete(new Failure("ui_timeout", "the window system did not report the change in time; the outcome is unknown"));
            Command = false;
        });
    }

    internal PointInt32 Physical(double x, double y) => new((int)Math.Round(x * Scale), (int)Math.Round(y * Scale));
    internal SizeInt32 PhysicalSize(double width, double height) =>
        new((int)Math.Round(width * Scale), (int)Math.Round(height * Scale));
}

static class WindowManager
{
    internal static readonly List<Win> All = new();

    internal static Win Named(string id) =>
        All.FirstOrDefault(win => win.Id == id) ?? throw new BernException("target_not_found", $"no open window `{id}`");

    /// A page header: the innermost title or title widget, a back button
    /// while something can go back, and the page's tools.
    internal static Grid Header(Page page, Func<string>? fallback, bool showTitle)
    {
        // No column spacing: a hidden back button must not indent the title.
        var grid = new Grid { Margin = new Thickness(0, 0, 0, 8) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var back = new Button
        {
            Content = new FontIcon { Glyph = "\uE72B", FontSize = 16 },
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
            Padding = new Thickness(10, 8, 10, 8),
        };
        Styles.MakeSubtle(back);
        back.Click += (_, _) => page.Back?.Invoke();
        Grid.SetColumn(back, 0);
        var title = new TextBlock
        {
            Style = (Style)Application.Current.Resources["TitleTextBlockStyle"],
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        var titleHolder = new ContentControl
        {
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(titleHolder, 1);
        Grid.SetColumn(page.Tools, 2);
        page.Tools.VerticalAlignment = VerticalAlignment.Center;
        page.Tools.Margin = new Thickness(8, 0, 0, 0);
        grid.Children.Add(back);
        grid.Children.Add(titleHolder);
        grid.Children.Add(page.Tools);
        Reactive.Effect(() =>
        {
            page.Version.Track();
            var backLabel = Text.Tr("bern.navigation.back");
            ToolTipService.SetToolTip(back, backLabel);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(back, backLabel);
            back.Visibility = page.Back is null ? Visibility.Collapsed : Visibility.Visible;
            if (page.TitleWidget is { } widget)
            {
                if (!ReferenceEquals(titleHolder.Content, widget)) titleHolder.Content = widget;
            }
            else
            {
                title.Text = showTitle ? page.Title ?? fallback?.Invoke() ?? "" : page.Title ?? "";
                if (!ReferenceEquals(titleHolder.Content, title)) titleHolder.Content = title;
            }
            var empty = page.Back is null && page.TitleWidget is null && page.Tools.Children.Count == 0
                && string.IsNullOrEmpty(title.Text);
            grid.Visibility = page.HeaderHidden || empty ? Visibility.Collapsed : Visibility.Visible;
        });
        return grid;
    }

    static unsafe (ulong Generation, string Id, Dictionary<int, Cell<Value>[]> Cells) DecodeInstance(byte[] bytes)
    {
        var (reader, failure) = Host.Status(bytes);
        if (failure is not null) throw new BernException(failure.Code, failure.Message);
        var generation = reader.U64();
        var id = reader.Text();
        var slots = reader.U32();
        var cells = new Dictionary<int, Cell<Value>[]>();
        for (var i = 0; i < slots; i++)
        {
            var session = (int)reader.U32();
            var def = Host.App.Sessions[session];
            cells[session] = def.Fields
                .Select(field => new Cell<Value>(Codec.Decode(field.Type, Host.App.Types, reader)))
                .ToArray();
        }
        return (generation, id, cells);
    }

    /// Open a window of `kind`, or show the one already open. Returns the
    /// instance id.
    internal static string Open(string kindId)
    {
        var kind = Array.FindIndex(Host.App.Windows, window => window.Id == kindId);
        if (kind < 0) throw new BernException("target_not_found", $"no window kind `{kindId}`");
        var existing = All.FirstOrDefault(win => win.Kind == kind);
        if (existing is not null)
        {
            Show(existing);
            return existing.Id;
        }
        var (generation, id, cells) = DecodeInstance(Native.Take(Native.bern_window_opened(Host.Handle, (uint)kind)));
        var def = Host.App.Windows[kind];
        var window = new Window();
        var win = new Win { Kind = kind, Generation = generation, Id = id, Cells = cells, Window = window, AppWindow = window.AppWindow };
        All.Add(win);

        window.SystemBackdrop = new MicaBackdrop();
        window.ExtendsContentIntoTitleBar = true;
        var titleBar = new TitleBar();
        var root = new Grid();
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.Children.Add(titleBar);
        var page = new Page();
        var content = new FlexPanel();
        var body = new Grid { Padding = new Thickness(0) };
        body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        body.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        var failures = new InfoBar { Severity = InfoBarSeverity.Error, IsOpen = false, Margin = new Thickness(12, 0, 12, 12), VerticalAlignment = VerticalAlignment.Bottom };
        win.Failures = failures;
        var host = new Grid();
        Grid.SetRow(host, 1);
        root.Children.Add(host);
        host.Children.Add(body);
        host.Children.Add(failures);
        window.Content = root;
        window.SetTitleBar(titleBar);

        win.Scope.Run(() =>
        {
            Reactive.Effect(() =>
            {
                var title = Text.Tr(def.Title);
                window.Title = title;
                titleBar.Title = title;
            });
            var header = Header(page, null, showTitle: false);
            header.Margin = new Thickness(16, 0, 16, 8);
            body.Children.Add(header);
            var holder = new Grid();
            Grid.SetRow(holder, 1);
            body.Children.Add(holder);
            holder.Children.Add(content);
            var ctx = Ctx.Root(win, page);
            def.Render(Parent.Of(content), ctx);
            Reactive.Effect(() => holder.IsHitTestVisible = win.Pending.Get() is null);
        });

        var presenter = (OverlappedPresenter)win.AppWindow.Presenter;
        if (def.MinWidth is { } minWidth) presenter.PreferredMinimumWidth = (int)Math.Round(minWidth * (Interop.DpiForWindow(win.Window) / 96.0));
        if (def.MinHeight is { } minHeight) presenter.PreferredMinimumHeight = (int)Math.Round(minHeight * (Interop.DpiForWindow(win.Window) / 96.0));
        if (def.Settings)
        {
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
        }
        var width = def.Width ?? (def.Settings ? 640 : 960);
        var height = def.Height ?? (def.Settings ? 560 : 680);
        win.AppWindow.Resize(win.PhysicalSize(width, height));

        win.AppWindow.Closing += (_, args) =>
        {
            if (win.AllowClose) return;
            args.Cancel = true;
            RequestCloseSafely(win);
        };
        window.Closed += (_, _) => Ended(win);
        window.Activated += (_, args) =>
        {
            win.Focused = args.WindowActivationState != WindowActivationState.Deactivated;
            win.ScheduleReport();
        };
        win.AppWindow.Changed += (_, _) => win.ScheduleReport();
        if (def.Settings)
        {
            var escape = new KeyboardAccelerator { Key = global::Windows.System.VirtualKey.Escape };
            escape.Invoked += (_, args) =>
            {
                args.Handled = true;
                RequestCloseSafely(win);
            };
            root.KeyboardAccelerators.Add(escape);
        }
        var closeKey = new KeyboardAccelerator { Key = global::Windows.System.VirtualKey.W, Modifiers = global::Windows.System.VirtualKeyModifiers.Control };
        closeKey.Invoked += (_, args) =>
        {
            args.Handled = true;
            RequestCloseSafely(win);
        };
        root.KeyboardAccelerators.Add(closeKey);
        root.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;

        window.Activate();
        win.ScheduleReport();
        Snapshot.Watch(win);
        return id;
    }

    internal static void Show(Win win)
    {
        if (win.AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized } presenter)
            presenter.Restore();
        win.Window.Activate();
    }

    /// The window instance ended: release its drafts, controls and
    /// subscriptions; application players stay.
    static void Ended(Win win)
    {
        if (win.Closed) return;
        win.Closed = true;
        All.Remove(win);
        win.Scope.Dispose();
        foreach (var wait in win.Waits.ToList())
            wait.Complete(new Failure("stale_window", $"`{win.Id}` closed while the command ran"));
        win.Waits.Clear();
        if (Host.Handle != 0) Native.bern_window_closed(Host.Handle, (uint)win.Kind, win.Generation);
    }

    static CloseCheck? Evaluate(Win win)
    {
        if (win.Def.Close is not { } check) return null;
        var ctx = Ctx.Root(win, null);
        return Reactive.Untrack(() => check(ctx));
    }

    static void RequestCloseSafely(Win win)
    {
        try { RequestClose(win, close: true); }
        catch (BernException error) { win.Fail(new Failure(error.Code, error.Message)); }
    }

    /// Run the window's close check and ask the session runtime whether to
    /// close now or wait for a decision. A pending confirmation is returned
    /// again rather than asked twice.
    internal static CloseResult RequestClose(Win win, bool close)
    {
        var check = Evaluate(win);
        var needs = check?.NeedsConfirmation ?? false;
        var canSave = check?.Save is not null;
        var (reader, failure) = Host.Status(Native.Take(
            Native.bern_window_request_close(Host.Handle, (uint)win.Kind, win.Generation, needs ? (byte)1 : (byte)0, canSave ? (byte)1 : (byte)0)));
        if (failure is not null) throw new BernException(failure.Code, failure.Message);
        if (reader.U8() == 0)
        {
            if (close) FinishClose(win);
            return new CloseResult(CloseResult.Closed, win.Id, null, 0, Array.Empty<uint>(), null, null);
        }
        var pending = new PendingClose(reader.Text(), check?.Message ?? "", canSave, reader.U64());
        win.Pending.Set(pending);
        ShowDialog(win, pending);
        Snapshot.Schedule();
        return new CloseResult(CloseResult.ConfirmationRequired, win.Id, pending.Id, pending.DraftRevision,
            canSave ? new uint[] { 0, 1, 2 } : new uint[] { 1, 2 }, pending.Message, null);
    }

    static void FinishClose(Win win)
    {
        win.AllowClose = true;
        DismissDialog(win);
        Ended(win);
        win.Window.Close();
    }

    static async void ShowDialog(Win win, PendingClose pending)
    {
        // The same confirmation is already on screen.
        if (win.Dialog is not null && win.DialogConfirmation == pending.Id) return;
        DismissDialog(win);
        var dialog = new ContentDialog
        {
            Title = pending.Message,
            Content = Text.Tr(pending.CanSave ? "bern.close.save_detail" : "bern.close.discard_detail"),
            CloseButtonText = Text.Tr("bern.close.cancel"),
            XamlRoot = win.Window.Content.XamlRoot,
        };
        if (pending.CanSave)
        {
            dialog.PrimaryButtonText = Text.Tr("bern.close.save");
            dialog.SecondaryButtonText = Text.Tr("bern.close.dont_save");
            dialog.DefaultButton = ContentDialogButton.Primary;
        }
        else
        {
            dialog.PrimaryButtonText = Text.Tr("bern.close.discard");
            dialog.DefaultButton = ContentDialogButton.Close;
        }
        win.Dialog = dialog;
        win.DialogConfirmation = pending.Id;
        ContentDialogResult result;
        try { result = await dialog.ShowAsync(); }
        catch (Exception) { return; }
        // Answered elsewhere, for example by an external entry.
        if (!ReferenceEquals(win.Dialog, dialog)) return;
        win.Dialog = null;
        win.DialogConfirmation = null;
        uint decision = result switch
        {
            ContentDialogResult.Primary => pending.CanSave ? 0u : 1u,
            ContentDialogResult.Secondary => 1u,
            _ => 2u,
        };
        ConfirmClose(win, pending.Id, decision, _ => { });
    }

    static void DismissDialog(Win win)
    {
        var dialog = win.Dialog;
        win.Dialog = null;
        win.DialogConfirmation = null;
        dialog?.Hide();
    }

    /// Apply a decision from the dialog or from an external entry. A save
    /// that fails keeps the window open.
    internal static unsafe void ConfirmClose(Win win, string confirmation, uint decision, Action<(CloseResult? Result, Failure? Failure)> completion)
    {
        var writer = new Writer();
        writer.Text(confirmation);
        writer.U32(decision);
        var reply = Native.With(writer.ToArray(), (pointer, len) => Native.Take(
            Native.bern_window_confirm_close(Host.Handle, (uint)win.Kind, win.Generation, (byte*)pointer, len)));
        var revision = win.Pending.Peek()?.DraftRevision ?? 0;
        var (reader, failure) = Host.Status(reply);
        if (failure is not null)
        {
            // A stale confirmation no longer describes the window.
            if (failure.Code == "stale_confirmation" && win.Pending.Peek()?.Id == confirmation)
            {
                win.Pending.Set(null);
                DismissDialog(win);
            }
            completion((null, failure));
            return;
        }
        var step = reader.U8();
        win.Pending.Set(null);
        DismissDialog(win);
        Snapshot.Schedule();
        switch (step)
        {
            case 0:
                FinishClose(win);
                completion((new CloseResult(CloseResult.Closed, win.Id, null, revision, Array.Empty<uint>(), null, null), null));
                break;
            case 1:
                if (Evaluate(win)?.Save is not { } save)
                {
                    completion((null, new Failure("invalid_params", "no save command")));
                    return;
                }
                save(Ctx.Root(win, null), saveFailure =>
                {
                    if (saveFailure is null)
                    {
                        FinishClose(win);
                        completion((new CloseResult(CloseResult.Closed, win.Id, null, revision, Array.Empty<uint>(), null, null), null));
                    }
                    else
                    {
                        completion((new CloseResult(CloseResult.Rejected, win.Id, null, revision, Array.Empty<uint>(), null,
                            $"save_failed: {saveFailure.Code}: {saveFailure.Message}"), null));
                    }
                });
                break;
            default:
                completion((new CloseResult(CloseResult.Rejected, win.Id, null, revision, Array.Empty<uint>(), null, "cancelled"), null));
                break;
        }
    }

    internal static void Minimize(Win win, Action<Failure?> done) =>
        win.Perform(() => ((OverlappedPresenter)win.AppWindow.Presenter).Minimize(), facts => facts.Visibility == 1, done);

    internal static void Restore(Win win, Action<Failure?> done)
    {
        var facts = win.CurrentFacts();
        if (facts.Size == 2)
            win.Perform(() => win.AppWindow.SetPresenter(AppWindowPresenterKind.Overlapped), f => f.Size != 2, done);
        else
            win.Perform(() =>
            {
                ((OverlappedPresenter)win.AppWindow.Presenter).Restore();
                win.Window.Activate();
            }, f => f.Visibility == 0 && (facts.Visibility == 1 ? f.Size == facts.Size : f.Size == 0), done);
    }

    internal static void Maximize(Win win, Action<Failure?> done) =>
        win.Perform(() => ((OverlappedPresenter)win.AppWindow.Presenter).Maximize(), facts => facts.Size == 1, done);

    internal static void Move(Win win, double x, double y, Action<Failure?> done) =>
        win.Perform(() => win.AppWindow.Move(win.Physical(x, y)),
            facts => Math.Abs(facts.X - x) <= 1.5 && Math.Abs(facts.Y - y) <= 1.5, done);

    internal static void Resize(Win win, double width, double height, Action<Failure?> done)
    {
        var minimumWidth = win.Def.MinWidth ?? 1;
        var minimumHeight = win.Def.MinHeight ?? 1;
        if (width < minimumWidth || height < minimumHeight)
        {
            done(new Failure("out_of_range", $"`{win.Id}` needs at least {R.Number(minimumWidth)} × {R.Number(minimumHeight)}"));
            return;
        }
        win.Perform(() => win.AppWindow.Resize(win.PhysicalSize(width, height)),
            facts => Math.Abs(facts.Width - width) <= 1.5 && Math.Abs(facts.Height - height) <= 1.5, done);
    }

    /// Open the windows declared `initial: open`.
    internal static void OpenInitial()
    {
        foreach (var def in Host.App.Windows.Where(window => window.InitialOpen))
        {
            try { Open(def.Id); }
            catch (BernException error) { Console.Error.WriteLine($"bern: cannot open `{def.Id}`: {error.Message}"); }
        }
    }

    internal static void ReopenPrimary()
    {
        try { Open(Host.App.Primary); }
        catch (BernException error) { Console.Error.WriteLine($"bern: cannot open `{Host.App.Primary}`: {error.Message}"); }
    }
}

/// A failure raised inside the runtime, with a session catalog error code.
public sealed class BernException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

public static class WindowOps
{
    /// `windows.<kind>.open()` and `activate()` in declarations.
    public static void Open(Ctx ctx, string kind)
    {
        try { WindowManager.Open(kind); }
        catch (BernException error) { ctx.Win.Fail(new Failure(error.Code, error.Message)); }
    }

    /// `windows.<kind>.close()` in declarations: the close protocol, as from
    /// the title bar.
    public static void Close(Ctx ctx, string kind)
    {
        var win = WindowManager.All.FirstOrDefault(candidate => candidate.Def.Id == kind);
        if (win is null) return;
        try { WindowManager.RequestClose(win, close: true); }
        catch (BernException error) { ctx.Win.Fail(new Failure(error.Code, error.Message)); }
    }
}

static class Interop
{
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern uint GetDpiForWindow(nint hwnd);

    internal static uint DpiForWindow(Window window)
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(window);
        var dpi = GetDpiForWindow(hwnd);
        return dpi == 0 ? 96u : dpi;
    }
}

/// The application: XAML resources, the engine, single instance and the
/// initial windows.
public sealed class BernApplication : Application, IXamlMetadataProvider
{
    readonly XamlControlsXamlMetaDataProvider provider = new();
    readonly Func<AppDef> app;

    public BernApplication(Func<AppDef> app) => this.app = app;

    public IXamlType GetXamlType(Type type) => provider.GetXamlType(type);
    public IXamlType GetXamlType(string fullName) => provider.GetXamlType(fullName);
    public XmlnsDefinition[] GetXmlnsDefinitions() => provider.GetXmlnsDefinitions();

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var definition = app();
        Resources.MergedDictionaries.Add(new XamlControlsResources());
        if (definition.BrandSeed is { } brand) ApplyBrand(brand);
        Styles.Install(Resources);
        Snapshot.ApplyAppearance(this);
        Text.Install(definition);
        var resources = System.IO.Path.Combine(AppContext.BaseDirectory, "resources");
        var problem = Host.Start(definition, System.IO.Directory.Exists(resources) ? resources : "");
        if (problem is not null)
        {
            ShowStartupError(problem);
            return;
        }
        Media.AttachExecutor();
        WindowManager.OpenInitial();
    }

    /// The declared brand color as the accent color resources, with the
    /// lighter and darker steps the controls use in each theme.
    void ApplyBrand((byte R, byte G, byte B) brand)
    {
        static byte Mix(byte from, byte to, double amount) => (byte)Math.Round(from + (to - from) * amount);
        global::Windows.UI.Color Toward(byte target, double amount) =>
            ColorHelper.FromArgb(255, Mix(brand.R, target, amount), Mix(brand.G, target, amount), Mix(brand.B, target, amount));
        Resources["SystemAccentColor"] = ColorHelper.FromArgb(255, brand.R, brand.G, brand.B);
        for (var step = 1; step <= 3; step++)
        {
            Resources[$"SystemAccentColorLight{step}"] = Toward(255, 0.2 * step);
            Resources[$"SystemAccentColorDark{step}"] = Toward(0, 0.2 * step);
        }
    }

    static void ShowStartupError(string message)
    {
        var window = new Window { Title = Text.Tr("bern.startup.failed") };
        var panel = new StackPanel { Spacing = 12, Margin = new Thickness(32), VerticalAlignment = VerticalAlignment.Center };
        panel.Children.Add(new FontIcon { Glyph = "\uEA39", FontSize = 40 });
        panel.Children.Add(new TextBlock { Text = Text.Tr("bern.startup.failed"), Style = (Style)Current.Resources["SubtitleTextBlockStyle"], HorizontalAlignment = HorizontalAlignment.Center });
        panel.Children.Add(new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap, HorizontalAlignment = HorizontalAlignment.Center });
        window.Content = panel;
        window.Activate();
    }
}

public static class Launcher
{
    /// Run the application: one instance per user; launching it again shows
    /// the primary window. The engine stops when the last window closes, as
    /// Windows applications do.
    public static void Run(Func<AppDef> app, string bundleId)
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        var instance = Microsoft.Windows.AppLifecycle.AppInstance.FindOrRegisterForKey(bundleId);
        if (!instance.IsCurrent)
        {
            var activation = Microsoft.Windows.AppLifecycle.AppInstance.GetCurrent().GetActivatedEventArgs();
            instance.RedirectActivationToAsync(activation).AsTask().Wait();
            return;
        }
        instance.Activated += (_, _) => Host.Post(WindowManager.ReopenPrimary);
        Application.Start(parameters =>
        {
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new BernApplication(app);
        });
        Media.DetachExecutor();
        Host.Stop();
    }
}
