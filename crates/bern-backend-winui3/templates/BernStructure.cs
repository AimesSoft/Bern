// Bern runtime for WinUI 3: conditional and repeated content, and the
// containers that navigate: the navigation view, push navigation, tabs and
// adaptive layouts. Generated build input; do not edit.

using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Bern;

/// Content that a builder replaces: the current scope and region.
sealed class Slot
{
    readonly Parent region;
    Scope? owner;

    public Slot(Parent region)
    {
        this.region = region;
        Reactive.OnCleanup(End);
    }

    void End()
    {
        var current = owner;
        owner = null;
        current?.Dispose();
    }

    /// Replace the content with what `build` makes, outside any effect.
    public void Fill(Action<Parent> build) => Reactive.Untrack(() =>
    {
        End();
        region.Clear();
        var next = new Scope();
        next.Run(() => build(region));
        owner = next;
    });

    public void Clear() => Reactive.Untrack(() =>
    {
        End();
        region.Clear();
    });
}

public static partial class B
{
    /// Conditional content: `branch` picks an arm, or none. Only a change of
    /// arm rebuilds; the arm's own bindings update in place.
    public static void When(Parent p, Ctx ctx, Branch branch, params Build[] arms)
    {
        var slot = new Slot(p.Sub());
        var started = false;
        int? current = null;
        Reactive.Effect(() =>
        {
            var next = branch(ctx);
            if (started && current == next) return;
            started = true;
            current = next;
            if (next is { } index && index < arms.Length)
            {
                var arm = arms[index];
                slot.Fill(region => arm(region, ctx));
            }
            else slot.Clear();
        });
    }

    /// An arm builder that runs in a fixed context.
    static Build In(Build build, Ctx ctx) => (p, _) => build(p, ctx);

    /// `if let Some(var) = value { then } else { otherwise }`.
    public static void IfLet(Parent p, Ctx ctx, string variable, Get value, Build then, Build? otherwise = null)
    {
        var inner = ctx.WithLocal(variable, () => R.Some(value(ctx)) ?? Value.Unit);
        var arms = otherwise is null ? new[] { In(then, inner) } : new[] { In(then, inner), otherwise };
        var hasElse = otherwise is not null;
        When(p, ctx, c => value(c) is OptionalValue { Inner: not null } ? 0 : hasElse ? 1 : (int?)null, arms);
    }

    /// One arm of a `match`: the variant (null for `_`), the names its
    /// payload fields bind, and its content.
    public sealed record MatchArm(uint? Variant, (string Name, int Field)[] Bindings, Build Build);

    public static MatchArm Arm(uint? variant, Build build, params (string Name, int Field)[] bindings) =>
        new(variant, bindings, build);

    public static void Match(Parent p, Ctx ctx, Get value, params MatchArm[] arms)
    {
        var variants = arms.Select(arm => arm.Variant).ToArray();
        var builders = arms.Select(arm =>
        {
            var inner = ctx;
            foreach (var (name, field) in arm.Bindings)
                inner = inner.WithLocal(name, () => R.Payload(value(ctx), field));
            return In(arm.Build, inner);
        }).ToArray();
        When(p, ctx, c =>
        {
            var tag = R.Tag(value(c));
            var index = Array.FindIndex(variants, variant => variant is not null && variant == tag);
            if (index < 0) index = Array.FindIndex(variants, variant => variant is null);
            return index < 0 ? null : (int?)index;
        }, builders);
    }

    sealed class Row(string key, Cell<Value> value, Scope owner, Parent region)
    {
        public readonly string Key = key;
        public readonly Cell<Value> Value = value;
        public readonly Scope Owner = owner;
        public readonly Parent Region = region;
    }

    /// Keyed rows: a row keeps its elements, controls and state while its
    /// key stays; its value is a cell the row's bindings read.
    public static void Each(Parent p, Ctx ctx, Get items, int key, string variable, Build row)
    {
        var region = p.Sub();
        var rows = new List<Row>();
        Reactive.OnCleanup(() =>
        {
            foreach (var existing in rows) existing.Owner.Dispose();
            rows.Clear();
        });
        Reactive.Effect(() =>
        {
            var list = R.AsList(items(ctx));
            Reactive.Untrack(() =>
            {
                var orderBefore = rows.Select(existing => existing.Key).ToList();
                var old = new Dictionary<string, Row>();
                foreach (var existing in rows) old[existing.Key] = existing;
                var next = new List<Row>();
                foreach (var item in list)
                {
                    var id = R.KeyText(R.At(item, key));
                    if (old.Remove(id, out var kept))
                    {
                        kept.Value.Set(item);
                        next.Add(kept);
                        continue;
                    }
                    var value = new Cell<Value>(item);
                    var owner = new Scope();
                    var child = region.Sub();
                    var rowCtx = ctx.WithKey(() => id).WithLocal(variable, value.Get);
                    owner.Run(() => row(child, rowCtx));
                    next.Add(new Row(id, value, owner, child));
                }
                foreach (var stale in old.Values)
                {
                    stale.Owner.Dispose();
                    region.Remove(stale.Region);
                }
                // New rows were added at the end; reorder only when the kept
                // rows changed order.
                var keptKeys = orderBefore.Where(k => next.Any(r => r.Key == k)).ToList();
                var inOrder = next.Take(keptKeys.Count).Select(r => r.Key).SequenceEqual(keptKeys);
                if (!inOrder) region.Reorder(next.Select(r => r.Region).ToList());
                rows.Clear();
                rows.AddRange(next);
            });
        });
    }

    /// A list of rows; inside a form section the rows join the section.
    public static void List(Parent p, Ctx ctx, Build children)
    {
        if (p.IsList)
        {
            children(p, ctx);
            return;
        }
        var rows = new StackPanel { Spacing = 4 };
        children(Parent.OfList(rows), ctx);
        p.Add(rows);
    }

    /// A destination in the sidebar.
    public sealed record SidebarItem(Get Value, string Name, Str? Label, string? Icon);

    public sealed record SidebarSection(Str? Title, SidebarItem[] Items);

    /// The search field of a sidebar.
    public sealed record SidebarSearch(string Id, Field Field, Str? Prompt, Act? Submit);

    /// The navigation view of Windows applications: a pane of destinations
    /// with optional search, and a detail page whose header shows the
    /// selected destination, the detail's tools and back. The pane collapses
    /// as the window narrows.
    public static void SplitView(Parent p, Ctx ctx, string? id, Field selection, SidebarSearch? search,
        SidebarSection[] sections, Build detail)
    {
        ctx.Page?.HideHeader();
        var view = new NavigationView
        {
            IsBackButtonVisible = NavigationViewBackButtonVisible.Collapsed,
            IsSettingsVisible = false,
            PaneDisplayMode = NavigationViewPaneDisplayMode.Auto,
            // The pane stays open at the default window width.
            ExpandedModeThresholdWidth = 800,
            IsTitleBarAutoPaddingEnabled = false,
            AlwaysShowHeader = false,
        };
        FlexPanel.SetFill(view, true);
        var entries = new List<(Value Value, string Name, Func<string> Label, NavigationViewItem Item)>();
        foreach (var section in sections)
        {
            if (section.Title is { } title)
            {
                var heading = new NavigationViewItemHeader();
                Reactive.Effect(() => heading.Content = title(ctx));
                view.MenuItems.Add(heading);
            }
            foreach (var entry in section.Items)
            {
                var value = Reactive.Untrack(() => entry.Value(ctx));
                Func<string> label = entry.Label is { } text ? () => text(ctx) : () => "";
                var item = new NavigationViewItem();
                if (entry.Icon is { } icon) item.Icon = Icons.Icon(icon);
                Reactive.Effect(() =>
                {
                    var name = label();
                    item.Content = name;
                    AutomationProperties.SetName(item, name);
                });
                view.MenuItems.Add(item);
                entries.Add((value, entry.Name, label, item));
            }
        }
        if (search is not null)
            view.AutoSuggestBox = SearchBox(ctx, search.Id, search.Field, search.Prompt, search.Submit);

        var updating = false;
        Reactive.Effect(() =>
        {
            var value = ctx.Read(selection);
            var item = entries.FirstOrDefault(entry => entry.Value.Equals(value)).Item;
            if (!ReferenceEquals(view.SelectedItem, item))
            {
                updating = true;
                view.SelectedItem = item;
                updating = false;
            }
        });
        view.SelectionChanged += (_, args) =>
        {
            if (updating || args.SelectedItem is not NavigationViewItem item) return;
            var entry = entries.FirstOrDefault(candidate => ReferenceEquals(candidate.Item, item));
            if (entry.Item is not null) ctx.Write(selection, entry.Value);
        };

        Func<string> selectedLabel = () =>
        {
            var value = ctx.Read(selection);
            var entry = entries.FirstOrDefault(candidate => candidate.Value.Equals(value));
            return entry.Label?.Invoke() ?? "";
        };
        var page = new Page();
        var header = WindowManager.Header(page, selectedLabel, showTitle: true);
        header.Margin = new Thickness(24, 8, 24, 8);
        var body = new Grid();
        body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        body.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        body.Children.Add(header);
        var content = new FlexPanel();
        Grid.SetRow(content, 1);
        body.Children.Add(content);
        view.Content = body;
        detail(Parent.Of(content), ctx.WithPage(page));

        if (id is not null)
            Controls.Register(ctx, id, "sidebar", selectedLabel, null, new Actions
            {
                Select = name =>
                {
                    var entry = entries.FirstOrDefault(candidate => candidate.Name == name);
                    if (entry.Item is null) return false;
                    ctx.Write(selection, entry.Value);
                    return true;
                },
                Value = () =>
                {
                    var value = ctx.Peek(selection);
                    return entries.FirstOrDefault(candidate => candidate.Value.Equals(value)).Name;
                },
            });
        p.Add(view);
    }

    /// Push navigation over a typed route path. The root content stays
    /// mounted under pushed destinations, so its subscriptions, players and
    /// scroll position survive going back; the top route's destination is
    /// built on top, and the page header offers back.
    public static void Navigation(Parent p, Ctx ctx, string? id, Field path, Build content, string variable, Build destination)
    {
        var stack = new Grid();
        FlexPanel.SetFill(stack, true);
        var root = new FlexPanel();
        var top = new FlexPanel { Visibility = Visibility.Collapsed };
        stack.Children.Add(root);
        stack.Children.Add(top);
        var inner = ctx.WithNav(path);
        // The root's tools show only while the root is.
        var rootShown = new Cell<bool>(true);
        content(Parent.Of(root), inner.WithShown(rootShown));

        bool Back()
        {
            var routes = R.AsList(ctx.Peek(path));
            if (routes.Length == 0) return false;
            ctx.Write(path, Value.List(routes[..^1]));
            return true;
        }
        foreach (var (key, modifiers) in new[]
        {
            (global::Windows.System.VirtualKey.Escape, global::Windows.System.VirtualKeyModifiers.None),
            (global::Windows.System.VirtualKey.Left, global::Windows.System.VirtualKeyModifiers.Menu),
        })
        {
            var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
            accelerator.Invoked += (_, args) => args.Handled = Back();
            top.KeyboardAccelerators.Add(accelerator);
        }
        top.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;

        var slot = new Slot(Parent.Of(top));
        var page = ctx.Page;
        Reactive.Effect(() =>
        {
            var routes = R.AsList(ctx.Read(path));
            rootShown.Set(routes.Length == 0);
            if (routes.Length == 0)
            {
                root.Visibility = Visibility.Visible;
                top.Visibility = Visibility.Collapsed;
                slot.Clear();
                return;
            }
            var route = routes[^1];
            var routeCtx = inner.WithLocal(variable, () => route);
            slot.Fill(region =>
            {
                page?.PushBack(() => Back());
                destination(region, routeCtx);
            });
            root.Visibility = Visibility.Collapsed;
            top.Visibility = Visibility.Visible;
        });
        if (id is not null)
            Controls.Register(ctx, id, "navigation", () => "", null, new Actions
            {
                Back = Back,
                Value = () => R.AsList(ctx.Peek(path)).Length.ToString(System.Globalization.CultureInfo.InvariantCulture),
            });
        p.Add(stack);
    }

    /// A row that pushes its route on the enclosing navigation.
    public static void NavLink(Parent p, Ctx ctx, string id, Get route, Str label, Build children)
    {
        var content = new FlexPanel(Orientation.Horizontal) { Spacing = 8 };
        children(Parent.Aligned(content, null, VerticalAlignment.Center), ctx);
        void Activate()
        {
            if (ctx.Nav is not { } path) return;
            var routes = R.AsList(ctx.Peek(path)).ToList();
            routes.Add(route(ctx));
            ctx.Write(path, Value.List(routes));
        }
        var inside = new Grid { ColumnSpacing = 12 };
        inside.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        inside.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        inside.Children.Add(content);
        var chevron = Icons.Icon("chevron.right", 12);
        chevron.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(chevron, 1);
        inside.Children.Add(chevron);
        var button = new Button
        {
            Content = inside,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        if (p.IsList)
        {
            button.Padding = new Thickness(16, 12, 16, 12);
            button.MinHeight = 56;
        }
        else Styles.MakeSubtle(button);
        Reactive.Effect(() => AutomationProperties.SetName(button, label(ctx)));
        button.Click += (_, _) => Activate();
        p.Add(button);
        if (ctx.Nav is not null)
            Controls.Register(ctx, id, "link", () => label(ctx), null, new Actions { Activate = Activate });
    }

    /// A tab of `Tabs`.
    public sealed record TabSpec(Get Value, string Name, Str Label, string? Icon, Build Build);

    /// Tabs as a selector bar in the page header, as Windows Settings pages
    /// show them. Only the selected tab is built.
    public static void Tabs(Parent p, Ctx ctx, string? id, Field selection, params TabSpec[] tabs)
    {
        var bar = new SelectorBar();
        var values = tabs.Select(tab => Reactive.Untrack(() => tab.Value(ctx))).ToArray();
        var items = new List<SelectorBarItem>();
        foreach (var tab in tabs)
        {
            var item = new SelectorBarItem();
            if (tab.Icon is { } icon) item.Icon = Icons.Icon(icon);
            Reactive.Effect(() => item.Text = tab.Label(ctx));
            bar.Items.Add(item);
            items.Add(item);
        }
        var holder = new FlexPanel();
        FlexPanel.SetFill(holder, true);
        var slot = new Slot(Parent.Of(holder));
        if (ctx.Page is { } page) page.PushTitleWidget(bar);
        else p.Add(bar);
        int? shown = null;
        var updating = false;
        Reactive.Effect(() =>
        {
            var value = ctx.Read(selection);
            var index = Array.FindIndex(values, candidate => candidate.Equals(value));
            if (index < 0 || shown == index) return;
            shown = index;
            var build = tabs[index].Build;
            slot.Fill(region => build(region, ctx));
            updating = true;
            bar.SelectedItem = items[index];
            updating = false;
        });
        bar.SelectionChanged += (_, _) =>
        {
            if (updating || bar.SelectedItem is not SelectorBarItem selected) return;
            var index = items.IndexOf(selected);
            if (index >= 0) ctx.Write(selection, values[index]);
        };
        if (id is not null)
            Controls.Register(ctx, id, "tabs", () =>
            {
                var value = ctx.Read(selection);
                var index = Array.FindIndex(values, candidate => candidate.Equals(value));
                return index < 0 ? "" : tabs[index].Label(ctx);
            }, null, new Actions
            {
                Select = name =>
                {
                    var index = Array.FindIndex(tabs, tab => tab.Name == name);
                    if (index < 0) return false;
                    ctx.Write(selection, values[index]);
                    return true;
                },
                Value = () =>
                {
                    var value = ctx.Peek(selection);
                    var index = Array.FindIndex(values, candidate => candidate.Equals(value));
                    return index < 0 ? null : tabs[index].Name;
                },
            });
        p.Add(holder);
    }

    /// Chooses one layout by the width available to it and the breakpoint
    /// from `[layouts]`. Engine state and drafts live outside the element
    /// tree, so switching keeps them; controls with the same id in both
    /// layouts are the same control.
    public static void Adaptive(Parent p, Ctx ctx, string? id, Build? compact, Build? expanded)
    {
        var holder = new FlexPanel { HorizontalAlignment = HorizontalAlignment.Stretch };
        var size = new Cell<string>("expanded");
        var below = Host.App.CompactBelow ?? 0;
        holder.SizeChanged += (_, args) =>
        {
            // A hidden layout has no width to judge by; it keeps its size.
            if (args.NewSize.Width > 0) size.Set(args.NewSize.Width < below ? "compact" : "expanded");
        };
        var region = p.IsList ? Parent.OfList(holder) : Parent.Of(holder);
        if (p.IsList) holder.Spacing = 4;
        var slot = new Slot(region);
        Reactive.Effect(() =>
        {
            var build = size.Get() == "compact" ? compact ?? expanded : expanded ?? compact;
            if (build is null) slot.Clear();
            else slot.Fill(target => build(target, ctx));
        });
        if (id is not null)
            Controls.Register(ctx, id, "adaptive", () => "", null, new Actions { Value = () => size.Peek() });
        p.Add(holder);
    }
}
