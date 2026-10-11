// Bern runtime for WinUI 3: standard controls and containers.
// Generated build input; do not edit.
//
// Controls are the Fluent controls of WinUI 3. Inside a form section each
// control is a card row, as Windows Settings lays them out; elsewhere it is
// the plain control. Colors come from the system theme resources only.

using System;
using System.Collections.Generic;
using System.Linq;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Markup;

namespace Bern;

/// Styles the runtime uses, all from system theme resources, so they follow
/// light, dark and contrast themes while the application runs.
static class Styles
{
    const string Xaml = """
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Style x:Key="BernCard" TargetType="Border">
    <Setter Property="Background" Value="{ThemeResource CardBackgroundFillColorDefaultBrush}" />
    <Setter Property="BorderBrush" Value="{ThemeResource CardStrokeColorDefaultBrush}" />
    <Setter Property="BorderThickness" Value="1" />
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}" />
    <Setter Property="Padding" Value="16,12" />
    <Setter Property="MinHeight" Value="56" />
  </Style>
  <Style x:Key="BernSecondary" TargetType="TextBlock">
    <Setter Property="Foreground" Value="{ThemeResource TextFillColorSecondaryBrush}" />
    <Setter Property="TextWrapping" Value="Wrap" />
  </Style>
  <Style x:Key="BernCriticalButton" TargetType="Button">
    <Setter Property="Foreground" Value="{ThemeResource SystemFillColorCriticalBrush}" />
  </Style>
</ResourceDictionary>
""";

    internal static void Install(ResourceDictionary resources)
    {
        try { resources.MergedDictionaries.Add((ResourceDictionary)XamlReader.Load(Xaml)); }
        catch (Exception error) { Console.Error.WriteLine($"bern: styles: {error.Message}"); }
    }

    /// Lightweight styling of a borderless button: transparent at rest and
    /// when disabled, the subtle fills on hover and press, in every theme.
    const string SubtleXaml = """
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <ResourceDictionary.ThemeDictionaries>
    <ResourceDictionary x:Key="Default">
      <StaticResource x:Key="ButtonBackground" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBackgroundPointerOver" ResourceKey="SubtleFillColorSecondaryBrush" />
      <StaticResource x:Key="ButtonBackgroundPressed" ResourceKey="SubtleFillColorTertiaryBrush" />
      <StaticResource x:Key="ButtonBackgroundDisabled" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrush" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushPointerOver" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushPressed" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushDisabled" ResourceKey="SubtleFillColorTransparentBrush" />
    </ResourceDictionary>
    <ResourceDictionary x:Key="Light">
      <StaticResource x:Key="ButtonBackground" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBackgroundPointerOver" ResourceKey="SubtleFillColorSecondaryBrush" />
      <StaticResource x:Key="ButtonBackgroundPressed" ResourceKey="SubtleFillColorTertiaryBrush" />
      <StaticResource x:Key="ButtonBackgroundDisabled" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrush" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushPointerOver" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushPressed" ResourceKey="SubtleFillColorTransparentBrush" />
      <StaticResource x:Key="ButtonBorderBrushDisabled" ResourceKey="SubtleFillColorTransparentBrush" />
    </ResourceDictionary>
  </ResourceDictionary.ThemeDictionaries>
</ResourceDictionary>
""";

    internal static void MakeSubtle(Button button)
    {
        try { button.Resources.MergedDictionaries.Add((ResourceDictionary)XamlReader.Load(SubtleXaml)); }
        catch (Exception error) { Console.Error.WriteLine($"bern: subtle button: {error.Message}"); }
    }

    internal static Style? Named(string key) =>
        Application.Current.Resources.TryGetValue(key, out var style) ? style as Style : null;
}

/// Bern icon names (SF Symbols naming) as Segoe Fluent Icons glyphs, the
/// system icon font of Windows 11.
static class Icons
{
    static readonly Dictionary<string, string> glyphs = new()
    {
        ["plus"] = "", ["minus"] = "", ["xmark"] = "", ["checkmark"] = "",
        ["chevron.left"] = "", ["chevron.right"] = "", ["chevron.down"] = "",
        ["chevron.up.chevron.down"] = "", ["ellipsis"] = "", ["ellipsis.circle"] = "",
        ["square.on.square"] = "", ["magnifyingglass"] = "", ["xmark.circle.fill"] = "",
        ["doc"] = "", ["photo"] = "", ["film"] = "", ["music.note"] = "",
        ["arrow.down.circle"] = "", ["gearshape"] = "", ["gearshape.2"] = "",
        ["paintbrush"] = "", ["hand.tap"] = "", ["list.bullet.rectangle"] = "",
        ["play.rectangle"] = "", ["play.fill"] = "", ["pause.fill"] = "",
        ["speaker.fill"] = "", ["speaker.wave.3.fill"] = "", ["speaker.slash.fill"] = "",
        ["sun.min"] = "", ["sun.max.fill"] = "", ["textformat"] = "",
        ["textformat.size.smaller"] = "", ["textformat.size.larger"] = "",
        ["character.cursor.ibeam"] = "", ["switch.2"] = "", ["star"] = "",
        ["star.fill"] = "", ["square.and.arrow.up"] = "", ["trash"] = "",
        ["exclamationmark.triangle"] = "", ["tray"] = "", ["heart"] = "",
    };

    internal static string Glyph(string name) => glyphs.GetValueOrDefault(name, "");

    internal static FontIcon Icon(string name, double size = 16) => new() { Glyph = Glyph(name), FontSize = size };
}

public static partial class B
{
    static Func<string> Bound(Ctx ctx, Str text) => () => text(ctx);

    static Func<bool>? Enabled(Ctx ctx, Get? enabled) =>
        enabled is null ? null : () => R.Truthy(enabled(ctx));

    static Action? Bound(Ctx ctx, Act? action) => action is null ? null : () => action(ctx);

    static void BindText(TextBlock block, Func<string> text) => Reactive.Effect(() => block.Text = text());

    /// A card row of a form section holding `content`.
    internal static Border Card(UIElement content)
    {
        if (content is FrameworkElement { VerticalAlignment: VerticalAlignment.Stretch } framework)
            framework.VerticalAlignment = VerticalAlignment.Center;
        var card = new Border { Child = content };
        if (Styles.Named("BernCard") is { } style) card.Style = style;
        return card;
    }

    /// A card row: a label on the left and a control on the right.
    static Border LabeledCard(Func<string>? label, FrameworkElement control, UIElement? prefix = null)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        var column = 0;
        if (prefix is not null)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.Children.Add(prefix);
            column++;
        }
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var caption = new TextBlock { VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap };
        if (label is not null) BindText(caption, label);
        Grid.SetColumn(caption, column);
        grid.Children.Add(caption);
        control.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(control, column + 1);
        grid.Children.Add(control);
        return Card(grid);
    }

    /// Add an element as it fits the parent: plainly, or as a card row.
    static void Place(Parent p, UIElement element)
    {
        if (p.IsList) p.Add(Card(element));
        else p.Add(element);
    }

    static string TextStyleKey(string? style) => style switch
    {
        "large_title" => "TitleLargeTextBlockStyle",
        "title" => "TitleTextBlockStyle",
        "title2" => "SubtitleTextBlockStyle",
        "title3" or "headline" or "subheadline" => "BodyStrongTextBlockStyle",
        "footnote" or "caption" or "caption2" => "CaptionTextBlockStyle",
        _ => "BodyTextBlockStyle",
    };

    static void Secondary(TextBlock block)
    {
        // Secondary text: the system's secondary text brush, as a theme
        // resource, so it follows the theme.
        block.SetValue(TextBlock.ForegroundProperty, Application.Current.Resources["TextFillColorSecondaryBrush"]);
        block.ActualThemeChanged += (sender, _) =>
            ((TextBlock)sender).Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];
    }

    public static void Text(Parent p, Ctx ctx, Str? content = null, string? style = null, string? emphasis = null)
    {
        var block = new TextBlock { TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = false };
        if (Styles.Named(TextStyleKey(style)) is { } textStyle) block.Style = textStyle;
        if (emphasis is "secondary" or "tertiary") Secondary(block);
        if (content is not null) BindText(block, Bound(ctx, content));
        Place(p, block);
    }

    public static void Label(Parent p, Ctx ctx, Str? title = null, string? icon = null, string? style = null)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        if (icon is not null) row.Children.Add(Icons.Icon(icon));
        var block = new TextBlock { VerticalAlignment = VerticalAlignment.Center };
        if (Styles.Named(TextStyleKey(style)) is { } textStyle) block.Style = textStyle;
        if (title is not null) BindText(block, Bound(ctx, title));
        row.Children.Add(block);
        Place(p, row);
    }

    static void RegisterButton(Ctx ctx, string id, Func<string>? label, Func<bool>? enabled, Action? action) =>
        Controls.Register(ctx, id, "button", label, enabled, new Actions { Activate = action });

    public static void Button(Parent p, Ctx ctx, string id, Str? label = null, string? icon = null,
        string? role = null, string? prominence = null, Get? enabled = null, Act? action = null)
    {
        var button = new Button();
        var text = label is null ? null : Bound(ctx, label);
        if (icon is not null && text is not null)
        {
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            content.Children.Add(Icons.Icon(icon));
            var block = new TextBlock();
            BindText(block, text);
            content.Children.Add(block);
            button.Content = content;
        }
        else if (icon is not null) button.Content = Icons.Icon(icon);
        else if (text is not null)
        {
            var block = new TextBlock();
            BindText(block, text);
            button.Content = block;
        }
        if (prominence == "prominent" && Styles.Named("AccentButtonStyle") is { } accent) button.Style = accent;
        else if (prominence == "plain") Styles.MakeSubtle(button);
        else if (role == "destructive" && Styles.Named("BernCriticalButton") is { } critical) button.Style = critical;
        if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(button, text()));
        var isEnabled = Enabled(ctx, enabled);
        if (isEnabled is not null) Reactive.Effect(() => button.IsEnabled = isEnabled());
        var run = Bound(ctx, action);
        if (run is not null) button.Click += (_, _) => run();
        RegisterButton(ctx, id, text, isEnabled, run);
        if (p.IsList && !ctx.InToolbar) p.Add(Card(button));
        else p.Add(button);
    }

    public static void IconButton(Parent p, Ctx ctx, string id, string icon, Str? label = null, Get? enabled = null, Act? action = null)
    {
        var button = new Button { Content = Icons.Icon(icon) };
        Styles.MakeSubtle(button);
        var text = label is null ? null : Bound(ctx, label);
        if (text is not null)
            Reactive.Effect(() =>
            {
                var name = text();
                ToolTipService.SetToolTip(button, name);
                AutomationProperties.SetName(button, name);
            });
        var isEnabled = Enabled(ctx, enabled);
        if (isEnabled is not null) Reactive.Effect(() => button.IsEnabled = isEnabled());
        var run = Bound(ctx, action);
        if (run is not null) button.Click += (_, _) => run();
        RegisterButton(ctx, id, text, isEnabled, run);
        Place(p, button);
    }

    /// A draft field bound to a control.
    sealed class Bound_(Ctx ctx, Field field)
    {
        public Value Get() => ctx.Read(field);
        public Value Peek() => ctx.Peek(field);

        public void Set(Value value)
        {
            if (!Peek().Equals(value)) ctx.Write(field, value);
        }
    }

    /// Text editing: changes update the draft; draft changes from elsewhere
    /// update the text without echoing back.
    static Func<bool> Editable(Func<string> read, Action<string> write, Bound_ bound)
    {
        var updating = false;
        Reactive.Effect(() =>
        {
            var value = R.AsText(bound.Get());
            if (Reactive.Untrack(read) != value)
            {
                updating = true;
                write(value);
                updating = false;
            }
        });
        return () => updating;
    }

    static StackPanel Stacked(Func<string>? label, UIElement control)
    {
        var panel = new StackPanel { Spacing = 6 };
        if (label is not null)
        {
            var caption = new TextBlock();
            BindText(caption, label);
            panel.Children.Add(caption);
        }
        panel.Children.Add(control);
        return panel;
    }

    public static void TextInput(Parent p, Ctx ctx, string id, Str? label = null, Field? field = null,
        Str? prompt = null, string? kind = null, Act? submit = null)
    {
        if (field is not { } f) return;
        var bound = new Bound_(ctx, f);
        var text = label is null ? null : Bound(ctx, label);
        var placeholder = prompt is null ? null : Bound(ctx, prompt);
        var run = Bound(ctx, submit);
        var secure = kind == "secure";
        FrameworkElement control;
        if (secure)
        {
            var box = new PasswordBox { MinWidth = 240 };
            if (placeholder is not null) Reactive.Effect(() => box.PlaceholderText = placeholder());
            var updating = Editable(() => box.Password, value => box.Password = value, bound);
            box.PasswordChanged += (_, _) => { if (!updating()) bound.Set(Value.Text(box.Password)); };
            if (run is not null) box.KeyDown += (_, args) => { if (args.Key == global::Windows.System.VirtualKey.Enter) run(); };
            control = box;
        }
        else
        {
            var multiline = kind == "multiline";
            var box = new TextBox
            {
                MinWidth = 240,
                AcceptsReturn = multiline,
                TextWrapping = multiline ? TextWrapping.Wrap : TextWrapping.NoWrap,
                MinHeight = multiline ? 80 : 0,
            };
            if (placeholder is not null) Reactive.Effect(() => box.PlaceholderText = placeholder());
            var updating = Editable(() => box.Text, value => box.Text = value, bound);
            box.TextChanged += (_, _) => { if (!updating()) bound.Set(Value.Text(box.Text)); };
            if (run is not null && !multiline)
                box.KeyDown += (_, args) => { if (args.Key == global::Windows.System.VirtualKey.Enter) run(); };
            control = box;
        }
        if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(control, text()));
        if (p.IsList)
        {
            if (kind == "multiline")
            {
                control.HorizontalAlignment = HorizontalAlignment.Stretch;
                p.Add(Card(Stacked(text, control)));
            }
            else p.Add(LabeledCard(text, control));
        }
        else p.Add(Stacked(text, control));
        Controls.Register(ctx, id, "text_input", text, null, new Actions
        {
            Edit = value => bound.Set(Value.Text(value)),
            Submit = run,
            Value = secure ? null : () => R.AsText(bound.Peek()),
        });
    }

    public static void Search(Parent p, Ctx ctx, string id, Field? field = null, Str? prompt = null, Act? submit = null)
    {
        if (field is not { } f) return;
        var bound = new Bound_(ctx, f);
        var placeholder = prompt is null ? null : Bound(ctx, prompt);
        var box = new AutoSuggestBox { QueryIcon = new SymbolIcon(Symbol.Find) };
        if (placeholder is not null)
            Reactive.Effect(() =>
            {
                var text = placeholder();
                box.PlaceholderText = text;
                AutomationProperties.SetName(box, text);
            });
        var updating = Editable(() => box.Text, value => box.Text = value, bound);
        box.TextChanged += (_, args) =>
        {
            if (!updating()) bound.Set(Value.Text(box.Text));
        };
        var run = Bound(ctx, submit);
        box.QuerySubmitted += (_, _) => run?.Invoke();
        Place(p, box);
        Controls.Register(ctx, id, "search", placeholder ?? (() => ""), null, new Actions
        {
            Edit = value => bound.Set(Value.Text(value)),
            Submit = run,
            Value = () => R.AsText(bound.Peek()),
        });
    }

    internal static AutoSuggestBox SearchBox(Ctx ctx, string id, Field field, Str? prompt, Act? submit)
    {
        var holder = new StackPanel();
        Search(Parent.Of(holder), ctx, id, field, prompt, submit);
        var box = (AutoSuggestBox)holder.Children[0];
        holder.Children.Clear();
        return box;
    }

    public static void Switch(Parent p, Ctx ctx, string id, Str? label = null, Field? field = null, string? style = null)
    {
        if (field is not { } f) return;
        var bound = new Bound_(ctx, f);
        var text = label is null ? null : Bound(ctx, label);
        var updating = false;
        if (style == "checkbox")
        {
            var check = new CheckBox();
            if (text is not null)
            {
                var block = new TextBlock();
                BindText(block, text);
                check.Content = block;
                Reactive.Effect(() => AutomationProperties.SetName(check, text()));
            }
            Reactive.Effect(() =>
            {
                var on = bound.Get().Equals(Value.Bool(true));
                if (check.IsChecked != on) { updating = true; check.IsChecked = on; updating = false; }
            });
            check.Checked += (_, _) => { if (!updating) bound.Set(Value.Bool(true)); };
            check.Unchecked += (_, _) => { if (!updating) bound.Set(Value.Bool(false)); };
            Place(p, check);
        }
        else
        {
            var toggle = new ToggleSwitch();
            // The state text follows the app's language, not the system's.
            Reactive.Effect(() =>
            {
                toggle.OnContent = Bern.Text.Tr("bern.switch.on");
                toggle.OffContent = Bern.Text.Tr("bern.switch.off");
            });
            if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(toggle, text()));
            Reactive.Effect(() =>
            {
                var on = bound.Get().Equals(Value.Bool(true));
                if (toggle.IsOn != on) { updating = true; toggle.IsOn = on; updating = false; }
            });
            toggle.Toggled += (_, _) => { if (!updating) bound.Set(Value.Bool(toggle.IsOn)); };
            if (p.IsList)
            {
                // The card shows the label; the switch shows its state text.
                toggle.MinWidth = 0;
                p.Add(LabeledCard(text, toggle));
            }
            else
            {
                if (text is not null)
                {
                    var header = new TextBlock();
                    BindText(header, text);
                    toggle.Header = header;
                }
                p.Add(toggle);
            }
        }
        Controls.Register(ctx, id, "switch", text, null, new Actions
        {
            SetBool = on => bound.Set(Value.Bool(on)),
            Value = () => bound.Peek().Equals(Value.Bool(true)) ? "true" : "false",
        });
    }

    /// A slider: dragging only updates this window's draft as a preview, and
    /// releasing commits once; keyboard changes commit at once.
    public static void Slider(Parent p, Ctx ctx, string id, Str? label = null, Field? field = null,
        double? min = null, double? max = null, double? step = null, string? minIcon = null, string? maxIcon = null,
        Act? commit = null)
    {
        if (field is not { } f) return;
        var bound = new Bound_(ctx, f);
        var text = label is null ? null : Bound(ctx, label);
        var (low, high) = (min ?? 0, max ?? 1);
        var slider = new Slider
        {
            Minimum = low,
            Maximum = high,
            StepFrequency = step ?? (high - low) / 100,
            SmallChange = step ?? (high - low) / 100,
            LargeChange = (high - low) / 10,
            IsThumbToolTipEnabled = true,
            MinWidth = 200,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        var run = Bound(ctx, commit);
        double Snap(double value) => step is { } s && s > 0
            ? Math.Clamp(low + Math.Round((value - low) / s) * s, low, high)
            : Math.Clamp(value, low, high);
        var updating = false;
        var dragging = false;
        Reactive.Effect(() =>
        {
            var value = R.AsDouble(bound.Get());
            if (Math.Abs(slider.Value - value) > double.Epsilon) { updating = true; slider.Value = value; updating = false; }
        });
        slider.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) => dragging = true), true);
        void Release()
        {
            if (!dragging) return;
            dragging = false;
            run?.Invoke();
        }
        slider.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler((_, _) => Release()), true);
        slider.AddHandler(UIElement.PointerCaptureLostEvent, new PointerEventHandler((_, _) => Release()), true);
        slider.ValueChanged += (_, args) =>
        {
            if (updating) return;
            bound.Set(Value.Float(Snap(args.NewValue)));
            if (!dragging) run?.Invoke();
        };
        if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(slider, text()));
        var track = new Grid { ColumnSpacing = 8 };
        track.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        track.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        track.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        if (minIcon is not null)
        {
            var icon = Icons.Icon(minIcon, 14);
            icon.VerticalAlignment = VerticalAlignment.Center;
            track.Children.Add(icon);
        }
        Grid.SetColumn(slider, 1);
        track.Children.Add(slider);
        if (maxIcon is not null)
        {
            var icon = Icons.Icon(maxIcon, 14);
            icon.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(icon, 2);
            track.Children.Add(icon);
        }
        var content = Stacked(text, track);
        if (p.IsList) p.Add(Card(content));
        else p.Add(content);
        Controls.Register(ctx, id, "slider", text, null, new Actions
        {
            SetNumber = number =>
            {
                bound.Set(Value.Float(Snap(number)));
                run?.Invoke();
            },
            Value = () => R.Number(R.AsDouble(bound.Peek())),
        });
    }

    /// One option of a picker: variant index, variant name and label.
    public sealed record PickOption(uint Value, string Name, Str Label);

    public static PickOption Option(uint value, string name, Str label) => new(value, name, label);

    public static void Picker(Parent p, Ctx ctx, string id, Str? label = null, Field? field = null,
        PickOption[]? options = null, string? style = null)
    {
        if (field is not { } f || options is null) return;
        var bound = new Bound_(ctx, f);
        var text = label is null ? null : Bound(ctx, label);
        int? IndexOf(Value value) => value is EnumValue e ? Array.FindIndex(options, option => option.Value == e.Variant) is var i && i >= 0 ? i : null : null;
        void Select(int index) => bound.Set(Value.Enum(options[index].Value));
        var updating = false;
        FrameworkElement control;
        if (style is "segmented" or "radio" or "inline")
        {
            var bar = new SelectorBar();
            var items = options.Select(_ => new SelectorBarItem()).ToList();
            foreach (var item in items) bar.Items.Add(item);
            for (var i = 0; i < options.Length; i++)
            {
                var (item, option) = (items[i], options[i]);
                Reactive.Effect(() => item.Text = option.Label(ctx));
            }
            Reactive.Effect(() =>
            {
                if (IndexOf(bound.Get()) is { } index && !ReferenceEquals(bar.SelectedItem, items[index]))
                {
                    updating = true;
                    bar.SelectedItem = items[index];
                    updating = false;
                }
            });
            bar.SelectionChanged += (_, _) =>
            {
                if (!updating && bar.SelectedItem is SelectorBarItem selected) Select(items.IndexOf(selected));
            };
            control = bar;
        }
        else
        {
            var combo = new ComboBox { MinWidth = 160 };
            Reactive.Effect(() =>
            {
                var labels = options.Select(option => option.Label(ctx)).ToList();
                updating = true;
                var selected = combo.SelectedIndex;
                combo.Items.Clear();
                foreach (var labelText in labels) combo.Items.Add(labelText);
                combo.SelectedIndex = IndexOf(bound.Peek()) ?? selected;
                updating = false;
            });
            Reactive.Effect(() =>
            {
                if (IndexOf(bound.Get()) is { } index && combo.SelectedIndex != index)
                {
                    updating = true;
                    combo.SelectedIndex = index;
                    updating = false;
                }
            });
            combo.SelectionChanged += (_, _) =>
            {
                if (!updating && combo.SelectedIndex >= 0) Select(combo.SelectedIndex);
            };
            control = combo;
        }
        if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(control, text()));
        if (ctx.InToolbar) p.Add(control);
        else if (p.IsList) p.Add(LabeledCard(text, control));
        else p.Add(Stacked(text, control));
        Controls.Register(ctx, id, "picker", text, null, new Actions
        {
            Select = name =>
            {
                var index = Array.FindIndex(options, option => option.Name == name);
                if (index < 0) return false;
                Select(index);
                return true;
            },
            Value = () => IndexOf(bound.Peek()) is { } index ? options[index].Name : null,
        });
    }

    public static void Stack(Parent p, Ctx ctx, string axis, double? gap, double? padding, string? align, Build children)
    {
        var column = axis == "column";
        var panel = new FlexPanel(column ? Orientation.Vertical : Orientation.Horizontal) { Spacing = gap ?? 8 };
        if (padding is { } pad) panel.Padding = new Thickness(pad);
        if (column)
        {
            var h = align switch
            {
                "center" => HorizontalAlignment.Center,
                "trailing" => HorizontalAlignment.Right,
                "leading" => HorizontalAlignment.Left,
                _ => HorizontalAlignment.Stretch,
            };
            children(Parent.Aligned(panel, h, null), ctx);
        }
        else
        {
            var v = align switch
            {
                "top" => VerticalAlignment.Top,
                "bottom" => VerticalAlignment.Bottom,
                "fill" => VerticalAlignment.Stretch,
                _ => VerticalAlignment.Center,
            };
            children(Parent.Aligned(panel, null, v), ctx);
        }
        Place(p, panel);
    }

    public static void Spacer(Parent p) => p.Add(new SpacerElement());

    public static void Divider(Parent p)
    {
        var line = new Border
        {
            Height = p.IsHorizontal ? double.NaN : 1,
            Width = p.IsHorizontal ? 1 : double.NaN,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        line.SetValue(Border.BackgroundProperty, Application.Current.Resources["DividerStrokeColorDefaultBrush"]);
        Place(p, line);
    }

    public static void ScrollView(Parent p, Ctx ctx, string? axis, Build children)
    {
        var horizontal = axis == "horizontal";
        var panel = new FlexPanel(horizontal ? Orientation.Horizontal : Orientation.Vertical) { Spacing = 8 };
        var scroller = new ScrollViewer
        {
            Content = panel,
            HorizontalScrollBarVisibility = horizontal ? ScrollBarVisibility.Auto : ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = horizontal ? ScrollBarVisibility.Disabled : ScrollBarVisibility.Auto,
            HorizontalScrollMode = horizontal ? ScrollMode.Enabled : ScrollMode.Disabled,
            VerticalScrollMode = horizontal ? ScrollMode.Disabled : ScrollMode.Enabled,
        };
        if (!horizontal) FlexPanel.SetFill(scroller, true);
        children(Parent.Of(panel), ctx);
        p.Add(scroller);
    }

    /// A grouped form: sections of card rows in a scrolling page with a
    /// readable maximum width, as Windows Settings pages lay them out.
    public static void Form(Parent p, Ctx ctx, Str? title, Build children)
    {
        if (title is not null && ctx.Page is { } page) page.PushTitle(Bound(ctx, title));
        var content = new StackPanel { Spacing = 24, MaxWidth = 1000, Padding = new Thickness(24, 8, 24, 24) };
        var scroller = new ScrollViewer
        {
            Content = content,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        FlexPanel.SetFill(scroller, true);
        children(Parent.Of(content), ctx);
        p.Add(scroller);
    }

    /// A titled group of card rows with an optional footnote.
    public static void Section(Parent p, Ctx ctx, Str? title, Str? footer, Build children)
    {
        var group = new StackPanel { Spacing = 4 };
        if (title is not null)
        {
            var heading = new TextBlock { Margin = new Thickness(1, 0, 0, 4) };
            if (Styles.Named("BodyStrongTextBlockStyle") is { } style) heading.Style = style;
            BindText(heading, Bound(ctx, title));
            group.Children.Add(heading);
        }
        var rows = new StackPanel { Spacing = 4 };
        group.Children.Add(rows);
        if (footer is not null)
        {
            var note = new TextBlock { TextWrapping = TextWrapping.Wrap, Margin = new Thickness(1, 4, 0, 0) };
            if (Styles.Named("CaptionTextBlockStyle") is { } style) note.Style = style;
            Secondary(note);
            BindText(note, Bound(ctx, footer));
            group.Children.Add(note);
        }
        children(Parent.OfList(rows), ctx);
        p.Add(group);
    }

    /// Toolbar items for the enclosing page's header.
    public static void Toolbar(Parent p, Ctx ctx, Build children)
    {
        if (ctx.Page is not { } page) return;
        var holder = page.AddTools();
        Reactive.Effect(() => holder.Visibility = ctx.Shown() ? Visibility.Visible : Visibility.Collapsed);
        children(Parent.Of(holder), ctx.Toolbar());
    }

    public static void LanguagePicker(Parent p, Ctx ctx, string id, Str? label = null)
    {
        var supported = Bern.Text.Supported;
        var combo = new ComboBox { MinWidth = 160 };
        foreach (var tag in supported) combo.Items.Add(Bern.Text.Autonym(tag));
        var text = label is null ? null : Bound(ctx, label);
        var updating = false;
        Reactive.Effect(() =>
        {
            var index = Array.IndexOf(supported, Bern.Text.Language);
            if (combo.SelectedIndex != index) { updating = true; combo.SelectedIndex = index; updating = false; }
        });
        combo.SelectionChanged += (_, _) =>
        {
            if (updating || combo.SelectedIndex < 0) return;
            var tag = supported[combo.SelectedIndex];
            // Text bindings update after the selection change finishes.
            Host.Post(() => Bern.Text.SetLanguage(tag));
        };
        if (text is not null) Reactive.Effect(() => AutomationProperties.SetName(combo, text()));
        if (p.IsList) p.Add(LabeledCard(text, combo));
        else p.Add(Stacked(text, combo));
        Controls.Register(ctx, id, "picker", text, null, new Actions
        {
            Select = tag => Bern.Text.SetLanguage(tag),
            Value = () => Bern.Text.PeekLanguage,
        });
    }

    /// An item of a menu; null entries are dividers.
    public sealed record MenuItem(string Id, Str? Label, string? Icon, string? Role, Get? Enabled, Act? Action);

    /// A menu button. Each item is registered on its own, so external entries
    /// reach the items while the menu is closed.
    public static void Menu(Parent p, Ctx ctx, string id, Str? label, string? icon, MenuItem?[] items)
    {
        var button = new DropDownButton();
        var text = label is null ? null : Bound(ctx, label);
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (icon is not null) content.Children.Add(Icons.Icon(icon));
        if (text is not null)
        {
            var block = new TextBlock();
            BindText(block, text);
            content.Children.Add(block);
            Reactive.Effect(() => AutomationProperties.SetName(button, text()));
        }
        button.Content = content;
        var flyout = new MenuFlyout();
        foreach (var item in items)
        {
            if (item is null)
            {
                flyout.Items.Add(new MenuFlyoutSeparator());
                continue;
            }
            var entry = new MenuFlyoutItem();
            if (item.Icon is not null) entry.Icon = Icons.Icon(item.Icon);
            var itemLabel = item.Label is null ? (() => "") : Bound(ctx, item.Label);
            Reactive.Effect(() => entry.Text = itemLabel());
            var isEnabled = Enabled(ctx, item.Enabled);
            if (isEnabled is not null) Reactive.Effect(() => entry.IsEnabled = isEnabled());
            var run = Bound(ctx, item.Action);
            if (run is not null) entry.Click += (_, _) => run();
            flyout.Items.Add(entry);
            Controls.Register(ctx, item.Id, "button", itemLabel, isEnabled, new Actions { Activate = run });
        }
        button.Flyout = flyout;
        var open = false;
        flyout.Opened += (_, _) => open = true;
        flyout.Closed += (_, _) => open = false;
        Controls.Register(ctx, id, "menu", text, null, new Actions { Value = () => open ? "open" : "closed" });
        if (p.IsList && !ctx.InToolbar) p.Add(Card(button));
        else p.Add(button);
    }
}
