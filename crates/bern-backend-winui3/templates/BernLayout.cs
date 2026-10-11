// Bern runtime for WinUI 3: the flexible panel content is laid out in.
// Generated build input; do not edit.
//
// Content stacks along one axis, as StackPanel does, but filling children
// share the free space along it, as boxes do on the other targets: spacers
// in rows, and scrolling forms, split views and navigation in columns. A
// column with a filling child fills in turn.

using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Bern;

public sealed class FlexPanel : Panel
{
    public static readonly DependencyProperty FillProperty =
        DependencyProperty.RegisterAttached("Fill", typeof(bool), typeof(FlexPanel), new PropertyMetadata(false));

    /// Mark `element` as one that takes the free space of its column.
    public static void SetFill(UIElement element, bool fill) => element.SetValue(FillProperty, fill);
    public static bool GetFill(UIElement element) => element.GetValue(FillProperty) is true;

    public Orientation Orientation { get; }
    public double Spacing { get; set; }
    public Thickness Padding { get; set; }

    public FlexPanel(Orientation orientation = Orientation.Vertical) => Orientation = orientation;

    bool Horizontal => Orientation == Orientation.Horizontal;

    /// Whether `element` takes free space along this panel's axis.
    bool Fills(UIElement element)
    {
        if (element is SpacerElement) return true;
        if (Horizontal) return false;
        if (GetFill(element)) return true;
        if (element is FlexPanel { Orientation: Orientation.Vertical } column)
            foreach (var child in column.Children)
                if (child.Visibility == Visibility.Visible && column.Fills(child)) return true;
        return false;
    }

    /// Whether this panel takes free space in a column.
    internal bool FillsColumn
    {
        get
        {
            if (Horizontal) return false;
            foreach (var child in Children)
                if (child.Visibility == Visibility.Visible && Fills(child)) return true;
            return false;
        }
    }

    double Main(Size size) => Horizontal ? size.Width : size.Height;
    double Cross(Size size) => Horizontal ? size.Height : size.Width;
    Size Make(double main, double cross) => Horizontal ? new Size(main, cross) : new Size(cross, main);

    protected override Size MeasureOverride(Size availableSize)
    {
        var inner = new Size(
            Math.Max(0, availableSize.Width - Padding.Left - Padding.Right),
            Math.Max(0, availableSize.Height - Padding.Top - Padding.Bottom));
        var available = Main(inner);
        var cross = Cross(inner);
        double used = 0, thickness = 0;
        int visible = 0, filling = 0;
        foreach (var child in Children)
        {
            if (child.Visibility != Visibility.Visible) continue;
            visible++;
            if (Fills(child))
            {
                filling++;
                continue;
            }
            child.Measure(Make(double.PositiveInfinity, cross));
            used += Main(child.DesiredSize);
            thickness = Math.Max(thickness, Cross(child.DesiredSize));
        }
        var gaps = Spacing * Math.Max(0, visible - 1);
        var free = double.IsInfinity(available) ? double.PositiveInfinity : Math.Max(0, available - used - gaps);
        double filled = 0;
        foreach (var child in Children)
        {
            if (child.Visibility != Visibility.Visible || !Fills(child)) continue;
            child.Measure(Make(double.IsInfinity(free) ? double.PositiveInfinity : free / filling, cross));
            filled += Main(child.DesiredSize);
            thickness = Math.Max(thickness, Cross(child.DesiredSize));
        }
        var main = filling > 0 && !double.IsInfinity(available) ? available : used + gaps + filled;
        var size = Make(main, thickness);
        return new Size(size.Width + Padding.Left + Padding.Right, size.Height + Padding.Top + Padding.Bottom);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        var inner = new Size(
            Math.Max(0, finalSize.Width - Padding.Left - Padding.Right),
            Math.Max(0, finalSize.Height - Padding.Top - Padding.Bottom));
        var available = Main(inner);
        var cross = Cross(inner);
        double used = 0;
        int visible = 0, filling = 0;
        foreach (var child in Children)
        {
            if (child.Visibility != Visibility.Visible) continue;
            visible++;
            if (Fills(child)) filling++;
            else used += Main(child.DesiredSize);
        }
        var gaps = Spacing * Math.Max(0, visible - 1);
        var share = filling > 0 ? Math.Max(0, available - used - gaps) / filling : 0;
        var position = 0.0;
        foreach (var child in Children)
        {
            if (child.Visibility != Visibility.Visible)
            {
                child.Arrange(new Rect(0, 0, 0, 0));
                continue;
            }
            var length = Fills(child) ? share : Main(child.DesiredSize);
            var rect = Horizontal
                ? new Rect(Padding.Left + position, Padding.Top, length, cross)
                : new Rect(Padding.Left, Padding.Top + position, cross, length);
            child.Arrange(rect);
            position += length + Spacing;
        }
        return finalSize;
    }
}

/// A flexible spacer inside rows and columns.
public sealed class SpacerElement : FrameworkElement
{
}
