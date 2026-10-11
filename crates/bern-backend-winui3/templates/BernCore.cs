// Bern runtime for WinUI 3: the application description, contexts,
// parents, control registry and localized text. Generated build input; do
// not edit.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bern;

public delegate Value Get(Ctx c);
public delegate string Str(Ctx c);
public delegate void Act(Ctx c);
public delegate void Build(Parent p, Ctx c);
public delegate int? Branch(Ctx c);

/// A failed command, shown to the user and reported to callers.
public sealed record Failure(string Code, string Message);

/// A catalog command: parameter types and result type.
public sealed record CommandDef(string Name, BType[] Params, BType Result, bool IsQuery);

/// Typed drafts of a UI session, one instance per hosting window.
public sealed record SessionDef(string Name, (string Name, BType Type)[] Fields);

/// What a window's close check found.
public sealed class CloseCheck
{
    public bool NeedsConfirmation { get; init; }
    public string Message { get; init; } = "";
    /// Runs the declared save command; closing waits for its result.
    public Action<Ctx, Action<Failure?>>? Save { get; init; }
}

/// A declared window kind.
public sealed class WindowDef
{
    public required string Id { get; init; }
    /// Text key of the title.
    public required string Title { get; init; }
    public bool Settings { get; init; }
    public double? Width { get; init; }
    public double? Height { get; init; }
    public double? MinWidth { get; init; }
    public double? MinHeight { get; init; }
    public bool InitialOpen { get; init; }
    /// Global indices of the session types each instance owns.
    public int[] Sessions { get; init; } = Array.Empty<int>();
    public required Build Render { get; init; }
    public Func<Ctx, CloseCheck>? Close { get; init; }
}

/// Everything the runtime needs to know about a generated application.
public sealed class AppDef
{
    public required string Id { get; init; }
    public required string BundleId { get; init; }
    /// Text key of the application name.
    public required string Name { get; init; }
    public required string Version { get; init; }
    public ulong Fingerprint { get; init; }
    public required string[] Languages { get; init; }
    public required string DefaultLanguage { get; init; }
    /// `language → key → text`.
    public required Dictionary<string, Dictionary<string, string>> Strings { get; init; }
    /// Protocol, UI, media and session catalog types.
    public required TypeTable Types { get; init; }
    /// Engine state field types, by field index.
    public required BType[] State { get; init; }
    /// Protocol commands and queries, by catalog index.
    public required CommandDef[] Commands { get; init; }
    public required SessionDef[] Sessions { get; init; }
    /// Result types of the framework session commands, by command index.
    public required BType[] SessionResults { get; init; }
    public required WindowDef[] Windows { get; init; }
    public required string Primary { get; init; }
    public string[] Players { get; init; } = Array.Empty<string>();
    public double? CompactBelow { get; init; }
    /// The declared brand color, mapped to the accent color resources.
    public (byte R, byte G, byte B)? BrandSeed { get; init; }
}

/// A writable session field bound to a control.
public readonly record struct Field(int Session, int FieldIndex);

/// What a component body reads: its window, the window's drafts, the
/// component name for control targets, the row key inside lists, local
/// variables, the page whose header shows titles, back and tools, and the
/// enclosing navigation.
public sealed class Ctx
{
    sealed record LocalVar(string Name, Func<Value> Get, LocalVar? Next);

    Win win = null!;
    string comp = "";
    Func<string>? key;
    LocalVar? locals;
    Page? page;
    bool toolbar;
    Field? nav;
    Cell<bool>? shown;

    internal Win Win => win;
    internal Page? Page => page;
    internal bool InToolbar => toolbar;
    internal Field? Nav => nav;

    internal static Ctx Root(Win win, Page? page) => new() { win = win, page = page };

    Ctx Copy() => (Ctx)MemberwiseClone();

    /// Engine state field, recording a dependency.
    public Value Engine(int field) => Host.EngineValue(field);

    /// A field of the window's session drafts, recording a dependency.
    public Value Session(int session, int field) => win.Cell(new Field(session, field)).Get();

    /// A local variable: a component parameter, row variable or pattern
    /// binding.
    public Value Local(string name)
    {
        for (var local = locals; local is not null; local = local.Next)
            if (local.Name == name) return local.Get();
        return Value.Unit;
    }

    /// The context of a referenced component: its own name and parameters,
    /// evaluated in this context.
    public Ctx Child(string component, params (string Name, Get Get)[] parameters)
    {
        var parent = this;
        LocalVar? next = null;
        foreach (var (name, get) in parameters) next = new LocalVar(name, () => get(parent), next);
        var child = Copy();
        child.comp = component;
        child.key = null;
        child.locals = next;
        return child;
    }

    internal Ctx WithLocal(string name, Func<Value> get)
    {
        var child = Copy();
        child.locals = new LocalVar(name, get, locals);
        return child;
    }

    internal Ctx WithKey(Func<string> rowKey)
    {
        var child = Copy();
        child.key = rowKey;
        return child;
    }

    internal Ctx WithPage(Page header)
    {
        var child = Copy();
        child.page = header;
        return child;
    }

    internal Ctx Toolbar()
    {
        var child = Copy();
        child.toolbar = true;
        return child;
    }

    internal Ctx WithNav(Field path)
    {
        var child = Copy();
        child.nav = path;
        return child;
    }

    internal Ctx WithShown(Cell<bool> cell)
    {
        var child = Copy();
        child.shown = cell;
        return child;
    }

    /// Whether this content is showing; content under a pushed destination
    /// is not.
    internal bool Shown() => shown?.Get() ?? true;

    /// The control target of `id`: `component.id`, with the row key inside
    /// lists.
    internal string Target(string id) => key is null ? $"{comp}.{id}" : $"{comp}.{id}[{key()}]";

    internal Value Read(Field field) => win.Cell(field).Get();
    internal Value Peek(Field field) => win.Cell(field).Peek();
    internal bool Write(Field field, Value value) => win.Write(field, value);
}

/// A page with a header: titles, back actions, title widgets and tools
/// registered by what it shows. The innermost registration wins.
public sealed class Page
{
    readonly List<(long Id, Func<string> Title)> titles = new();
    readonly List<(long Id, Action Back)> backs = new();
    readonly List<(long Id, UIElement Widget)> titleWidgets = new();
    readonly List<long> hidden = new();
    long next = 1;
    internal readonly StackPanel Tools = new() { Orientation = Orientation.Horizontal, Spacing = 4 };
    internal readonly Version Version = new();

    long Register<T>(List<(long, T)> list, T item)
    {
        var id = next++;
        list.Add((id, item));
        Version.Bump();
        Reactive.OnCleanup(() =>
        {
            list.RemoveAll(entry => entry.Item1 == id);
            Version.Bump();
        });
        return id;
    }

    internal void PushTitle(Func<string> title) => Register(titles, title);
    internal void PushBack(Action back) => Register(backs, back);
    internal void PushTitleWidget(UIElement widget) => Register(titleWidgets, widget);

    /// Hide this page's header while the current scope lasts, for content
    /// that brings headers of its own.
    internal void HideHeader()
    {
        var id = next++;
        hidden.Add(id);
        Version.Bump();
        Reactive.OnCleanup(() => { hidden.Remove(id); Version.Bump(); });
    }

    /// A holder for toolbar items, removed when the current scope ends.
    internal StackPanel AddTools()
    {
        var holder = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        Tools.Children.Add(holder);
        Version.Bump();
        Reactive.OnCleanup(() => { Tools.Children.Remove(holder); Version.Bump(); });
        return holder;
    }

    internal string? Title => titles.Count > 0 ? titles[^1].Title() : null;
    internal Action? Back => backs.Count > 0 ? backs[^1].Back : null;
    internal UIElement? TitleWidget => titleWidgets.Count > 0 ? titleWidgets[^1].Widget : null;
    internal bool HeaderHidden => hidden.Count > 0;
}

/// Where builders put elements: a panel and a region inside it.
/// Conditional and repeated content gets a region of its own: clearing or
/// reordering a region touches only its elements, and new elements land
/// at the region's place among its siblings.
public sealed class Parent
{
    sealed class Region(Region? parent)
    {
        public readonly List<object> Items = new(); // UIElement or Region
        public readonly Region? Up = parent;

        public void Elements(List<UIElement> output)
        {
            foreach (var item in Items)
                if (item is UIElement element) output.Add(element);
                else ((Region)item).Elements(output);
        }

        public int Count() => Items.Sum(item => item is Region region ? region.Count() : 1);

        public int Offset()
        {
            if (Up is null) return 0;
            var before = Up.Offset();
            foreach (var item in Up.Items)
            {
                if (ReferenceEquals(item, this)) break;
                before += item is Region region ? region.Count() : 1;
            }
            return before;
        }
    }

    readonly Panel panel;
    readonly Region region;
    readonly HorizontalAlignment? crossH;
    readonly VerticalAlignment? crossV;

    /// Whether children are rows of a form section (cards).
    public bool IsList { get; }
    public bool IsHorizontal => panel is StackPanel { Orientation: Orientation.Horizontal } or FlexPanel { Orientation: Orientation.Horizontal };

    Parent(Panel panel, Region region, bool list, HorizontalAlignment? h, VerticalAlignment? v)
    {
        this.panel = panel;
        this.region = region;
        IsList = list;
        crossH = h;
        crossV = v;
    }

    internal static Parent Of(Panel panel) => new(panel, new Region(null), false, null, null);
    internal static Parent OfList(Panel panel) => new(panel, new Region(null), true, null, null);
    internal static Parent Aligned(Panel panel, HorizontalAlignment? h, VerticalAlignment? v) =>
        new(panel, new Region(null), false, h, v);

    /// Add an element at the end of this region.
    public void Add(UIElement element)
    {
        if (element is FrameworkElement framework)
        {
            if (crossH is { } h) framework.HorizontalAlignment = h;
            if (crossV is { } v) framework.VerticalAlignment = v;
        }
        var index = region.Offset() + region.Count();
        region.Items.Add(element);
        panel.Children.Insert(Math.Min(index, panel.Children.Count), element);
    }

    /// A new empty region at the end of this one.
    public Parent Sub()
    {
        var child = new Region(region);
        region.Items.Add(child);
        return new Parent(panel, child, IsList, crossH, crossV);
    }

    /// Remove every element of this region.
    public void Clear()
    {
        var elements = new List<UIElement>();
        region.Elements(elements);
        foreach (var element in elements) panel.Children.Remove(element);
        region.Items.Clear();
    }

    /// Put child regions in a new order, keeping their elements.
    internal void Reorder(IList<Parent> order)
    {
        var elements = new List<UIElement>();
        region.Elements(elements);
        foreach (var element in elements) panel.Children.Remove(element);
        region.Items.Clear();
        foreach (var child in order) region.Items.Add(child.region);
        var start = region.Offset();
        elements.Clear();
        region.Elements(elements);
        for (var i = 0; i < elements.Count; i++)
            panel.Children.Insert(Math.Min(start + i, panel.Children.Count), elements[i]);
    }

    /// Drop a child region and its elements.
    internal void Remove(Parent child)
    {
        child.Clear();
        region.Items.Remove(child.region);
    }
}

/// What a control can do, each the same closure its element runs.
sealed class Actions
{
    public Action? Activate;
    public Action<string>? Edit;
    public Action? Submit;
    public Action<bool>? SetBool;
    public Action<double>? SetNumber;
    /// Select an option by its name; false when there is no such option.
    public Func<string, bool>? Select;
    /// Go back; false when there is nothing to go back to.
    public Func<bool>? Back;
    public Func<string?>? Value;
}

sealed record ControlEntry(
    string Target, WeakReference<Win> Window, string Role,
    Func<string>? Label, Func<bool>? Enabled, Actions Actions);

/// Mounted controls by target. External entries operate them through the
/// same closures the user's input runs.
static class Controls
{
    static readonly Dictionary<long, ControlEntry> entries = new();
    static long next = 1;

    public static void Register(
        Ctx ctx, string id, string role, Func<string>? label, Func<bool>? enabled, Actions actions)
    {
        var token = next++;
        entries[token] = new ControlEntry(ctx.Target(id), new WeakReference<Win>(ctx.Win), role, label, enabled, actions);
        Reactive.OnCleanup(() => entries.Remove(token));
    }

    public static List<ControlEntry> All => entries.OrderBy(entry => entry.Key).Select(entry => entry.Value).ToList();
}

/// Localized text and the interface language. The language and its strings
/// are cells: switching language re-runs every binding that shows text, in
/// one update, while drafts, navigation and windows stay.
public static class Text
{
    static Dictionary<string, Dictionary<string, string>> all = new();
    static readonly Cell<string> language = new("en");
    static readonly Cell<Dictionary<string, string>> strings = new(new());
    internal static string[] Supported = Array.Empty<string>();

    internal static void Install(AppDef app)
    {
        all = app.Strings;
        Supported = app.Languages;
        var chosen = Choose(app.Languages, app.DefaultLanguage);
        language.Set(chosen);
        strings.Set(all.GetValueOrDefault(chosen) ?? new());
    }

    /// The supported language that best matches the user's languages.
    static string Choose(string[] supported, string fallback)
    {
        var wanted = new List<string>();
        try
        {
            wanted.AddRange(global::Windows.System.UserProfile.GlobalizationPreferences.Languages);
        }
        catch (Exception) { }
        wanted.Add(CultureInfo.CurrentUICulture.Name);
        foreach (var tag in wanted)
        {
            var exact = supported.FirstOrDefault(candidate => string.Equals(candidate, tag, StringComparison.OrdinalIgnoreCase));
            if (exact is not null) return exact;
        }
        foreach (var tag in wanted)
        {
            var lower = tag.ToLowerInvariant();
            // zh-CN, zh-SG → zh-Hans; zh-TW, zh-HK → zh-Hant.
            var script = lower.StartsWith("zh-tw") || lower.StartsWith("zh-hk") || lower.StartsWith("zh-mo") || lower.StartsWith("zh-hant")
                ? "zh-hant"
                : lower.StartsWith("zh") ? "zh-hans" : lower.Split('-')[0];
            var match = supported.FirstOrDefault(candidate => candidate.ToLowerInvariant() == script)
                ?? supported.FirstOrDefault(candidate => candidate.ToLowerInvariant().Split('-')[0] == script);
            if (match is not null) return match;
        }
        return fallback;
    }

    /// The current language tag, recording a dependency.
    public static string Language => language.Get();
    internal static string PeekLanguage => language.Peek();

    /// Switch the interface language. False for unsupported tags.
    internal static bool SetLanguage(string tag)
    {
        if (!Supported.Contains(tag)) return false;
        Reactive.Batch(() =>
        {
            language.Set(tag);
            strings.Set(all.GetValueOrDefault(tag) ?? new());
        });
        return true;
    }

    /// The translation of `key` in the current language.
    public static string Tr(string key) => strings.Get().TryGetValue(key, out var text) ? text : key;

    /// A translation with `{name}` placeholders filled from `args`.
    public static string Fmt(string key, params (string Name, string Value)[] args) => Fill(Tr(key), args);

    internal static string Fill(string template, (string Name, string Value)[] args)
    {
        var output = new StringBuilder(template.Length);
        for (var i = 0; i < template.Length; i++)
        {
            var c = template[i];
            if (c == '{' && i + 1 < template.Length && template[i + 1] == '{') { output.Append('{'); i++; continue; }
            if (c == '}' && i + 1 < template.Length && template[i + 1] == '}') { output.Append('}'); i++; continue; }
            if (c == '{')
            {
                var end = template.IndexOf('}', i + 1);
                if (end < 0) { output.Append(template, i, template.Length - i); break; }
                var name = template.Substring(i + 1, end - i - 1);
                foreach (var (argName, value) in args)
                    if (argName == name) { output.Append(value); break; }
                i = end;
                continue;
            }
            output.Append(c);
        }
        return output.ToString();
    }

    /// A language's own name for itself, such as “简体中文” or “English”.
    public static string Autonym(string tag)
    {
        try
        {
            var culture = CultureInfo.GetCultureInfo(tag);
            var name = culture.NativeName;
            return tag switch
            {
                "zh-Hans" => "简体中文",
                "zh-Hant" => "繁體中文",
                _ => name.Length > 0 ? char.ToUpper(name[0], culture) + name[1..] : tag,
            };
        }
        catch (Exception)
        {
            return tag;
        }
    }

    /// Text of a value for runtime data: numbers and texts as Bern formats
    /// them, enums by their label.
    public static string Display(Value value, string? enumName) =>
        value is EnumValue e && enumName is not null ? Label(enumName, e.Variant) : R.Plain(value);

    /// The localized label of an enum variant.
    public static string Label(string enumName, uint variant)
    {
        if (!Host.App.Types.Enums.TryGetValue(enumName, out var variants) || variant >= variants.Length) return "";
        return Tr($"{enumName}.{variants[variant].Name}");
    }
}
