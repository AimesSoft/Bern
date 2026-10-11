// Bern runtime for WinUI 3: the result of an asynchronous query. The
// subscription belongs to the window instance and ends with this view;
// parameters are sent again only when they change, and only the current
// answer is shown. Generated build input; do not edit.

using System;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bern;

/// A query's state: 0 idle, 1 loading, 2 ready, 3 refreshing, 4 failed.
sealed record QueryState(ulong Version, byte Phase, Value? Data, string? ErrorCode, string? ErrorMessage);

static unsafe class QueryLink
{
    internal static (ulong? Subscription, Failure? Failure) Open(Win win, int query)
    {
        var (reader, failure) = Host.Status(Native.Take(Native.bern_query_open(Host.Handle, (uint)win.Kind, win.Generation, (uint)query)));
        return failure is null ? (reader.U64(), null) : (null, failure);
    }

    internal static byte[]? Request(ulong subscription, byte[] parameters)
    {
        var reply = Native.With(parameters, (pointer, len) =>
            Native.Take(Native.bern_query_request(Host.Handle, subscription, (byte*)pointer, len)));
        return Host.Status(reply).Failure is null ? reply[1..] : null;
    }

    internal static void Close(ulong subscription)
    {
        if (Host.Handle != 0) Native.bern_query_close(Host.Handle, subscription);
    }
}

public static partial class B
{
    static QueryState? ReadQueryState(byte[] bytes, BType result)
    {
        try
        {
            var reader = new Reader(bytes);
            var version = reader.U64();
            var phase = reader.U8();
            Value? data = null;
            string? code = null, message = null;
            if (phase is 2 or 3) data = Codec.Decode(result, Host.App.Types, reader);
            if (phase == 4)
            {
                if (reader.U8() == 1) data = Codec.Decode(result, Host.App.Types, reader);
                code = reader.Text();
                message = reader.Text();
            }
            return new QueryState(version, phase, data, code, message);
        }
        catch (DecodeException)
        {
            return null;
        }
    }

    /// What the template shows of the data: null when the result is empty
    /// and the declaration has an empty state.
    static Value? ShownData(Value data, BType result, bool hasEmpty) => (data, result) switch
    {
        (ListValue { Items.Length: 0 }, ListType) when hasEmpty => null,
        (OptionalValue { Inner: null }, _) => null,
        (OptionalValue { Inner: { } inner }, _) => inner,
        _ => data,
    };

    static int? Count(Value data) => data switch
    {
        ListValue list => list.Items.Length,
        OptionalValue optional => optional.Inner is null ? 0 : 1,
        _ => null,
    };

    public static void Query(Parent p, Ctx ctx, string? id, int query, Get[] parameters, Str? loading, Str? failed,
        Str? empty, string? icon, string variable, Build template)
    {
        if (query >= Host.App.Commands.Length) return;
        var spec = Host.App.Commands[query];
        var result = spec.Result;
        var hasEmpty = empty is not null;
        var state = new Cell<QueryState>(new QueryState(0, 0, null, null, null));
        void Receive(byte[] bytes)
        {
            if (ReadQueryState(bytes, result) is { } next && next.Version >= state.Peek().Version) state.Set(next);
        }
        var (subscription, openFailure) = QueryLink.Open(ctx.Win, query);
        if (openFailure is not null) state.Set(new QueryState(0, 4, null, openFailure.Code, openFailure.Message));
        if (subscription is { } opened)
        {
            Host.Queries[opened] = Receive;
            Reactive.OnCleanup(() =>
            {
                Host.Queries.Remove(opened);
                QueryLink.Close(opened);
            });
        }
        void Request(byte[] bytes)
        {
            if (subscription is not { } current) return;
            if (QueryLink.Request(current, bytes) is { } reply) Receive(reply);
        }
        byte[] Parameters()
        {
            var writer = new Writer();
            for (var i = 0; i < parameters.Length && i < spec.Params.Length; i++)
                Codec.Encode(parameters[i](ctx), spec.Params[i], Host.App.Types, writer);
            return writer.ToArray();
        }
        byte[]? sent = null;
        Reactive.Effect(() =>
        {
            var bytes = Parameters();
            if (sent is not null && sent.AsSpan().SequenceEqual(bytes)) return;
            sent = bytes;
            Reactive.Untrack(() => Request(bytes));
        });
        void Retry()
        {
            var bytes = Reactive.Untrack(Parameters);
            sent = bytes;
            Request(bytes);
        }

        // Which status to show, and whether the template shows data.
        string Key()
        {
            var current = state.Get();
            var visible = current.Data is { } data ? ShownData(data, result, hasEmpty) : null;
            if (current.Phase == 1 || (current.Phase == 0 && current.Data is null)) return "loading";
            if (current.Phase == 4 && current.Data is null) return "failed";
            if (visible is null && hasEmpty) return "empty";
            return "data";
        }
        Func<string> TextOf(Str? text, string fallback) => text is null ? () => Bern.Text.Tr(fallback) : () => text(ctx);
        var loadingText = TextOf(loading, "bern.query.loading");
        var failedText = TextOf(failed, "bern.query.failed");
        var emptyText = TextOf(empty, "");
        var inList = p.IsList;
        void Add(Parent target, FrameworkElement element)
        {
            if (inList) target.Add(Card(element));
            else target.Add(element);
        }
        TextBlock BoundBlock(Func<string> text)
        {
            var block = new TextBlock { TextWrapping = TextWrapping.Wrap, VerticalAlignment = VerticalAlignment.Center };
            BindText(block, text);
            return block;
        }
        Button RetryButton()
        {
            var button = new Button();
            Reactive.Effect(() => button.Content = Bern.Text.Tr("bern.query.retry"));
            button.Click += (_, _) => Retry();
            return button;
        }
        ProgressRing Ring(double size) => new() { IsActive = true, Width = size, Height = size };
        FrameworkElement Status(string glyphName, Func<string> title, UIElement? action)
        {
            var column = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 24, 0, 24) };
            var symbol = Icons.Icon(glyphName, 32);
            symbol.HorizontalAlignment = HorizontalAlignment.Center;
            column.Children.Add(symbol);
            var heading = BoundBlock(title);
            heading.HorizontalAlignment = HorizontalAlignment.Center;
            heading.TextAlignment = TextAlignment.Center;
            if (Styles.Named("BodyStrongTextBlockStyle") is { } style) heading.Style = style;
            column.Children.Add(heading);
            if (action is FrameworkElement framework)
            {
                framework.HorizontalAlignment = HorizontalAlignment.Center;
                column.Children.Add(framework);
            }
            return column;
        }
        Build[] arms =
        {
            (target, _) =>
            {
                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 18, 0, 18) };
                line.Children.Add(Ring(20));
                var label = BoundBlock(loadingText);
                Secondary(label);
                line.Children.Add(label);
                Add(target, line);
            },
            (target, _) => Add(target, Status("exclamationmark.triangle", failedText, RetryButton())),
            (target, _) => Add(target, Status(icon ?? "tray", emptyText, null)),
            (target, _) =>
            {
                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                line.Children.Add(Ring(16));
                var label = BoundBlock(loadingText);
                Secondary(label);
                line.Children.Add(label);
                Add(target, line);
            },
            (target, _) =>
            {
                var line = new Grid { ColumnSpacing = 8 };
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                var warning = Icons.Icon("exclamationmark.triangle");
                warning.VerticalAlignment = VerticalAlignment.Center;
                line.Children.Add(warning);
                var label = BoundBlock(failedText);
                Grid.SetColumn(label, 1);
                line.Children.Add(label);
                var retry = RetryButton();
                Grid.SetColumn(retry, 2);
                line.Children.Add(retry);
                Add(target, line);
            },
        };
        When(p, ctx, _ => (Key(), state.Get().Phase) switch
        {
            ("loading", _) => 0,
            ("failed", _) => 1,
            ("empty", _) => 2,
            ("data", 3) => 3,
            ("data", 4) => 4,
            _ => (int?)null,
        }, arms);

        var inner = ctx.WithLocal(variable, () =>
            state.Get().Data is { } data && ShownData(data, result, hasEmpty) is { } shown ? shown : Value.Unit);
        When(p, ctx, _ => Key() == "data" ? 0 : (int?)null, (target, _) => template(target, inner));

        if (id is not null)
            Controls.Register(ctx, id, "query", () => "", null, new Actions
            {
                Activate = Retry,
                Value = () =>
                {
                    var current = state.Peek();
                    var name = new[] { "idle", "loading", "ready", "refreshing", "failed" }[Math.Min((int)current.Phase, 4)];
                    return current.Data is { } data && Count(data) is { } n
                        ? $"{name} v{current.Version} ({n})"
                        : $"{name} v{current.Version}";
                },
            });
    }
}
