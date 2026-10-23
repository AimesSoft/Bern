// Bern runtime for Windows, XAML part 6 of 8: fixed scenes scaled to their
// room, marquees, accordions and flowing rows, modals over the window,
// timers, control groups and the search palette. Scenes keep one
// coordinate space under one render transform, so pictures, text and hit
// targets stay registered. Modals are the system's content dialogs; the
// palette grows out of its search box in the flyout's material. Generated
// build input; do not edit.

namespace B {

// ---------------------------------------------------------------------------
// Scenes

/// A `ScaleBox` scene's size and the room the box has around it on each
/// side, in scene points.
struct Room {
    double width = 0, height = 0, x = 0, y = 0;
    bool operator==(Room const&) const = default;

    /// How far each side of a layer at `x`, `y`, `w`, `h` reaches past the
    /// scene when it bleeds — left, right, top, bottom: a side at an edge
    /// of the scene reaches the edge of the room, with half a point of
    /// slack so a layer laid out to the edge reaches it.
    std::array<double, 4> Reach(double lx, double ly, double lw, double lh) const {
        return {lx <= 0.5 ? x : 0, lx + lw >= width - 0.5 ? x : 0, ly <= 0.5 ? y : 0, ly + lh >= height - 0.5 ? y : 0};
    }
};

/// The room around a scene, shared with the layers that bleed into it.
struct SceneRoom {
    Room room;
    /// The layers that bleed, laid out again when the room changes.
    std::vector<winrt::weak_ref<controls::Panel>> layers;

    /// Take the room around a `w` × `h` scene scaled by `scale` and
    /// centered in `area`. A box of a new shape keeps its scene's size,
    /// which XAML does not lay out again, so the layers that bleed are.
    void Fit(double w, double h, Size area, float scale) {
        bool fits = scale > 0 && std::isfinite(scale);
        auto around = [&](float side, double scene) {
            return fits && !std::isinf(side) ? (std::max)(0.0, (side / scale - scene) / 2) : 0.0;
        };
        Room next{w, h, around(area.Width, w), around(area.Height, h)};
        if (next == room) return;
        room = next;
        std::erase_if(layers, [](winrt::weak_ref<controls::Panel> const& weak) {
            auto layer = weak.get();
            if (layer) layer.InvalidateMeasure();
            return !layer;
        });
    }
};

/// `ScaleBox`: its children stacked from the top leading corner in a space
/// of `width` × `height`, scaled as a whole to fit the room it is given and
/// centered there; what stands outside is clipped. The scene's layers
/// learn the room around it, which a layer that bleeds reaches into.
struct ScalePanel : controls::PanelT<ScalePanel> {
    double width = 1, height = 1;
    std::shared_ptr<SceneRoom> scene = std::make_shared<SceneRoom>();

    float Fit(Size room) const {
        float w = static_cast<float>((std::max)(1.0, width)), h = static_cast<float>((std::max)(1.0, height));
        bool across = !std::isinf(room.Width), down = !std::isinf(room.Height);
        if (across && down) return (std::min)(room.Width / w, room.Height / h);
        if (across) return room.Width / w;
        if (down) return room.Height / h;
        return 1;
    }

    Size MeasureOverride(Size available) {
        Size space{static_cast<float>(width), static_cast<float>(height)};
        for (auto child : Children()) child.Measure(space);
        float scale = Fit(available);
        return Size{std::isinf(available.Width) ? space.Width * scale : available.Width, std::isinf(available.Height) ? space.Height * scale : available.Height};
    }

    Size ArrangeOverride(Size final) {
        float scale = Fit(final);
        // The room follows the size the box is given; a layer that bleeds
        // measures again for a new one.
        scene->Fit(width, height, final, scale);
        float w = static_cast<float>(width), h = static_cast<float>(height);
        for (auto child : Children()) {
            child.Arrange(Rect{0, 0, w, h});
            media::CompositeTransform transform;
            transform.ScaleX(scale);
            transform.ScaleY(scale);
            transform.TranslateX((final.Width - w * scale) / 2);
            transform.TranslateY((final.Height - h * scale) / 2);
            child.RenderTransform(transform);
        }
        media::RectangleGeometry clip;
        clip.Rect(Rect{0, 0, final.Width, final.Height});
        Clip(clip);
        return final;
    }
};

struct ScaleBoxProps {
    Len width;
    Len height;
};

inline void ScaleBox(Parent const& p, Ctx const& ctx, ScaleBoxProps props, Build const& children) {
    auto impl = winrt::make_self<ScalePanel>();
    controls::Panel panel = impl.as<controls::Panel>();
    MarkFill(panel);
    ScalePanel* scene = impl.get();
    auto keep = winrt::make_weak(panel);
    BindLen(ctx, props.width, nullptr, [scene, keep](double v) {
        scene->width = (std::max)(1.0, v);
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    });
    BindLen(ctx, props.height, nullptr, [scene, keep](double v) {
        scene->height = (std::max)(1.0, v);
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    });
    // The space the children stack in, from its top leading corner.
    controls::Grid space;
    space.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    space.VerticalAlignment(xaml::VerticalAlignment::Top);
    auto inner = ctx;
    inner.scene = impl->scene;
    children(Parent::Aligned(space, xaml::HorizontalAlignment::Left, xaml::VerticalAlignment::Top), inner);
    panel.Children().Append(space);
    p.Add(panel);
}

/// The sides of a layer that bleeds: whether it bleeds, where it is going
/// (its place before any motion), the room of its scene as it last laid
/// out, and how far each side reaches past the scene — left, right, top,
/// bottom — on the layer's motion.
struct Bleed {
    bool on = false;
    std::array<double, 4> target{};
    std::optional<Room> seen;
    std::array<std::shared_ptr<Spring>, 4> sides;
    Motion const* motion = nullptr;

    /// Move the sides to where they reach at the layer's target: on the
    /// layer's motion, or with `jump` at once, for a box of a new shape.
    void Follow(bool jump) {
        std::array<double, 4> reach{};
        if (on && seen) reach = seen->Reach(target[0], target[1], target[2], target[3]);
        for (size_t i = 0; i < sides.size(); ++i) {
            auto& side = *sides[i];
            if (jump) {
                Frames::Stop(&side);
                side.value = side.target = reach[i];
                side.velocity = 0;
            } else if (side.target != reach[i]) {
                side.To(reach[i], motion);
            }
        }
    }
};

/// `Positioned`: its content in the rectangle `x`, `y`, `width`, `height`
/// of the space it stands in, centered there. It takes all the room its
/// stack offers, as a layer does. With `bleed`, each side that reaches its
/// scene's edge continues to the edge of the room around the scene.
struct PositionedPanel : controls::PanelT<PositionedPanel> {
    double x = 0, y = 0, width = 0, height = 0;
    std::shared_ptr<SceneRoom> scene;
    std::shared_ptr<Bleed> bleed;

    /// How far each side reaches past the scene now.
    std::array<double, 4> Sides() {
        if (!bleed) return {};
        if (scene && bleed->seen != scene->room) {
            bleed->seen = scene->room;
            bleed->Follow(true);
        }
        std::array<double, 4> sides{};
        for (size_t i = 0; i < sides.size(); ++i) sides[i] = (std::max)(0.0, bleed->sides[i]->value);
        return sides;
    }

    Size MeasureOverride(Size available) {
        auto [left, right, top, bottom] = Sides();
        Size content{static_cast<float>((std::max)(0.0, width + left + right)), static_cast<float>((std::max)(0.0, height + top + bottom))};
        for (auto child : Children()) child.Measure(content);
        return Size{std::isinf(available.Width) ? static_cast<float>(x + width) : available.Width,
            std::isinf(available.Height) ? static_cast<float>(y + height) : available.Height};
    }

    Size ArrangeOverride(Size final) {
        auto [left, right, top, bottom] = Sides();
        Rect frame{static_cast<float>(x - left), static_cast<float>(y - top), static_cast<float>((std::max)(0.0, width + left + right)), static_cast<float>((std::max)(0.0, height + top + bottom))};
        for (auto child : Children()) child.Arrange(frame);
        return final;
    }
};

struct PositionedProps {
    Len x;
    Len y;
    Len width;
    Len height;
    Motion const* animate = nullptr;
    /// Whether the layer continues past each edge of its scene it
    /// reaches, to the edge of the `ScaleBox`.
    Get bleed;
};

inline void Positioned(Parent const& p, Ctx const& ctx, PositionedProps props, Build const& children) {
    auto impl = winrt::make_self<PositionedPanel>();
    controls::Panel panel = impl.as<controls::Panel>();
    MarkFill(panel);
    PositionedPanel* place = impl.get();
    auto keep = winrt::make_weak(panel);
    auto relayout = [keep] {
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    };
    BindLen(ctx, props.x, props.animate, [place, relayout](double v) { place->x = v; relayout(); });
    BindLen(ctx, props.y, props.animate, [place, relayout](double v) { place->y = v; relayout(); });
    BindLen(ctx, props.width, props.animate, [place, relayout](double v) { place->width = v; relayout(); });
    BindLen(ctx, props.height, props.animate, [place, relayout](double v) { place->height = v; relayout(); });
    if (props.bleed && ctx.scene) {
        // The sides follow where the layer is going, on its motion.
        auto bleed = std::make_shared<Bleed>();
        bleed->motion = props.animate;
        for (auto& side : bleed->sides) side = Spring::Make(0, [relayout](double) { relayout(); });
        place->bleed = bleed;
        place->scene = ctx.scene;
        ctx.scene->layers.push_back(keep);
        auto on = props.bleed;
        std::array<Len, 4> frame{props.x, props.y, props.width, props.height};
        Reactive::Run([ctx, on, frame, bleed] {
            bool bleeds = R::Truthy(on(ctx));
            std::array<double, 4> target{};
            for (size_t i = 0; i < frame.size(); ++i) target[i] = frame[i].Now(ctx);
            Reactive::Untrack([&] {
                bleed->on = bleeds;
                bleed->target = target;
                bleed->Follow(false);
                return 0;
            });
        });
        Reactive::OnCleanup([bleed] {
            for (auto const& side : bleed->sides) Frames::Stop(side.get());
        });
    }
    // The layer's own layers are placed in its space.
    auto inner = ctx;
    inner.scene = nullptr;
    controls::Grid content;
    children(Parent::Aligned(content, xaml::HorizontalAlignment::Center, xaml::VerticalAlignment::Center), inner);
    panel.Children().Append(content);
    p.Add(panel);
}

// ---------------------------------------------------------------------------
// Marquees

/// A dark shadow under each text in `holder`, drawn by Composition from
/// the text's own shape, as a marquee's text runs over pictures.
inline void ShadowTexts(controls::Grid const& holder, controls::Canvas const& host) {
    auto hostVisual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(host);
    auto compositor = hostVisual.Compositor();
    auto layer = compositor.CreateContainerVisual();
    xaml::Hosting::ElementCompositionPreview::SetElementChildVisual(host, layer);
    auto refresh = [layer, compositor, weak = winrt::make_weak(holder)] {
        auto holder = weak.get();
        if (!holder) return;
        layer.Children().RemoveAll();
        std::function<void(xaml::DependencyObject const&)> visit = [&](xaml::DependencyObject const& node) {
            if (auto text = node.try_as<controls::TextBlock>()) {
                try {
                    auto at = text.TransformToVisual(holder).TransformPoint(winrt::Windows::Foundation::Point{0, 0});
                    auto shadow = compositor.CreateDropShadow();
                    shadow.Mask(text.GetAlphaMask());
                    shadow.Color(winrt::Windows::UI::Color{255, 0, 0, 0});
                    shadow.BlurRadius(1);
                    shadow.Offset(float3{0, 1, 0});
                    auto sprite = compositor.CreateSpriteVisual();
                    sprite.Size(float2{static_cast<float>(text.ActualWidth()), static_cast<float>(text.ActualHeight())});
                    sprite.Offset(float3{at.X, at.Y, 0});
                    sprite.Shadow(shadow);
                    layer.Children().InsertAtTop(sprite);
                } catch (winrt::hresult_error const&) {
                }
                return;
            }
            int32_t count = media::VisualTreeHelper::GetChildrenCount(node);
            for (int32_t i = 0; i < count; ++i) visit(media::VisualTreeHelper::GetChild(node, i));
        };
        visit(holder);
    };
    holder.SizeChanged([refresh](auto&&, auto&&) { refresh(); });
}

/// `Marquee`: its content at its natural width runs from beyond the right
/// edge to beyond the left one every `duration` seconds, starting `phase`
/// of the way; `paused` holds it where it is. With reduced motion it stands
/// still at 28% of the width.
struct MarqueePanel : controls::PanelT<MarqueePanel> {
    double duration = 8, phase = 0, elapsed = 0;
    float shift = 0;

    Size MeasureOverride(Size available) {
        Size content{0, 0};
        for (auto child : Children()) {
            child.Measure(Size{INFINITY, available.Height});
            content = child.DesiredSize();
        }
        return Size{std::isinf(available.Width) ? content.Width : available.Width, content.Height};
    }

    Size ArrangeOverride(Size final) {
        for (auto child : Children()) {
            auto size = child.DesiredSize();
            float progress = static_cast<float>(std::fmod(phase + elapsed / (std::max)(0.001, duration), 1.0));
            float x = Reduced() ? final.Width * 0.28f : final.Width - (final.Width + size.Width) * progress;
            child.Arrange(Rect{x, 0, size.Width, (std::max)(size.Height, final.Height)});
        }
        media::RectangleGeometry clip;
        clip.Rect(Rect{0, 0, final.Width, final.Height});
        Clip(clip);
        return final;
    }
};

struct MarqueeProps {
    Len duration;
    std::optional<double> phase;
    Get paused;
    Len opacity;
};

inline void Marquee(Parent const& p, Ctx const& ctx, MarqueeProps props, Build const& children) {
    auto impl = winrt::make_self<MarqueePanel>();
    impl->phase = props.phase.value_or(0);
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, L"bern-fill-width");
    MarqueePanel* marquee = impl.get();
    BindLen(ctx, props.duration, nullptr, [marquee](double v) { marquee->duration = (std::max)(0.05, v); });
    BindLen(ctx, props.opacity, nullptr, [panel](double v) { panel.Opacity((std::clamp)(v, 0.0, 1.0)); });
    // The content and the shadows of its text move together.
    controls::Grid mover;
    controls::Canvas host;
    host.IsHitTestVisible(false);
    controls::Grid holder;
    children(Parent::Of(holder), ctx);
    mover.Children().Append(host);
    mover.Children().Append(holder);
    ShadowTexts(holder, host);
    panel.Children().Append(mover);
    // Each frame while running moves the content.
    auto running = std::make_shared<bool>(false);
    auto token = std::make_shared<winrt::event_token>();
    auto last = std::make_shared<std::optional<int64_t>>();
    auto keep = winrt::make_weak(panel);
    auto stop = [running, token] {
        if (!*running) return;
        *running = false;
        media::CompositionTarget::Rendering(*token);
    };
    auto start = [running, token, last, marquee, keep] {
        if (*running) return;
        *running = true;
        last->reset();
        *token = media::CompositionTarget::Rendering([last, marquee, keep](auto&&, IInspectable const& args) {
            auto panel = keep.get();
            if (!panel) return;
            int64_t now = 0;
            if (auto rendering = args.try_as<media::RenderingEventArgs>()) now = rendering.RenderingTime().count();
            if (*last) marquee->elapsed += (std::min)(0.1, (now - **last) / 1e7);
            *last = now;
            panel.InvalidateArrange();
        });
    };
    auto paused = props.paused;
    Reactive::Run([ctx, paused, start, stop] {
        bool hold = paused && R::Truthy(paused(ctx));
        Reactive::Untrack([&] {
            if (hold || Reduced()) stop();
            else start();
            return 0;
        });
    });
    Reactive::OnCleanup(stop);
    p.Add(panel);
}

// ---------------------------------------------------------------------------
// Accordions

/// `Accordion`: columns side by side, one open into the width the others
/// leave; each closed column is `collapsedWidth` wide. A weight from 0 to
/// 1 per column follows the selection on its spring, so an interrupted
/// change keeps every column's width; the widths always fill the row. Each
/// column is clipped to `corner`.
struct AccordionPanel : controls::PanelT<AccordionPanel> {
    double collapsed = 86, gap = 8, height = 560, corner = 0;
    bool fillsHeight = false;
    std::vector<std::shared_ptr<Spring>> weights;
    /// Each column's rounded clip, sized as the column is arranged.
    std::vector<std::pair<winrt::weak_ref<xaml::UIElement>, composition::CompositionRoundedRectangleGeometry>> clips;

    std::vector<float> Widths(float total, size_t n) const {
        std::vector<float> widths(n, 0);
        if (n == 0) return widths;
        float available = (std::max)(0.0f, total - static_cast<float>(gap) * static_cast<float>(n - 1));
        float closed = (std::min)(static_cast<float>(collapsed), available / static_cast<float>(n));
        float extra = (std::max)(0.0f, available - closed * static_cast<float>(n));
        float sum = 0;
        for (size_t i = 0; i < n; ++i) {
            double weight = i < weights.size() ? weights[i]->value : 0;
            widths[i] = (std::max)(0.0f, closed + extra * static_cast<float>(weight));
            sum += widths[i];
        }
        // A spring overshooting zero still fits the row.
        if (sum > 0)
            for (auto& width : widths) width *= available / sum;
        return widths;
    }

    Size MeasureOverride(Size available) {
        size_t n = Children().Size();
        float total = std::isinf(available.Width) ? static_cast<float>(collapsed * n + gap * (n > 0 ? n - 1 : 0)) : available.Width;
        float measuredHeight = fillsHeight && std::isfinite(available.Height) ? available.Height : static_cast<float>(height);
        auto widths = Widths(total, n);
        for (uint32_t i = 0; i < n; ++i) Children().GetAt(i).Measure(Size{widths[i], measuredHeight});
        return Size{total, measuredHeight};
    }

    Size ArrangeOverride(Size final) {
        size_t n = Children().Size();
        float arrangedHeight = fillsHeight ? final.Height : static_cast<float>(height);
        auto widths = Widths(final.Width, n);
        float x = 0;
        for (uint32_t i = 0; i < n; ++i) {
            auto child = Children().GetAt(i);
            child.Arrange(Rect{x, 0, widths[i], arrangedHeight});
            x += widths[i] + static_cast<float>(gap);
            // Each column clipped to the corner at the size it has now.
            if (corner <= 0) continue;
            composition::CompositionRoundedRectangleGeometry geometry{nullptr};
            for (auto const& [one, made] : clips)
                if (one.get() == child) geometry = made;
            if (!geometry) {
                auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(child);
                geometry = visual.Compositor().CreateRoundedRectangleGeometry();
                visual.Clip(visual.Compositor().CreateGeometricClip(geometry));
                clips.emplace_back(winrt::make_weak(child), geometry);
            }
            float w = widths[i], h = arrangedHeight;
            float r = (std::min)({static_cast<float>(corner), w / 2, h / 2});
            geometry.Size(float2{w, h});
            geometry.CornerRadius(float2{r, r});
        }
        return final;
    }
};

struct AccordionProps {
    Len count;
    Len selected;
    Len collapsedWidth;
    Len gap;
    Len height;
    Len corner;
    Motion const* animate = nullptr;
};

inline void Accordion(Parent const& p, Ctx const& ctx, AccordionProps props, Build const& children) {
    auto impl = winrt::make_self<AccordionPanel>();
    controls::Panel panel = impl.as<controls::Panel>();
    impl->fillsHeight = !props.height.Given();
    SetRole(panel, impl->fillsHeight ? L"bern-fill" : L"bern-fill-width");
    AccordionPanel* accordion = impl.get();
    auto keep = winrt::make_weak(panel);
    auto relayout = [keep] {
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    };
    BindLen(ctx, props.collapsedWidth, nullptr, [accordion, relayout](double v) { accordion->collapsed = (std::max)(0.0, v); relayout(); });
    BindLen(ctx, props.gap, nullptr, [accordion, relayout](double v) { accordion->gap = (std::max)(0.0, v); relayout(); });
    BindLen(ctx, props.height, nullptr, [accordion, relayout](double v) { accordion->height = (std::max)(0.0, v); relayout(); });
    BindLen(ctx, props.corner, nullptr, [accordion](double v) { accordion->corner = (std::max)(0.0, v); });
    auto count = props.count, selected = props.selected;
    auto motion = props.animate;
    auto first = std::make_shared<bool>(true);
    Reactive::Run([ctx, count, selected, accordion, relayout, motion, first] {
        int n = static_cast<int>((std::max)(0.0, count.Now(ctx)));
        int chosen = static_cast<int>(selected.Now(ctx));
        Reactive::Untrack([&] {
            while (static_cast<int>(accordion->weights.size()) < n) accordion->weights.push_back(Spring::Make(0, [relayout](double) { relayout(); }));
            accordion->weights.resize(n);
            for (int i = 0; i < n; ++i) {
                double target = i == (std::clamp)(chosen, 0, (std::max)(0, n - 1)) ? 1.0 : 0.0;
                if (*first) accordion->weights[i]->Set(target);
                else accordion->weights[i]->To(target, motion);
            }
            *first = false;
            relayout();
            return 0;
        });
    });
    Reactive::OnCleanup([accordion] {
        for (auto const& weight : accordion->weights) Frames::Stop(weight.get());
    });
    children(Parent::Of(panel), ctx);
    p.Add(panel);
}

// ---------------------------------------------------------------------------
// Flowing rows

/// `Flow`: items at their own width from the leading edge, wrapping onto a
/// new row where the width runs out; each row as tall as its tallest, its
/// items centred in it, and a spacer taking the room left on its row.
struct FlowPanel : controls::PanelT<FlowPanel> {
    double gap = 8;

    /// Lines broken at the width, a spacer taking no room there; then the
    /// spacers of a line share the room left on it.
    template <class F> Size Lay(float width, F&& place) {
        struct Item {
            xaml::UIElement child;
            float width, height;
            bool spacer;
        };
        auto spacing = static_cast<float>(gap);
        std::vector<std::vector<Item>> lines(1);
        float x = 0;
        for (auto child : Children()) {
            if (child.Visibility() != xaml::Visibility::Visible) continue;
            bool spacer = RoleOf(Unwrapped(child)) == L"bern-spacer";
            auto size = child.DesiredSize();
            float w = spacer ? 0 : size.Width;
            if (!lines.back().empty() && x + w > width) {
                lines.emplace_back();
                x = 0;
            }
            lines.back().push_back(Item{child, w, spacer ? 0 : size.Height, spacer});
            x += w + spacing;
        }
        float y = 0, widest = 0;
        for (size_t n = 0; n < lines.size(); ++n) {
            auto const& line = lines[n];
            if (line.empty()) continue;
            float used = spacing * static_cast<float>(line.size() - 1), tall = 0;
            int spacers = 0;
            for (auto const& item : line) {
                used += item.width;
                tall = (std::max)(tall, item.height);
                spacers += item.spacer ? 1 : 0;
            }
            float share = spacers && std::isfinite(width) ? (std::max)(0.0f, width - used) / spacers : 0;
            float at = 0;
            for (auto const& item : line) {
                float w = item.spacer ? share : item.width;
                float h = item.spacer ? tall : item.height;
                place(item.child, Rect{at, y + (tall - h) / 2, w, h});
                at += w + spacing;
            }
            widest = (std::max)(widest, at - spacing);
            y += tall + (n + 1 < lines.size() ? spacing : 0);
        }
        return Size{widest, y};
    }

    Size MeasureOverride(Size available) {
        for (auto child : Children()) child.Measure(Size{available.Width, INFINITY});
        auto size = Lay(available.Width, [](auto const&, Rect) {});
        return Size{std::isinf(available.Width) ? size.Width : available.Width, size.Height};
    }

    Size ArrangeOverride(Size final) {
        Lay(final.Width, [](xaml::UIElement const& child, Rect rect) { child.Arrange(rect); });
        return final;
    }
};

struct FlowProps {
    Len gap;
};

inline void Flow(Parent const& p, Ctx const& ctx, FlowProps props, Build const& children) {
    auto impl = winrt::make_self<FlowPanel>();
    controls::Panel panel = impl.as<controls::Panel>();
    SetRole(panel, L"bern-fill-width");
    FlowPanel* flow = impl.get();
    BindLen(ctx, props.gap, nullptr, [flow, keep = winrt::make_weak(panel)](double v) {
        flow->gap = (std::max)(0.0, v);
        if (auto panel = keep.get()) panel.InvalidateMeasure();
    });
    auto items = Parent::Of(panel);
    items.flow = true;
    children(items, ctx);
    p.Add(panel);
}

inline controls::Panel MakeFlowPanel(double gap) {
    auto impl = winrt::make_self<FlowPanel>();
    impl->gap = gap;
    return impl.as<controls::Panel>();
}

// ---------------------------------------------------------------------------
// Modals

struct ModalProps {
    Len width;
    Len height;
    /// `cover: compact`: the whole window where it is compact.
    bool cover = false;
    /// Taken by the dialog's own entrance and exit.
    Transition const* appear = nullptr;
    Transition const* disappear = nullptr;
    std::optional<double> appearDelay;
};

/// Show a modal's dialog in the window's XAML root.
inline winrt::fire_and_forget ShowModal(winrt::weak_ref<controls::ContentDialog> weak, controls::Grid root, std::function<void()> size,
    std::shared_ptr<bool> closing) {
    auto dialog = weak.get();
    if (!dialog || *closing || !root.XamlRoot()) co_return;
    dialog.XamlRoot(root.XamlRoot());
    size();
    try {
        co_await dialog.ShowAsync();
    } catch (winrt::hresult_error const& error) {
        Report("modal: " + Utf8(error.message()));
    }
}

/// `Modal`: its content in the system's content dialog over the window,
/// which dims what lies below and takes no input there, with the dialog's
/// own entrance and exit. The dialog is the declared size within the
/// window; `cover: compact` fills a compact window. It stays while the
/// element is mounted: Escape does not close it.
inline void Modal(Parent const&, Ctx const& ctx, ModalProps props, Build const& children) {
    if (!ctx.win || !ctx.win->root) return;
    double width = props.width.Given() ? Reactive::Untrack([&] { return props.width.Now(ctx); }) : 960;
    double height = props.height.Given() ? Reactive::Untrack([&] { return props.height.Now(ctx); }) : 600;
    Win* win = ctx.win;
    auto root = win->root;
    controls::ContentDialog dialog;
    if (auto style = Styles::Named(L"DefaultContentDialogStyle")) dialog.Style(style);
    auto resources = dialog.Resources();
    resources.Insert(winrt::box_value(L"ContentDialogPadding"), winrt::box_value(xaml::Thickness{0, 0, 0, 0}));
    resources.Insert(winrt::box_value(L"ContentDialogMinWidth"), winrt::box_value(0.0));
    resources.Insert(winrt::box_value(L"ContentDialogMinHeight"), winrt::box_value(0.0));
    resources.Insert(winrt::box_value(L"ContentDialogMaxWidth"), winrt::box_value(100000.0));
    resources.Insert(winrt::box_value(L"ContentDialogMaxHeight"), winrt::box_value(100000.0));
    auto content = MakeFlex(false, 0);
    dialog.Content(content);
    children(Parent::Aligned(content, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
    // The declared size, within the window's edges, or the whole window.
    bool cover = props.cover;
    auto size = [weak = winrt::make_weak(dialog), content, root, win, width, height, cover] {
        auto dialog = weak.get();
        if (!dialog) return;
        double roomW = root.ActualWidth(), roomH = root.ActualHeight();
        bool covering = cover && win->size.Peek() == "compact";
        dialog.FullSizeDesired(covering);
        content.Width(covering ? roomW : (std::max)(0.0, (std::min)(width, roomW - 32)));
        content.Height(covering ? roomH : (std::max)(0.0, (std::min)(height, roomH - 32)));
    };
    auto sizeToken = root.SizeChanged([size](auto&&, auto&&) { size(); });
    Reactive::Run([win, size] {
        win->size.Get();
        Reactive::Untrack([&] {
            size();
            return 0;
        });
    });
    // It takes the window's theme and language, which a popup does not
    // inherit.
    dialog.RequestedTheme(root.ActualTheme());
    dialog.Language(root.Language());
    auto themeToken = root.ActualThemeChanged([weak = winrt::make_weak(dialog)](xaml::FrameworkElement const& sender, auto&&) {
        if (auto dialog = weak.get()) dialog.RequestedTheme(sender.ActualTheme());
    });
    auto closing = std::make_shared<bool>(false);
    dialog.Closing([closing](controls::ContentDialog const&, controls::ContentDialogClosingEventArgs const& args) {
        if (!*closing) args.Cancel(true);
    });
    // Shown once the window is in its XAML root.
    auto show = [weak = winrt::make_weak(dialog), root, size, closing] { ShowModal(weak, root, size, closing); };
    winrt::event_token loadedToken{};
    if (root.XamlRoot() && root.IsLoaded()) show();
    else {
        auto token = std::make_shared<winrt::event_token>();
        *token = root.Loaded([root, show, token](auto&&, auto&&) {
            root.Loaded(*token);
            show();
        });
        loadedToken = *token;
    }
    Reactive::OnCleanup([dialog, closing, root, sizeToken, themeToken, loadedToken] {
        *closing = true;
        root.SizeChanged(sizeToken);
        root.ActualThemeChanged(themeToken);
        if (loadedToken.value) root.Loaded(loadedToken);
        dialog.Hide();
    });
}

// ---------------------------------------------------------------------------
// Timers

struct TimerProps {
    std::optional<double> interval;
    Get enabled;
    Act action;
};

/// `Timer`: runs its action every `interval` seconds (at least 0.05) while
/// mounted and enabled; turning it off or leaving cancels the next one.
inline void Timer(Parent const&, Ctx const& ctx, TimerProps props) {
    auto generation = std::make_shared<int64_t>(0);
    auto every = std::chrono::milliseconds(static_cast<int64_t>((std::max)(0.05, props.interval.value_or(1)) * 1000));
    auto action = props.action;
    auto tick = std::make_shared<std::function<void(int64_t)>>();
    *tick = [generation, every, action, ctx, weak = std::weak_ptr<std::function<void(int64_t)>>(tick)](int64_t mine) {
        Host::After(every, [generation, action, ctx, weak, mine] {
            if (*generation != mine) return;
            if (action) action(ctx);
            if (auto next = weak.lock(); next && *generation == mine) (*next)(mine);
        });
    };
    auto enabled = props.enabled;
    Reactive::Run([ctx, enabled, generation, tick] {
        bool on = !enabled || R::Truthy(enabled(ctx));
        auto mine = ++*generation;
        if (on) (*tick)(mine);
    });
    Reactive::OnCleanup([generation, tick] {
        ++*generation;
        *tick = nullptr;
    });
}

// ---------------------------------------------------------------------------
// Control groups

struct ControlGroupProps {
    Str label;
    Len height;
};

/// `ControlGroup`: related controls side by side, as a command bar shows
/// its commands: plain buttons without their own backgrounds and no
/// surface of the group's (Fluent groups by placement), 40 points tall
/// unless `height` says.
inline void ControlGroup(Parent const& p, Ctx const& ctx, ControlGroupProps props, Build const& children) {
    auto row = MakeFlex(true, 2);
    Styles::Restyle(row, Styles::SubtleXaml());
    row.HorizontalAlignment(xaml::HorizontalAlignment::Left);
    row.VerticalAlignment(xaml::VerticalAlignment::Center);
    row.Height(40);
    BindLen(ctx, props.height, nullptr, [row](double v) { row.Height((std::max)(0.0, v)); });
    BindName(row, Bound(ctx, props.label));
    children(Parent::Aligned(row, std::nullopt, xaml::VerticalAlignment::Center), ctx);
    p.Add(row);
}

// ---------------------------------------------------------------------------
// The search palette

/// One result the arrow keys move to: where it is on screen and what
/// choosing it does.
struct PaletteEntry {
    winrt::weak_ref<xaml::FrameworkElement> row;
    std::function<void()> choose;
};

/// An open palette: its results, the one the keys stand at, and how it
/// closes.
struct PaletteState {
    std::map<int64_t, PaletteEntry> entries;
    int64_t next = 1;
    std::optional<int64_t> current;
    std::function<void()> dismiss;
    Version version;

    std::vector<int64_t> Ordered() const {
        std::vector<std::pair<float, int64_t>> tops;
        for (auto const& [token, entry] : entries)
            if (auto row = entry.row.get()) {
                float top = 0;
                try {
                    top = row.TransformToVisual(nullptr).TransformPoint(winrt::Windows::Foundation::Point{0, 0}).Y;
                } catch (winrt::hresult_error const&) {
                }
                tops.emplace_back(top, token);
            }
        std::sort(tops.begin(), tops.end());
        std::vector<int64_t> order;
        for (auto const& [top, token] : tops) order.push_back(token);
        return order;
    }

    void Move(int step) {
        auto order = Ordered();
        if (order.empty()) return;
        auto at = current ? std::find(order.begin(), order.end(), *current) : order.end();
        if (at == order.end()) current = step > 0 ? order.front() : order.back();
        else current = order[(std::clamp)(static_cast<int>(at - order.begin()) + step, 0, static_cast<int>(order.size()) - 1)];
        version.Bump();
        if (auto row = entries[*current].row.get()) row.StartBringIntoView();
    }

    /// Choose the current result; false when there is none.
    bool Choose() {
        if (!current) return false;
        auto found = entries.find(*current);
        if (found == entries.end()) return false;
        found->second.choose();
        return true;
    }
};

/// The palette whose results are being built.
inline std::shared_ptr<PaletteState> currentPalette;

/// `Search(style: palette)`: the system's search box, which grows into the
/// search over the window (design 10.8, T17): the panel moves out of the
/// box to its place on the hero spring, its height follows the results on
/// the bouncy spring, and it goes back into the box on the snappy one. The
/// panel is in the flyout's material over the dimmed page, `width` wide
/// and at most `height` tall; the field row (the glass, the native text
/// box, `esc`) on top, the results below, built only while it is open. The
/// arrow keys move among the results, Return chooses the current one or
/// else submits, Escape, `esc` or a click outside close it. External
/// entries open it with `activate` or `focus`.
inline void SearchPalette(Parent const& p, Ctx const& ctx, SearchProps props, Build const& results) {
    if (!props.field || !ctx.win) return;
    auto field = *props.field;
    auto prompt = Bound(ctx, props.prompt);
    auto panelPrompt = props.panelPrompt ? Bound(ctx, props.panelPrompt) : prompt;
    // The system's search box where it stands; the palette opens over it
    // and grows down from it, as the design's search box expands in place.
    controls::AutoSuggestBox entry;
    entry.QueryIcon(controls::SymbolIcon(controls::Symbol::Find));
    entry.Width(ctx.toolbar ? 240 : 280);
    if (prompt)
        Reactive::Run([entry, prompt] {
            auto name = prompt();
            entry.PlaceholderText(name);
            xaml::Automation::AutomationProperties::SetName(entry, name);
        });
    double width = props.width.value_or(640), height = props.height.value_or(480);
    auto submit = Run(ctx, props.submit);
    auto grow = props.grow, follow = props.follow ? props.follow : props.grow, shrink = props.shrink ? props.shrink : props.grow;
    Win* win = ctx.win;
    std::weak_ptr<Page> page = ctx.page;
    // Fluent's control and overlay corners; square in the system XAML.
    double fieldCorner = kWinUI ? 4 : 0, panelCorner = kWinUI ? 8 : 0;
    struct Open {
        controls::Grid layer{nullptr};
        controls::Border dim{nullptr};
        controls::Border panel{nullptr};
        controls::Grid content{nullptr};
        xaml::FrameworkElement list{nullptr};
        std::unique_ptr<Scope> scope;
        std::shared_ptr<PaletteState> state;
        /// How far the panel has grown out of the field (0 to 1), and the
        /// height its results ask for.
        std::shared_ptr<Spring> progress, tall;
        /// The field's rectangle and the panel's: left, top, width, height
        /// (the panel's most) and corner, in the layer.
        std::array<double, 5> from{}, to{};
        bool started = false, closing = false;
    };
    auto open = std::make_shared<Open>();
    auto close = std::make_shared<std::function<void()>>();
    // The panel where its springs stand: between the field and its place,
    // as tall as its results; its content shows once it has mostly grown
    // and the page dims with it.
    auto place = [open] {
        if (!open->panel) return;
        double t = open->progress ? open->progress->value : 0;
        auto mix = [t](double a, double b) { return a + (b - a) * t; };
        double tall = open->tall ? open->tall->value : open->to[3];
        double w = (std::max)(1.0, mix(open->from[2], open->to[2])), h = (std::max)(1.0, mix(open->from[3], tall));
        double r = (std::max)(0.0, mix(open->from[4], open->to[4]));
        controls::Canvas::SetLeft(open->panel, mix(open->from[0], open->to[0]));
        controls::Canvas::SetTop(open->panel, mix(open->from[1], open->to[1]));
        open->panel.Width(w);
        open->panel.Height(h);
        open->panel.CornerRadius(xaml::CornerRadius{r, r, r, r});
        media::RectangleGeometry clip;
        clip.Rect(winrt::Windows::Foundation::Rect{0, 0, static_cast<float>(w), static_cast<float>(h)});
        open->panel.Clip(clip);
        if (open->content) open->content.Opacity(open->closing ? 0.0 : (std::clamp)(t * 1.6 - 0.5, 0.0, 1.0));
        if (open->dim) open->dim.Opacity((std::clamp)(t, 0.0, 1.0));
    };
    // Where the field stands and where the panel goes: as wide as declared
    // (16 from the window's edges at most), centered over the column of the
    // page header the field stands in, 26 under the field as the design's
    // panel stands under its toolbar, and at most as tall as declared.
    auto measure = [open, page, width, height, fieldCorner, panelCorner, weakEntry = winrt::make_weak(entry)] {
        auto entry = weakEntry.get();
        if (!entry || !open->layer) return;
        double W = open->layer.ActualWidth(), H = open->layer.ActualHeight();
        winrt::Windows::Foundation::Rect field{0, 0, static_cast<float>(entry.ActualWidth()), static_cast<float>(entry.ActualHeight())};
        try {
            field = entry.TransformToVisual(open->layer).TransformBounds(field);
        } catch (winrt::hresult_error const&) {
        }
        open->from = {field.X, field.Y, field.Width, field.Height, fieldCorner};
        double w = (std::min)(width, (std::max)(280.0, W - 32));
        double center = W / 2;
        if (auto shown = page.lock(); shown && shown->tools)
            if (auto header = shown->tools.Parent().try_as<xaml::FrameworkElement>()) {
                try {
                    auto box = header.TransformToVisual(open->layer)
                                   .TransformBounds({0, 0, static_cast<float>(header.ActualWidth()), static_cast<float>(header.ActualHeight())});
                    center = box.X + box.Width / 2;
                } catch (winrt::hresult_error const&) {
                }
            }
        center = (std::clamp)(center, w / 2 + 16, (std::max)(w / 2 + 16, W - w / 2 - 16));
        double top = (std::clamp)(static_cast<double>(field.Y + field.Height) + 26, 16.0, (std::max)(16.0, H * 0.3));
        open->to = {center - w / 2, top, w, (std::min)(height, (std::max)(120.0, H - top - 16)), panelCorner};
    };
    // The height the results ask for: the field row, its rule and the list.
    auto wanted = [open] {
        double list = open->list ? open->list.ActualHeight() : 0;
        return (std::min)(open->to[3], 57 + list);
    };
    *close = [open, place, shrink, weakEntry = winrt::make_weak(entry)] {
        if (!open->layer || open->closing) return;
        open->closing = true;
        auto finish = [open, weakEntry] {
            if (auto layer = open->layer)
                if (auto parent = layer.Parent().try_as<controls::Panel>()) {
                    uint32_t at = 0;
                    if (parent.Children().IndexOf(layer, at)) parent.Children().RemoveAt(at);
                }
            if (open->scope) open->scope->Dispose();
            open->scope.reset();
            open->state = nullptr;
            open->layer = nullptr;
            open->dim = nullptr;
            open->panel = nullptr;
            open->content = nullptr;
            open->list = nullptr;
            open->progress = open->tall = nullptr;
            open->started = open->closing = false;
            if (auto entry = weakEntry.get()) entry.Opacity(1);
        };
        if (!open->started || !open->progress) {
            finish();
            return;
        }
        place();
        open->progress->To(0, shrink, finish);
    };
    auto show = [open, close, place, measure, wanted, win, ctx, field, panelPrompt, results, submit, grow, follow, weakEntry = winrt::make_weak(entry)] {
        auto entry = weakEntry.get();
        if (open->layer || !entry || !entry.XamlRoot() || !win) return;
        auto host = win->host ? win->host : win->root;
        if (!host) return;
        // What the open palette runs belongs to its own scope, ended when
        // it closes.
        open->scope = std::make_unique<Scope>();
        auto state = std::make_shared<PaletteState>();
        std::weak_ptr<std::function<void()>> weakClose = close;
        state->dismiss = [weakClose] {
            if (auto close = weakClose.lock()) (*close)();
        };
        open->state = state;
        // Over the window's content: the page dims, a click on it closes.
        controls::Grid layer;
        controls::Border dim;
        dim.Background(media::SolidColorBrush(Rgb(0x000000, IsDark(host) ? 82 : 51)));
        dim.Opacity(0);
        dim.Tapped([weakClose](auto&&, auto&&) {
            if (auto close = weakClose.lock()) (*close)();
        });
        layer.Children().Append(dim);
        controls::Canvas stage;
        layer.Children().Append(stage);
        // The panel in the platform's overlay material, as a flyout is.
        controls::Border panel;
        if (auto style = Styles::Themed(L"Border", {{L"Background", kWinUI ? L"AcrylicInAppFillColorDefaultBrush" : L"SystemControlTransientBackgroundBrush"},
                                                       {L"BorderBrush", kWinUI ? L"SurfaceStrokeColorFlyoutBrush" : L"SystemControlTransientBorderBrush"}}))
            panel.Style(style);
        panel.BorderThickness(xaml::Thickness{1, 1, 1, 1});
        panel.Opacity(0);
        media::ThemeShadow shadow;
        shadow.Receivers().Append(dim);
        panel.Shadow(shadow);
        panel.Translation({0.0f, 0.0f, 32.0f});
        stage.Children().Append(panel);
        // At the panel's full width from the start; the panel's growing
        // edges uncover it.
        controls::Grid body;
        body.HorizontalAlignment(xaml::HorizontalAlignment::Left);
        body.VerticalAlignment(xaml::VerticalAlignment::Stretch);
        for (auto rowHeight : {56.0, 1.0}) {
            controls::RowDefinition row;
            row.Height(xaml::GridLength{rowHeight, xaml::GridUnitType::Pixel});
            body.RowDefinitions().Append(row);
        }
        controls::RowDefinition rest;
        rest.Height(Star());
        body.RowDefinitions().Append(rest);
        // The field row (`.pin`): the glass, the native text box, and `esc`.
        controls::Grid head;
        head.Padding(xaml::Thickness{18, 0, 10, 0});
        for (auto star : {false, true, false}) {
            controls::ColumnDefinition column;
            column.Width(star ? Star() : AutoLength());
            head.ColumnDefinitions().Append(column);
        }
        controls::FontIcon glass;
        glass.Glyph(L"");
        glass.FontSize(16);
        glass.VerticalAlignment(xaml::VerticalAlignment::Center);
        if (auto style = Styles::Themed(L"FontIcon", {{L"Foreground", Keys::SecondaryText}})) glass.Style(style);
        head.Children().Append(glass);
        controls::TextBox box;
        box.Margin(xaml::Thickness{12, 0, 8, 0});
        box.VerticalAlignment(xaml::VerticalAlignment::Center);
        controls::Grid::SetColumn(box, 1);
        head.Children().Append(box);
        controls::Button escape;
        Styles::MakeSubtle(escape);
        controls::TextBlock escapeText;
        escapeText.Text(L"esc");
        escapeText.FontSize(12);
        if (auto style = Styles::Themed(L"TextBlock", {{L"Foreground", Keys::SecondaryText}})) escapeText.Style(style);
        escape.Content(escapeText);
        escape.Padding(xaml::Thickness{8, 4, 8, 4});
        escape.VerticalAlignment(xaml::VerticalAlignment::Center);
        xaml::Automation::AutomationProperties::SetName(escape, bern::Text::Tr("bern.window.close"));
        escape.Click([weakClose](auto&&, auto&&) {
            if (auto close = weakClose.lock()) (*close)();
        });
        controls::Grid::SetColumn(escape, 2);
        head.Children().Append(escape);
        body.Children().Append(head);
        controls::Border rule;
        if (auto style = Styles::Themed(L"Border", {{L"Background", Keys::Divider}})) rule.Style(style);
        controls::Grid::SetRow(rule, 1);
        body.Children().Append(rule);
        controls::ScrollViewer scroller;
        scroller.HorizontalScrollBarVisibility(controls::ScrollBarVisibility::Disabled);
        scroller.VerticalScrollBarVisibility(controls::ScrollBarVisibility::Auto);
        controls::Grid::SetRow(scroller, 2);
        auto list = MakeFlex(false, 2, xaml::Thickness{6, 6, 6, 6});
        scroller.Content(list);
        body.Children().Append(scroller);
        panel.Child(body);
        open->layer = layer;
        open->dim = dim;
        open->panel = panel;
        open->content = body;
        open->list = list.as<xaml::FrameworkElement>();
        open->scope->Run([&] {
            if (panelPrompt) Reactive::Run([box, panelPrompt] { box.PlaceholderText(panelPrompt()); });
            auto updating = Editable([box] { return box.Text(); }, [box](auto const& v) { box.Text(v); }, ctx, field);
            box.TextChanged([weak = winrt::make_weak(box), updating, ctx, field](auto&&, auto&&) {
                if (auto box = weak.get(); box && !*updating) SetDraft(ctx, field, Value::Text(Utf8(box.Text())));
            });
            auto saved = currentPalette;
            currentPalette = state;
            if (results) results(Parent::Aligned(list, xaml::HorizontalAlignment::Stretch, std::nullopt), ctx);
            currentPalette = saved;
        });
        // The keys move among the results and choose; Escape closes.
        body.PreviewKeyDown([state, submit, weakClose](auto&&, xaml::Input::KeyRoutedEventArgs const& args) {
            using winrt::Windows::System::VirtualKey;
            switch (args.Key()) {
            case VirtualKey::Down: state->Move(1); args.Handled(true); break;
            case VirtualKey::Up: state->Move(-1); args.Handled(true); break;
            case VirtualKey::Enter:
                if (!state->Choose() && submit) submit();
                args.Handled(true);
                break;
            case VirtualKey::Escape:
                if (auto close = weakClose.lock()) (*close)();
                args.Handled(true);
                break;
            default: break;
            }
        });
        // Once laid out: from the field to its place on the hero spring,
        // as tall as the results at the panel's width; then the height
        // follows them on the bouncy spring, and a resized window moves it.
        layer.SizeChanged([open, place, measure, wanted, grow, weakBox = winrt::make_weak(box), weakEntry](auto&&, auto&&) {
            if (!open->layer || open->closing) return;
            measure();
            open->content.Width(open->to[2]);
            if (open->started) {
                if (open->tall) open->tall->Set(wanted());
                place();
                return;
            }
            open->layer.UpdateLayout();
            open->started = true;
            open->progress = Spring::Make(0, [place](double) { place(); });
            open->tall = Spring::Make(wanted(), [place](double) { place(); });
            place();
            open->panel.Opacity(1);
            if (auto entry = weakEntry.get()) entry.Opacity(0);
            open->progress->To(1, grow);
            if (auto box = weakBox.get()) {
                box.Focus(xaml::FocusState::Programmatic);
                box.SelectionStart(static_cast<int32_t>(box.Text().size()));
            }
        });
        list.as<xaml::FrameworkElement>().SizeChanged([open, wanted, follow](auto&&, auto&&) {
            if (open->started && !open->closing && open->tall) open->tall->To(wanted(), follow);
        });
        host.Children().Append(layer);
    };
    // Pointing at the box, its search button or typing in it opens the
    // palette with the text. The box's own text field marks the tap handled,
    // so the handler asks for handled taps too.
    entry.AddHandler(xaml::UIElement::TappedEvent(), winrt::box_value(xaml::Input::TappedEventHandler([show](auto&&, auto&&) { show(); })), true);
    entry.QuerySubmitted([show](auto&&, auto&&) { show(); });
    entry.TextChanged([show, ctx, field](controls::AutoSuggestBox const& sender, controls::AutoSuggestBoxTextChangedEventArgs const& args) {
        if (args.Reason() != controls::AutoSuggestionBoxTextChangeReason::UserInput || sender.Text().empty()) return;
        SetDraft(ctx, field, Value::Text(Utf8(sender.Text())));
        sender.Text(L"");
        show();
    });
    Reactive::OnCleanup([close] { (*close)(); });
    Controls::Register(ctx, props.id, "search", prompt ? prompt : TextFn([] { return winrt::hstring(); }), nullptr,
        Actions{.activate = show,
            .edit = [ctx, field](std::string const& value) { SetDraft(ctx, field, Value::Text(value)); },
            .submit = submit,
            .value = [ctx, field, open] { return std::optional<std::string>((open->layer ? "open: " : "") + R::AsText(ctx.Peek(field))); },
            .focus = show});
    p.Add(entry);
}

struct SearchItemProps {
    char const* id = "";
    Str label;
    char const* icon = nullptr;
    Str detail;
    Str hint;
    /// Whether the arrow keys stop at it; they do unless it says.
    Get navigation;
    Act action;
    std::optional<Field> pick;
    char const* files = nullptr;
};

/// `SearchItem`: a result row with its icon, label, detail and hint, or the
/// content given instead. Choosing it closes the palette it stands in and
/// then runs its action, for the user and for external entries alike; the
/// result the keys stand at is highlighted.
inline void SearchItem(Parent const& p, Ctx const& ctx, SearchItemProps props, Build const& content) {
    auto palette = currentPalette;
    auto run = RunOf(ctx, props.pick, props.files, props.action);
    std::weak_ptr<PaletteState> weak = palette;
    auto choose = [weak, activate = run.activate] {
        if (auto palette = weak.lock(); palette && palette->dismiss) palette->dismiss();
        if (activate) activate();
    };
    controls::Button row;
    Styles::MakeSubtle(row);
    row.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    row.HorizontalContentAlignment(xaml::HorizontalAlignment::Stretch);
    row.Padding(xaml::Thickness{10, 6, 10, 6});
    SetRole(row, L"bern-fill-width");
    auto text = Bound(ctx, props.label);
    if (content) {
        auto holder = MakeFlex();
        content(Parent::Of(holder), ctx);
        row.Content(holder);
    } else {
        controls::Grid line;
        line.ColumnSpacing(12);
        line.ColumnDefinitions().Append(Column(AutoLength()));
        line.ColumnDefinitions().Append(Column(Star()));
        line.ColumnDefinitions().Append(Column(AutoLength()));
        if (props.icon) {
            auto icon = Icons::Icon(props.icon);
            icon.VerticalAlignment(xaml::VerticalAlignment::Center);
            line.Children().Append(icon);
        }
        controls::StackPanel words;
        controls::Grid::SetColumn(words, 1);
        auto title = TextOf(text);
        title.TextWrapping(xaml::TextWrapping::NoWrap);
        title.TextTrimming(xaml::TextTrimming::CharacterEllipsis);
        words.Children().Append(title);
        if (props.detail) {
            auto detail = TextOf(Bound(ctx, props.detail));
            detail.TextWrapping(xaml::TextWrapping::NoWrap);
            detail.TextTrimming(xaml::TextTrimming::CharacterEllipsis);
            if (auto style = Styles::TextStyle("caption")) detail.Style(style);
            Secondary(detail);
            words.Children().Append(detail);
        }
        line.Children().Append(words);
        if (props.hint) {
            auto hint = TextOf(Bound(ctx, props.hint));
            hint.VerticalAlignment(xaml::VerticalAlignment::Center);
            Secondary(hint);
            controls::Grid::SetColumn(hint, 2);
            line.Children().Append(hint);
        }
        row.Content(line);
    }
    BindName(row, text);
    row.Click([choose](auto&&, auto&&) { choose(); });
    bool navigation = !props.navigation || Reactive::Untrack([&] { return R::Truthy(props.navigation(ctx)); });
    if (palette && navigation) {
        auto token = palette->next++;
        palette->entries[token] = PaletteEntry{winrt::make_weak(row.as<xaml::FrameworkElement>()), choose};
        // The pointer over a result makes it the current one.
        row.PointerEntered([weak, token](auto&&, auto&&) {
            if (auto palette = weak.lock()) {
                palette->current = token;
                palette->version.Bump();
            }
        });
        Reactive::Run([weak, token, row] {
            auto palette = weak.lock();
            if (!palette) return;
            palette->version.Track();
            bool current = palette->current == token;
            Reactive::Untrack([&] {
                if (current) {
                    if (auto style = Styles::Themed(L"Button", {{L"Background", kWinUI ? L"SubtleFillColorSecondaryBrush" : L"SystemControlBackgroundListLowBrush"}}))
                        row.Style(style);
                } else {
                    row.ClearValue(xaml::FrameworkElement::StyleProperty());
                }
                return 0;
            });
        });
        Reactive::OnCleanup([weak, token] {
            if (auto palette = weak.lock()) {
                palette->entries.erase(token);
                if (palette->current == token) palette->current.reset();
            }
        });
    }
    Controls::Register(ctx, props.id, "button", text, nullptr,
        Actions{.activate = choose,
            .setFile = run.give ? std::function<void(std::string const&)>([weak, give = run.give](std::string const& url) {
                if (auto palette = weak.lock(); palette && palette->dismiss) palette->dismiss();
                give(url);
            })
                                : nullptr,
            .files = props.files});
    p.Add(row);
}

}
