// Bern runtime for Windows, XAML part 2 of 7: motion, theme colors and
// style overrides. Springs step once per frame on the UI thread, as on the
// other targets, so a new target continues from the current value and
// velocity; an element's transform is the sum of the moves and the product
// of the scales and opacities of independent layers (a style's state, an
// appearing transition, a shared element, reflow, a nudge), written to the
// element's Translation, Scale and Opacity. Effects that follow scrolling
// run as Composition expression animations. Generated build input; do not
// edit.

#if BERN_WINUI
namespace composition = winrt::Microsoft::UI::Composition;
using PointerKind = winrt::Microsoft::UI::Input::PointerDeviceType;
#else
namespace composition = winrt::Windows::UI::Composition;
using PointerKind = winrt::Windows::Devices::Input::PointerDeviceType;
#endif
using float3 = winrt::Windows::Foundation::Numerics::float3;
using float2 = winrt::Windows::Foundation::Numerics::float2;

namespace B {

// ---------------------------------------------------------------------------
// Springs

/// Elements holding `reduce_motion: true` while shown, by token: the whole
/// application then runs with reduced motion.
inline std::map<int64_t, bool> reduceHolders;
inline int64_t nextReduceHolder = 1;

/// Whether motion is reduced: the system asks for less motion, or the
/// application does (`reduce_motion`). Springs then settle at once and
/// transitions become a short fade.
inline bool Reduced() {
    for (auto const& [token, on] : reduceHolders)
        if (on) return true;
    static winrt::Windows::UI::ViewManagement::UISettings settings;
    try {
        return !settings.AnimationsEnabled();
    } catch (...) {
        return false;
    }
}

struct Spring;

/// Springs being driven, stepped together once per rendered frame.
namespace Frames {
inline std::vector<std::shared_ptr<Spring>> running;
inline std::optional<int64_t> last;
inline bool subscribed = false;
inline winrt::event_token token;
inline void Start(std::shared_ptr<Spring> const& spring);
inline void Stop(Spring const* spring);
/// True while the frame's springs step: what they move is written once
/// after all have stepped, not once per spring.
inline bool stepping = false;
inline std::vector<std::function<void()>> pending;
/// Run `write` after this frame's springs have stepped, or now outside a
/// frame.
inline void AfterStep(std::function<void()> write) {
    if (stepping) pending.push_back(std::move(write));
    else write();
}

/// `BERN_FRAME_LOG`: how each run of springs went, on stderr — how long
/// the first frame took to come, the frames' spacing, the slow ones and
/// the time the springs themselves took.
inline bool Logging() {
    static bool const on = [] {
        char* value = nullptr;
        size_t size = 0;
        bool set = _dupenv_s(&value, &size, "BERN_FRAME_LOG") == 0 && value && *value;
        std::free(value);
        return set;
    }();
    return on;
}
struct Run {
    std::chrono::steady_clock::time_point started, previous;
    int64_t frames = 0;
    int slow = 0;
    double first = 0, total = 0, worst = 0, stepping = 0;
    size_t most = 0;
    /// The UI thread's own CPU cycles in the run, and in its busiest
    /// frame, in millions: what the window did, whatever else ran.
    double cycles = 0, busiest = 0;
};
inline Run run;
inline double Since(std::chrono::steady_clock::time_point at) {
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - at).count();
}
inline std::chrono::steady_clock::time_point const origin = std::chrono::steady_clock::now();
/// The UI thread's CPU cycles so far, in millions.
inline double Cycles() {
    ULONG64 cycles = 0;
    QueryThreadCycleTime(GetCurrentThread(), &cycles);
    return static_cast<double>(cycles) / 1e6;
}
/// While logging, every frame is watched: a frame that comes more than 50
/// ms after the one before says how long the window stood still, such as
/// while a page was built.
inline void Watch() {
    static bool watching = false;
    if (watching || !Logging()) return;
    watching = true;
    static auto previous = std::chrono::steady_clock::now();
    static double spent = Cycles();
    media::CompositionTarget::Rendering([](auto&&, auto&&) {
        double gap = Since(previous);
        previous = std::chrono::steady_clock::now();
        double now = Cycles(), used = now - spent;
        spent = now;
        if (subscribed) {
            run.cycles += used;
            run.busiest = (std::max)(run.busiest, used);
        }
        static int64_t parsed = 0;
        static double parsing = 0, working = 0;
        double worked = Host::working - working;
        working = Host::working;
        if (gap <= 50) {
            parsed = Styles::parsed;
            parsing = Styles::parsing;
            return;
        }
        // What is not posted work is the framework's: layout, text and the
        // frame itself.
        char line[288];
        std::snprintf(line, sizeof line, "stall: %.0f ms, until %.2f s, UI thread %.0f Mcycles; %.0f ms posted work, %lld dictionaries parsed in %.0f ms meanwhile", gap,
            Since(origin) / 1000, used, worked, static_cast<long long>(Styles::parsed - parsed), Styles::parsing - parsing);
        parsed = Styles::parsed;
        parsing = Styles::parsing;
        Report(line);
    });
}
}

/// A value that moves to its target as a spring of period `T` and damping
/// ratio `ζ` (mass 1): stiffness (2π / T)², damping 4πζ / T.
struct Spring : std::enable_shared_from_this<Spring> {
    double value = 0, target = 0, velocity = 0;
    double stiffness = 0, damping = 0, epsilon = 1e-4;
    /// Seconds of frames to wait before moving, for staggered starts.
    double wait = 0;
    std::function<void(double)> apply;
    std::function<void()> done;

    static std::shared_ptr<Spring> Make(double initial, std::function<void(double)> apply) {
        auto spring = std::make_shared<Spring>();
        spring->value = spring->target = initial;
        spring->apply = std::move(apply);
        return spring;
    }

    /// Jump to `next`, ending any motion.
    void Set(double next) {
        Frames::Stop(this);
        value = target = next;
        velocity = 0;
        wait = 0;
        if (apply) apply(value);
        auto finished = std::move(done);
        done = nullptr;
        if (finished) finished();
    }

    void To(double next, Motion const* motion, std::function<void()> finished = nullptr, double delay = 0) {
        auto previous = std::move(done);
        done = nullptr;
        if (previous) previous();
        wait = delay;
        if (!motion || Reduced()) {
            target = next;
            Set(next);
            if (finished) finished();
            return;
        }
        double period = (std::max)(0.01, motion->period);
        stiffness = std::pow(2 * 3.14159265358979 / period, 2);
        damping = 4 * 3.14159265358979 * motion->damping / period;
        target = next;
        done = std::move(finished);
        epsilon = (std::max)(1e-4, std::abs(next - value) * 1e-4);
        if (std::abs(next - value) < epsilon && std::abs(velocity) < epsilon && wait <= 0) {
            Set(next);
            return;
        }
        Frames::Start(shared_from_this());
    }

    /// Give the spring a push, as a nudge does.
    void Kick(double push, Motion const* motion) {
        velocity += push;
        auto finished = std::move(done);
        done = nullptr;
        To(target, motion, std::move(finished));
    }

    /// Advance by `elapsed` seconds; false once it has settled.
    bool Step(double elapsed) {
        if (wait > 0) {
            wait -= elapsed;
            return true;
        }
        double left = elapsed;
        while (left > 0) {
            double h = (std::min)(left, 1.0 / 240);
            double force = -stiffness * (value - target) - damping * velocity;
            velocity += force * h;
            value += velocity * h;
            left -= h;
        }
        if (std::abs(value - target) < epsilon && std::abs(velocity) < epsilon * 10) return false;
        if (apply) apply(value);
        return true;
    }
};

inline void Frames::Stop(Spring const* spring) {
    std::erase_if(running, [spring](auto const& one) { return one.get() == spring; });
}

inline void Frames::Start(std::shared_ptr<Spring> const& spring) {
    for (auto const& one : running)
        if (one == spring) return;
    running.push_back(spring);
    if (subscribed) return;
    subscribed = true;
    last.reset();
    if (Logging()) {
        Watch();
        run = Run{std::chrono::steady_clock::now(), std::chrono::steady_clock::now()};
    }
    token = media::CompositionTarget::Rendering([](auto&&, IInspectable const& args) {
        int64_t now = 0;
        if (auto rendering = args.try_as<media::RenderingEventArgs>()) now = rendering.RenderingTime().count();
        double elapsed = last ? (std::min)(0.064, (now - *last) / 1e7) : 1.0 / 60;
        if (elapsed <= 0) elapsed = 1.0 / 60;
        last = now;
        bool logging = Logging();
        auto began = std::chrono::steady_clock::now();
        if (logging) {
            double gap = Since(run.previous);
            if (run.frames == 0) run.first = gap;
            else {
                run.total += gap;
                run.worst = (std::max)(run.worst, gap);
                if (gap > 25) ++run.slow;
            }
            run.previous = began;
            ++run.frames;
            run.most = (std::max)(run.most, running.size());
        }
        auto current = running;
        stepping = true;
        for (auto const& spring : current) {
            // Settled springs land on their target and say they are done.
            if (!spring->Step(elapsed)) spring->Set(spring->target);
        }
        stepping = false;
        // Then each thing they move is written once.
        auto writes = std::move(pending);
        pending.clear();
        for (auto const& write : writes) write();
        if (logging) run.stepping += Since(began);
        if (running.empty() && subscribed) {
            subscribed = false;
            media::CompositionTarget::Rendering(token);
            if (logging) {
                char line[384];
                std::snprintf(line, sizeof line,
                    "frames: %lld in %.0f ms until %.2f s, first after %.0f ms, spacing %.1f ms on average, worst %.0f ms, %d over 25 ms; %zu springs at most, %.1f ms stepping them; UI thread %.0f Mcycles, %.1f in the busiest frame",
                    static_cast<long long>(run.frames), Since(run.started), Since(origin) / 1000, run.first, run.frames > 1 ? run.total / (run.frames - 1) : 0.0,
                    run.worst, run.slow, run.most, run.stepping, run.cycles, run.busiest);
                Report(line);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Layers of an element's transform

/// One layer: a move, a scale and an opacity, each its own spring. With a
/// `pivot` (fractions of the element's size) its scale holds that point
/// still, as a transition growing from an edge does.
struct Layer {
    std::shared_ptr<Spring> x, y, sx, sy, opacity;
    std::optional<float2> pivot;
};

/// The layers of one element, combined into its Translation, Scale (about
/// its center) and Opacity.
struct LayerStack : winrt::implements<LayerStack, IInspectable> {
    winrt::weak_ref<xaml::UIElement> node;
    std::vector<std::pair<std::string, std::shared_ptr<Layer>>> layers;
    double baseOpacity = 1;
    /// Whether the element's own Composition visual moves, for controls
    /// that already use it themselves and so refuse Translation and Scale.
    bool handoff = false;
    /// For a navigation view item: its scale holds the line of its
    /// selection pill still, so the pill keeps to the track it slides along
    /// from item to item while a row grows or shrinks.
    bool alongPill = false;
    winrt::weak_ref<xaml::UIElement> pillTrack;
    /// While true the element shows no move or scale, whatever its layers
    /// say: see `Rest`.
    bool resting = false;

    /// Whether a write waits for the end of the frame's stepping.
    bool queued = false;
    /// What was written last, so an unchanged value is not set again. Code
    /// that sets the element's (or its visual's) transform or opacity
    /// itself, outside `Write`, resets it, or the next write may be skipped.
    std::optional<std::array<float, 8>> written;

    /// Write once the frame's springs have all stepped: a layer's five
    /// springs, and every layer, move the element in one write.
    void Touch() {
        if (queued) return;
        queued = true;
        Frames::AfterStep([weak = get_weak()] {
            if (auto self = weak.get()) {
                self->queued = false;
                self->Write();
            }
        });
    }

    std::shared_ptr<Layer> Get(std::string const& name) {
        for (auto const& [existing, layer] : layers)
            if (existing == name) return layer;
        auto layer = std::make_shared<Layer>();
        auto write = [weak = get_weak()] {
            if (auto self = weak.get()) self->Touch();
        };
        layer->x = Spring::Make(0, [write](double) { write(); });
        layer->y = Spring::Make(0, [write](double) { write(); });
        layer->sx = Spring::Make(1, [write](double) { write(); });
        layer->sy = Spring::Make(1, [write](double) { write(); });
        layer->opacity = Spring::Make(1, [write](double) { write(); });
        layers.emplace_back(name, layer);
        return layer;
    }

    void Write() {
        auto element = node.get();
        if (!element) return;
        double x = 0, y = 0, sx = 1, sy = 1, opacity = baseOpacity;
        // Scale about the middle of what the element shows: a text stretched
        // across a column shows only as wide as its lines.
        float3 center{0, 0, 0};
        float w = 0, h = 0;
        if (auto framework = element.try_as<xaml::FrameworkElement>()) {
            auto shown = Unwrapped(element).DesiredSize();
            w = static_cast<float>(framework.ActualWidth());
            h = static_cast<float>(framework.ActualHeight());
            if (shown.Width > 0) w = (std::min)(w, shown.Width);
            if (shown.Height > 0) h = (std::min)(h, shown.Height);
            center = float3{w / 2, h / 2, 0};
            if (alongPill)
                if (auto line = PillLine(element)) center.x = *line;
        }
        for (auto const& [name, layer] : layers) {
            x += layer->x->value;
            y += layer->y->value;
            sx *= layer->sx->value;
            sy *= layer->sy->value;
            opacity *= layer->opacity->value;
            // A pivoted scale is the scale about the center moved so the
            // pivot stays where it is.
            if (layer->pivot) {
                x += (layer->pivot->x * w - center.x) * (1 - layer->sx->value);
                y += (layer->pivot->y * h - center.y) * (1 - layer->sy->value);
            }
        }
        if (resting) x = y = 0, sx = sy = 1;
        float3 move{static_cast<float>(x), static_cast<float>(y), 0};
        float3 scale{static_cast<float>(sx), static_cast<float>(sy), 1};
        std::array<float, 8> now{move.x, move.y, scale.x, scale.y, center.x, center.y, static_cast<float>((std::clamp)(opacity, 0.0, 1.0)), handoff ? 1.0f : 0.0f};
        if (written == now) return;
        written = now;
        element.Opacity((std::clamp)(opacity, 0.0, 1.0));
        if (!handoff) {
            try {
                element.CenterPoint(center);
                element.Translation(move);
                element.Scale(scale);
                return;
            } catch (winrt::hresult_error const&) {
                handoff = true;
                xaml::Hosting::ElementCompositionPreview::SetIsTranslationEnabled(element, true);
            }
        }
        auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(element);
        visual.CenterPoint(center);
        visual.Scale(scale);
        visual.Properties().InsertVector3(L"Translation", move);
    }

    /// Show the element without its move and scale until `Rest(false)`,
    /// for a control that measures where its parts are, within one update
    /// so the screen never shows it.
    void Rest(bool rest) {
        if (resting == rest) return;
        resting = rest;
        Write();
    }

    /// How far in from the item's leading edge its selection pill runs.
    std::optional<float> PillLine(xaml::UIElement const& element) {
        auto track = pillTrack.get();
        if (!track) {
            // The pill's own visual is the view's to animate: its line is
            // that of the cell holding it.
            auto pill = TemplatePart(element, L"SelectionIndicator");
            if (!pill) return std::nullopt;
            track = media::VisualTreeHelper::GetParent(pill).try_as<xaml::UIElement>();
            if (!track) return std::nullopt;
            pillTrack = winrt::make_weak(track);
        }
        try {
            return track.TransformToVisual(element).TransformPoint({0, 0}).X;
        } catch (winrt::hresult_error const&) {
            pillTrack = nullptr;
            return std::nullopt;
        }
    }

    /// The move and scale all layers but `except` apply now.
    std::array<double, 4> Current(std::string const& except) const {
        double x = 0, y = 0, sx = 1, sy = 1;
        for (auto const& [name, layer] : layers) {
            if (name == except) continue;
            x += layer->x->value;
            y += layer->y->value;
            sx *= layer->sx->value;
            sy *= layer->sy->value;
        }
        return {x, y, sx, sy};
    }
};

/// The layers of `element`, if anything has moved it.
inline winrt::com_ptr<LayerStack> ExistingLayers(xaml::UIElement const& element) {
    auto framework = element.try_as<xaml::FrameworkElement>();
    if (!framework) return nullptr;
    auto key = winrt::box_value(L"bern.layers");
    auto resources = framework.Resources();
    if (!resources.HasKey(key)) return nullptr;
    return winrt::get_self<LayerStack>(resources.Lookup(key))->get_strong();
}

/// The layers of `element`, made on first use and kept with it.
inline winrt::com_ptr<LayerStack> LayersOf(xaml::UIElement const& element) {
    if (auto existing = ExistingLayers(element)) return existing;
    auto framework = element.as<xaml::FrameworkElement>();
    auto made = winrt::make_self<LayerStack>();
    made->node = winrt::make_weak(element);
    made->baseOpacity = element.Opacity();
    // Navigation view items move their own visual, which then cannot take
    // Translation and Scale: move that visual instead, from the start.
    if (element.try_as<controls::NavigationViewItem>()) {
        made->handoff = true;
        made->alongPill = true;
        xaml::Hosting::ElementCompositionPreview::SetIsTranslationEnabled(element, true);
    }
    framework.Resources().Insert(winrt::box_value(L"bern.layers"), made.as<IInspectable>());
    // The center moves with the size.
    framework.SizeChanged([weak = made->get_weak()](auto&&, auto&&) {
        if (auto self = weak.get()) self->Write();
    });
    return made;
}

/// Move `layer` of `element` to a move, scale and opacity with `motion`.
inline void LayerTo(xaml::UIElement const& element, std::string const& name, double x, double y, double sx, double sy, double opacity,
    Motion const* motion, std::function<void()> done = nullptr, double delay = 0) {
    auto layer = LayersOf(element)->Get(name);
    auto left = std::make_shared<int>(5);
    auto finish = [left, done] {
        if (--*left == 0 && done) done();
    };
    auto to = [&](std::shared_ptr<Spring> const& spring, double value) {
        if (spring->target == value && !spring->done && delay <= 0) {
            finish();
            return;
        }
        spring->To(value, motion, finish, delay);
    };
    to(layer->x, x);
    to(layer->y, y);
    to(layer->sx, sx);
    to(layer->sy, sy);
    to(layer->opacity, opacity);
}

inline void LayerSet(xaml::UIElement const& element, std::string const& name, double x, double y, double sx, double sy, double opacity) {
    auto layer = LayersOf(element)->Get(name);
    layer->x->Set(x);
    layer->y->Set(y);
    layer->sx->Set(sx);
    layer->sy->Set(sy);
    layer->opacity->Set(opacity);
}

/// The spring with which a short fade plays when motion is reduced.
inline Motion const kQuickFade{0.15, 1};
inline Motion const kNudgeUp{0.12, 1};

// ---------------------------------------------------------------------------
// Structural motion: which mounting elements play `appear`

/// True while `When` or `Each` change after their first display, for the
/// elements they mount themselves; `updating` stays true at any depth, for
/// containers that stagger their children.
namespace Structure {
inline bool entering = false;
inline bool updating = false;

struct Scope {
    bool savedEntering, savedUpdating;
    explicit Scope(bool animated) : savedEntering(entering), savedUpdating(updating) {
        entering = animated;
        updating = updating || animated;
    }
    ~Scope() {
        entering = savedEntering;
        updating = savedUpdating;
    }
};
}

/// The stagger of a container: its direct children that appear take turns,
/// `step` seconds apart; the count starts over with each update.
struct StaggerClock {
    double step = 0;
    int next = 0;
    bool play = false;
    bool resetting = false;
};

/// The context for a container's children with `stagger`.
inline Ctx Staggered(Ctx const& ctx, double step) {
    auto child = ctx;
    child.clock = std::make_shared<StaggerClock>();
    child.clock->step = step;
    child.clock->play = Structure::updating;
    return child;
}

/// The delay of the next child to appear on `clock`.
inline double Turn(std::shared_ptr<StaggerClock> const& clock) {
    double delay = clock->next * clock->step;
    clock->next += 1;
    if (!clock->resetting) {
        clock->resetting = true;
        Host::Post([clock] {
            clock->next = 0;
            clock->resetting = false;
        });
    }
    return delay;
}

/// Where a transition starts (or ends): a move, a scale and no opacity; a
/// growing one has no size along its axis and stays opaque.
inline std::array<double, 5> TransitionFrom(Transition const& transition) {
    std::array<double, 5> from{0, 0, 1, 1, 0};
    if (transition.kind == Transition::Grow) {
        std::string_view edge = transition.edge ? transition.edge : "leading";
        if (edge == "leading" || edge == "trailing") from[2] = 0;
        else from[3] = 0;
        from[4] = 1;
        return from;
    }
    if (transition.kind == Transition::Pop) {
        from[2] = from[3] = transition.from;
    } else if (transition.kind == Transition::Slide) {
        std::string_view edge = transition.edge ? transition.edge : "bottom";
        double distance = transition.distance;
        if (edge == "top") from[1] = -distance;
        else if (edge == "bottom") from[1] = distance;
        else if (edge == "leading") from[0] = -distance;
        else if (edge == "trailing") from[0] = distance;
    }
    return from;
}

/// Play `transition` on `element`: in from its start, or out to it.
inline void PlayTransition(xaml::UIElement const& element, Transition const& transition, bool entering, std::function<void()> done = nullptr,
    double delay = 0) {
    bool reduced = Reduced();
    auto from = reduced ? std::array<double, 5>{0, 0, 1, 1, 0} : TransitionFrom(transition);
    std::array<double, 5> rest{0, 0, 1, 1, 1};
    auto start = entering ? from : rest;
    auto end = entering ? rest : from;
    std::string name = entering ? "enter" : "exit";
    // Growing holds its edge still.
    auto layer = LayersOf(element)->Get(name);
    if (transition.kind == Transition::Grow && !reduced) {
        std::string_view edge = transition.edge ? transition.edge : "leading";
        layer->pivot = edge == "leading" ? float2{0, 0.5f} : edge == "trailing" ? float2{1, 0.5f} : edge == "top" ? float2{0.5f, 0} : float2{0.5f, 1};
    } else {
        layer->pivot.reset();
    }
    LayerSet(element, name, start[0], start[1], start[2], start[3], start[4]);
    Motion const* motion = reduced ? &kQuickFade : transition.motion;
    // A staggered element waits at its start, hidden, for its turn, counted
    // in rendered frames.
    LayerTo(element, name, end[0], end[1], end[2], end[3], end[4], motion, done, reduced ? 0 : delay);
}

/// A short nudge: to `scale` of its size, then back with `motion`.
inline void NudgeOnce(xaml::UIElement const& element, Motion const* motion, double scale = 1.06) {
    if (Reduced()) return;
    LayerTo(element, "nudge", 0, 0, scale, scale, 1, &kNudgeUp, [element, motion] { LayerTo(element, "nudge", 0, 0, 1, 1, 1, motion); });
}

inline void Reflow(xaml::UIElement const& child, float dx, float dy, Motion const* motion) {
    if (Reduced()) return;
    auto layer = LayersOf(child)->Get("reflow");
    layer->x->Set(layer->x->value + dx);
    layer->y->Set(layer->y->value + dy);
    layer->x->To(0, motion);
    layer->y->To(0, motion);
}

// ---------------------------------------------------------------------------
// Bound lengths

/// A number that follows `len`, springing to each new value with `motion`
/// (at once without one), written by `write`.
inline void BindLen(Ctx const& ctx, Len const& len, Motion const* motion, std::function<void(double)> write) {
    if (!len.Given()) return;
    if (len.fixed) {
        write(*len.fixed);
        return;
    }
    auto bound = len.bound;
    double initial = Reactive::Untrack([&] { return R::AsDouble(bound(ctx)); });
    auto spring = Spring::Make(initial, write);
    write(initial);
    auto first = std::make_shared<bool>(true);
    Reactive::Run([ctx, bound, spring, motion, first] {
        double next = R::AsDouble(bound(ctx));
        if (*first) {
            *first = false;
            return;
        }
        Reactive::Untrack([&] {
            spring->To(next, motion);
            return 0;
        });
    });
    Reactive::OnCleanup([spring] { Frames::Stop(spring.get()); });
}

// ---------------------------------------------------------------------------
// Theme colors. Roles map to each front end's theme resources; drawings
// read concrete colors from probes that follow the theme of the part they
// are in and the accent chosen at run time.

/// Bumped when the colors behind roles change: an accent chosen at run
/// time. Theme changes reach drawings through the element's theme.
inline Cell<int64_t> themeTick{0};

/// The theme resource of a role in this front end, or nothing.
inline wchar_t const* RoleKey(std::string_view role) {
    struct Entry {
        std::string_view role;
        wchar_t const* winui;
        wchar_t const* system;
    };
    static Entry const table[] = {
        {"surface.canvas", L"SolidBackgroundFillColorBaseBrush", L"ApplicationPageBackgroundThemeBrush"},
        {"surface.panel", L"CardBackgroundFillColorDefaultBrush", L"SystemControlBackgroundListLowBrush"},
        {"surface.raised", L"SolidBackgroundFillColorTertiaryBrush", L"SystemControlBackgroundChromeMediumLowBrush"},
        {"text.primary", L"TextFillColorPrimaryBrush", L"SystemControlForegroundBaseHighBrush"},
        {"text.secondary", L"TextFillColorSecondaryBrush", L"SystemControlForegroundBaseMediumBrush"},
        {"text.placeholder", L"TextFillColorTertiaryBrush", L"SystemControlPageTextBaseMediumBrush"},
        {"icon.primary", L"TextFillColorPrimaryBrush", L"SystemControlForegroundBaseHighBrush"},
        {"icon.secondary", L"TextFillColorSecondaryBrush", L"SystemControlForegroundBaseMediumBrush"},
        {"accent", L"AccentFillColorDefaultBrush", L"SystemControlBackgroundAccentBrush"},
        {"on_accent", L"TextOnAccentFillColorPrimaryBrush", L"SystemControlForegroundChromeWhiteBrush"},
        {"on_error", L"TextOnAccentFillColorPrimaryBrush", L"SystemControlForegroundChromeWhiteBrush"},
        {"status.success", L"SystemFillColorSuccessBrush", nullptr},
        {"status.warning", L"SystemFillColorCautionBrush", nullptr},
        {"status.error", L"SystemFillColorCriticalBrush", L"SystemControlErrorTextForegroundBrush"},
        // The hairline of controls and cards (the design's Fluent stroke),
        // not the strong outline of an unchecked box.
        {"control.border", L"ControlStrokeColorDefaultBrush", L"SystemControlForegroundBaseLowBrush"},
        {"control.track", L"ControlStrongFillColorDefaultBrush", L"SystemControlForegroundBaseMediumLowBrush"},
        {"control.thumb", L"ControlSolidFillColorDefaultBrush", L"SystemControlBackgroundChromeWhiteBrush"},
        {"focus.indicator", L"FocusStrokeColorOuterBrush", L"SystemControlFocusVisualPrimaryBrush"},
        {"selection.background", L"SubtleFillColorTertiaryBrush", L"SystemControlHighlightListLowBrush"},
        {"selection.foreground", L"TextFillColorPrimaryBrush", L"SystemControlForegroundBaseHighBrush"},
        {"disabled.foreground", L"TextFillColorDisabledBrush", L"SystemControlDisabledBaseMediumLowBrush"},
        {"disabled.surface", L"ControlFillColorDisabledBrush", L"SystemControlDisabledBaseLowBrush"},
        {"fill.subtle", L"SubtleFillColorSecondaryBrush", L"SystemControlBackgroundListLowBrush"},
        // A step above `fill.subtle` in both appearances, as light as the
        // other targets' (about 10 %); the control's strong fill is the
        // dark one of slider tracks.
        {"fill.strong", L"ControlAltFillColorQuarternaryBrush", L"SystemControlBackgroundListMediumBrush"},
    };
    for (auto const& entry : table)
        if (entry.role == role) return kWinUI ? entry.winui : entry.system;
    return nullptr;
}

/// Colors of roles the system XAML has no resource for: light and dark.
inline std::optional<std::pair<uint32_t, uint32_t>> RoleFallback(std::string_view role) {
    if (role == "status.success") return std::pair<uint32_t, uint32_t>{0x0F7B0F, 0x6CCB5F};
    if (role == "status.warning") return std::pair<uint32_t, uint32_t>{0x9D5D00, 0xFCE100};
    return std::nullopt;
}

inline winrt::Windows::UI::Color Rgb(uint32_t rgb, uint8_t alpha = 255) {
    return winrt::Windows::UI::Color{alpha, static_cast<uint8_t>(rgb >> 16), static_cast<uint8_t>(rgb >> 8), static_cast<uint8_t>(rgb)};
}

/// `#AARRGGBB`, for XAML markup.
inline std::wstring Hex(winrt::Windows::UI::Color color) {
    wchar_t text[10];
    swprintf(text, 10, L"#%02X%02X%02X%02X", color.A, color.R, color.G, color.B);
    return text;
}

/// Hidden elements in a window whose theme brushes say what color a role
/// has in the light and the dark appearance.
struct Probes {
    controls::Grid holder{nullptr};
    controls::Grid themes[2]{nullptr, nullptr};
    std::map<std::pair<int, std::string>, controls::Border> borders;
};

inline std::map<Win*, std::shared_ptr<Probes>>& AllProbes() {
    static std::map<Win*, std::shared_ptr<Probes>> probes;
    return probes;
}

/// The probes of a window, made in its root on first use.
inline std::shared_ptr<Probes> ProbesOf(Win* win) {
    auto& all = AllProbes();
    if (auto found = all.find(win); found != all.end()) return found->second;
    auto made = std::make_shared<Probes>();
    if (win && win->root) {
        made->holder = controls::Grid();
        made->holder.Width(0);
        made->holder.Height(0);
        made->holder.Opacity(0);
        made->holder.IsHitTestVisible(false);
        xaml::Automation::AutomationProperties::SetAccessibilityView(made->holder, xaml::Automation::Peers::AccessibilityView::Raw);
        for (int dark = 0; dark < 2; ++dark) {
            controls::Grid theme;
            theme.RequestedTheme(dark ? xaml::ElementTheme::Dark : xaml::ElementTheme::Light);
            made->themes[dark] = theme;
            made->holder.Children().Append(theme);
        }
        win->root.Children().Append(made->holder);
    }
    all[win] = made;
    return made;
}

/// The color of `paint` in the light or dark appearance, as `win` shows it.
inline winrt::Windows::UI::Color ColorOf(Win* win, bool dark, Paint const& paint, double alpha = 1) {
    auto scaled = [alpha](winrt::Windows::UI::Color color) {
        color.A = static_cast<uint8_t>(std::lround(color.A * (std::clamp)(alpha, 0.0, 1.0)));
        return color;
    };
    if (paint.fixed) return scaled(Rgb(dark ? paint.dark : paint.light));
    if (!paint.role) return winrt::Windows::UI::Color{0, 0, 0, 0};
    std::string role = paint.role;
    if (auto fallback = RoleFallback(role); fallback && !RoleKey(role)) return scaled(Rgb(dark ? fallback->second : fallback->first));
    auto key = RoleKey(role);
    if (!key) return scaled(Rgb(dark ? 0xFFFFFF : 0x000000));
    auto probes = ProbesOf(win);
    if (!probes->holder) return scaled(Rgb(dark ? 0xFFFFFF : 0x000000));
    auto id = std::pair<int, std::string>{dark ? 1 : 0, role};
    auto found = probes->borders.find(id);
    if (found == probes->borders.end()) {
        controls::Border border;
        if (auto style = Styles::Themed(L"Border", {{L"Background", key}})) border.Style(style);
        probes->themes[dark ? 1 : 0].Children().Append(border);
        found = probes->borders.emplace(id, border).first;
    }
    if (auto solid = found->second.Background().try_as<media::SolidColorBrush>()) {
        auto color = solid.Color();
        // A brush's own opacity is part of its color.
        color.A = static_cast<uint8_t>(std::lround(color.A * solid.Opacity()));
        return scaled(color);
    }
    return scaled(Rgb(dark ? 0xFFFFFF : 0x000000));
}

/// Whether `element` shows the dark appearance.
inline bool IsDark(xaml::FrameworkElement const& element) { return element.ActualTheme() == xaml::ElementTheme::Dark; }

/// Set `property` of `element` (a `target` such as `Border`) to `paint`,
/// following the theme where it is used: theme resources through a style,
/// a fixed pair by the element's theme.
template <class T> void PaintWith(T const& element, wchar_t const* target, wchar_t const* property, B::Paint const& paint, void (*set)(T const&, media::Brush const&)) {
    if (paint.role)
        if (auto key = RoleKey(paint.role)) {
            if (auto style = Styles::Themed(target, {{property, key}})) element.Style(style);
            return;
        }
    auto apply = [element, paint, set] {
        bool dark = IsDark(element);
        uint32_t rgb = paint.fixed ? (dark ? paint.dark : paint.light) : 0;
        if (!paint.fixed && paint.role)
            if (auto fallback = RoleFallback(paint.role)) rgb = dark ? fallback->second : fallback->first;
        set(element, media::SolidColorBrush(Rgb(rgb)));
    };
    apply();
    element.ActualThemeChanged([apply](auto&&, auto&&) { apply(); });
}

inline void Background(controls::Border const& border, B::Paint const& paint) {
    PaintWith<controls::Border>(border, L"Border", L"Background", paint, [](controls::Border const& e, media::Brush const& b) { e.Background(b); });
}

inline void FillShape(xaml::Shapes::Rectangle const& shape, B::Paint const& paint) {
    PaintWith<xaml::Shapes::Rectangle>(shape, L"Rectangle", L"Fill", paint, [](xaml::Shapes::Rectangle const& e, media::Brush const& b) { e.Fill(b); });
}

// ---------------------------------------------------------------------------
// The accent chosen at run time. Control templates use the accent brushes
// of the theme dictionaries, through aliases as well; the runtime finds
// those brushes and changes their colors, so every control shows the new
// accent at once.

/// The accent brushes and which shade of the accent each shows, in the
/// light (0) and dark (1) appearance: 0 the accent, -1…-3 darker, 1…3
/// lighter, with an opacity. `on` brushes are the text on an accent fill:
/// white or black, whichever reads on the accent.
struct AccentBrush {
    wchar_t const* key;
    int light;
    int dark;
    double opacity = 1;
    bool on = false;
};

inline std::vector<AccentBrush> const& AccentBrushes() {
    static std::vector<AccentBrush> const winui{
        // Fills show the accent itself in both appearances, as the design
        // does, with the text that reads on it.
        {L"AccentFillColorDefaultBrush", 0, 0},
        {L"AccentFillColorSecondaryBrush", 0, 0, 0.9},
        {L"AccentFillColorTertiaryBrush", 0, 0, 0.8},
        {L"TextOnAccentFillColorPrimaryBrush", 0, 0, 1, true},
        {L"TextOnAccentFillColorSecondaryBrush", 0, 0, 0.7, true},
        {L"AccentFillColorSelectedTextBackgroundBrush", 0, 0},
        {L"AccentTextFillColorPrimaryBrush", -2, 3},
        {L"AccentTextFillColorSecondaryBrush", -3, 3},
        {L"AccentTextFillColorTertiaryBrush", -1, 2},
        {L"SystemControlHighlightAccentBrush", 0, 0},
        {L"SystemControlBackgroundAccentBrush", 0, 0},
        {L"SystemControlForegroundAccentBrush", 0, 0},
        {L"SystemControlHighlightAltAccentBrush", 0, 0},
        {L"SystemControlHighlightListAccentLowBrush", 0, 0, 0.6},
        {L"SystemControlHighlightListAccentMediumBrush", 0, 0, 0.8},
        {L"SystemControlHighlightListAccentHighBrush", 0, 0, 0.9},
    };
    static std::vector<AccentBrush> const system{
        {L"SystemControlHighlightAccentBrush", 0, 0},
        {L"SystemControlBackgroundAccentBrush", 0, 0},
        {L"SystemControlForegroundAccentBrush", 0, 0},
        {L"SystemControlHighlightAltAccentBrush", 0, 0},
        {L"SystemControlHyperlinkTextBrush", 0, 0},
        {L"SystemControlHighlightListAccentLowBrush", 0, 0, 0.6},
        {L"SystemControlHighlightListAccentMediumBrush", 0, 0, 0.8},
        {L"SystemControlHighlightListAccentHighBrush", 0, 0, 0.9},
        {L"SystemControlBackgroundAccentRevealBorderBrush", 0, 0},
    };
    return kWinUI ? winui : system;
}

/// A shade of `rgb`: toward white for steps above 0, toward black below.
inline winrt::Windows::UI::Color Shade(uint32_t rgb, int step) {
    auto mix = [](uint8_t from, uint8_t to, double amount) { return static_cast<uint8_t>(std::lround(from + (to - from) * amount)); };
    uint8_t r = static_cast<uint8_t>(rgb >> 16), g = static_cast<uint8_t>(rgb >> 8), b = static_cast<uint8_t>(rgb);
    if (step == 0) return winrt::Windows::UI::Color{255, r, g, b};
    uint8_t to = step > 0 ? 255 : 0;
    double amount = 0.2 * std::abs(step);
    return winrt::Windows::UI::Color{255, mix(r, to, amount), mix(g, to, amount), mix(b, to, amount)};
}

/// The text on an accent fill: white, or near black on a light accent
/// (relative luminance above one half), as the design pairs them.
inline winrt::Windows::UI::Color OnAccent(uint32_t rgb) {
    auto linear = [](uint8_t c) {
        double v = c / 255.0;
        return v <= 0.04045 ? v / 12.92 : std::pow((v + 0.055) / 1.055, 2.4);
    };
    double l = 0.2126 * linear(uint8_t(rgb >> 16)) + 0.7152 * linear(uint8_t(rgb >> 8)) + 0.0722 * linear(uint8_t(rgb));
    return l > 0.5 ? winrt::Windows::UI::Color{255, 11, 11, 13} : winrt::Windows::UI::Color{255, 255, 255, 255};
}

/// Every solid brush stored under `key` for `theme` in `dictionary`, its
/// theme dictionaries and merged dictionaries.
inline void FindThemeBrushes(xaml::ResourceDictionary const& dictionary, std::wstring const& theme, wchar_t const* key,
    std::vector<media::SolidColorBrush>& found, int depth = 0) {
    if (!dictionary || depth > 8) return;
    auto boxedKey = winrt::box_value(key);
    try {
        auto themes = dictionary.ThemeDictionaries();
        auto boxedTheme = winrt::box_value(theme);
        if (themes.HasKey(boxedTheme)) {
            if (auto inner = themes.Lookup(boxedTheme).try_as<xaml::ResourceDictionary>()) {
                if (inner.HasKey(boxedKey))
                    if (auto brush = inner.Lookup(boxedKey).try_as<media::SolidColorBrush>()) found.push_back(brush);
                FindThemeBrushes(inner, theme, key, found, depth + 1);
            }
        }
        for (auto merged : dictionary.MergedDictionaries()) FindThemeBrushes(merged, theme, key, found, depth + 1);
    } catch (winrt::hresult_error const&) {
    }
}

/// Show the accent `light` / `dark` (sRGB) in every control.
inline void ApplyAccent(uint32_t light, uint32_t dark) {
    auto resources = xaml::Application::Current().Resources();
    for (auto const& entry : AccentBrushes()) {
        for (int theme = 0; theme < 2; ++theme) {
            std::vector<media::SolidColorBrush> brushes;
            std::vector<std::wstring> names = theme ? std::vector<std::wstring>{L"Dark", L"Default"} : std::vector<std::wstring>{L"Light"};
            for (auto const& name : names) FindThemeBrushes(resources, name, entry.key, brushes);
            auto color = entry.on ? OnAccent(theme ? dark : light) : Shade(theme ? dark : light, theme ? entry.dark : entry.light);
            for (auto& brush : brushes) {
                try {
                    brush.Color(color);
                    brush.Opacity(entry.opacity);
                } catch (winrt::hresult_error const&) {
                }
            }
        }
        // Resources the system provides for the current theme only.
        try {
            auto boxed = winrt::box_value(entry.key);
            if (resources.HasKey(boxed) || !kWinUI)
                if (auto brush = resources.Lookup(boxed).try_as<media::SolidColorBrush>()) {
                    bool night = xaml::Application::Current().RequestedTheme() == xaml::ApplicationTheme::Dark;
                    brush.Color(entry.on ? OnAccent(night ? dark : light) : Shade(night ? dark : light, 0));
                }
        } catch (winrt::hresult_error const&) {
        }
    }
    // The accent colors themselves, for what resolves them later.
    auto rgb = [](uint32_t value) { return winrt::box_value(Rgb(value)); };
    try {
        resources.Insert(winrt::box_value(L"SystemAccentColor"), rgb(light));
        for (int step = 1; step <= 3; ++step) {
            resources.Insert(winrt::box_value(L"SystemAccentColorLight" + std::to_wstring(step)), winrt::box_value(Shade(dark, step)));
            resources.Insert(winrt::box_value(L"SystemAccentColorDark" + std::to_wstring(step)), winrt::box_value(Shade(light, -step)));
        }
    } catch (winrt::hresult_error const&) {
    }
    themeTick.Set(themeTick.Peek() + 1);
}

// ---------------------------------------------------------------------------
// Style overrides of `style!`

/// The foreground that pairs with a fill.
inline std::optional<Paint> PairOf(std::optional<Paint> const& fill) {
    if (!fill || !fill->role) return std::nullopt;
    std::string_view role = fill->role;
    if (role == "accent") return Paint{"on_accent"};
    if (role == "status.error") return Paint{"on_error"};
    return std::nullopt;
}

/// The looks of a style merged for a set of states, later ones winning:
/// selected, hover, pressed, focus, disabled.
inline Look Merged(StyleDef const& style, bool selected, bool hover, bool pressed, bool focus, bool disabled, bool& quiet, bool& system) {
    Look look = style.base;
    quiet = style.base.quiet;
    system = style.base.system;
    auto merge = [&](std::optional<Look> const& state, bool on) {
        if (!state || !on) return;
        if (state->background) look.background = state->background;
        if (state->foreground) look.foreground = state->foreground;
        if (state->border) look.border = state->border;
        if (state->borderWidth) look.borderWidth = state->borderWidth;
        if (state->fill) look.fill = state->fill;
        if (state->shadow) look.shadow = state->shadow;
        if (state->scaleX) look.scaleX = state->scaleX;
        if (state->scaleY) look.scaleY = state->scaleY;
        if (state->offsetX) look.offsetX = state->offsetX;
        if (state->offsetY) look.offsetY = state->offsetY;
        if (state->opacity) look.opacity = state->opacity;
        if (state->motion) look.motion = state->motion;
        quiet = quiet || state->quiet;
        system = system || state->system;
    };
    merge(style.selected, selected);
    merge(style.hover, hover);
    merge(style.pressed, pressed);
    merge(style.focus, focus);
    merge(style.disabled, disabled);
    return look;
}

/// A theme dictionary entry for a lightweight styling resource.
inline std::wstring BrushEntry(wchar_t const* key, Paint const& paint, bool dark) {
    if (paint.role && std::string_view(paint.role) == "transparent")
        return std::wstring(L"<SolidColorBrush x:Key=\"") + key + L"\" Color=\"#00000000\" />";
    if (paint.role)
        if (auto resource = RoleKey(paint.role))
            return std::wstring(L"<StaticResource x:Key=\"") + key + L"\" ResourceKey=\"" + resource + L"\" />";
    uint32_t rgb = paint.fixed ? (dark ? paint.dark : paint.light) : 0;
    if (!paint.fixed)
        if (auto fallback = paint.role ? RoleFallback(paint.role) : std::nullopt) rgb = dark ? fallback->second : fallback->first;
    return std::wstring(L"<SolidColorBrush x:Key=\"") + key + L"\" Color=\"" + Hex(Rgb(rgb)) + L"\" />";
}

/// Lightweight styling resources, `key → paint` in each theme.
inline std::wstring ThemedXaml(std::vector<std::pair<std::wstring, Paint>> const& entries) {
    std::wstring themes;
    for (auto [theme, dark] : {std::pair{L"Default", true}, std::pair{L"Light", false}}) {
        themes += std::wstring(L"<ResourceDictionary x:Key=\"") + theme + L"\">";
        for (auto const& [key, paint] : entries) themes += BrushEntry(key.c_str(), paint, dark);
        themes += L"</ResourceDictionary>";
    }
    return LR"(<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"><ResourceDictionary.ThemeDictionaries>)" +
        themes + L"</ResourceDictionary.ThemeDictionaries></ResourceDictionary>";
}

/// The pointer and focus state a styled control shows, and what it does
/// when that changes.
struct StyleState {
    bool hover = false, pressed = false, focus = false, disabled = false, selected = false;
    Motion const* lastMotion = nullptr;
};

/// Scale, offset and opacity of the merged look, on the `state` layer of
/// `moving`, with the motion of the topmost state that moves, or `settle`.
inline void MoveTo(xaml::UIElement const& moving, StyleDef const& style, StyleState& state) {
    bool quiet = false, system = false;
    auto look = Merged(style, state.selected, state.hover, state.pressed, state.focus, state.disabled, quiet, system);
    Motion const* motion = nullptr;
    std::vector<std::optional<Look> const*> order{&style.disabled, &style.focus, &style.pressed, &style.hover, &style.selected};
    std::vector<bool> active{state.disabled, state.focus, state.pressed, state.hover, state.selected};
    for (size_t i = 0; i < order.size(); ++i)
        if (active[i] && *order[i] && (*order[i])->motion) {
            motion = (*order[i])->motion;
            break;
        }
    if (motion) state.lastMotion = motion;
    else motion = style.settle ? style.settle : state.lastMotion;
    LayerTo(moving, "state", look.offsetX.value_or(0), look.offsetY.value_or(0), look.scaleX.value_or(1), look.scaleY.value_or(1),
        look.opacity.value_or(1), motion);
}

/// Follow the pointer, keys and focus of `control`, calling `changed`.
inline void TrackState(xaml::UIElement const& control, std::shared_ptr<StyleState> state, std::function<void()> changed) {
    auto set = [state, changed](bool StyleState::* member, bool value) {
        if (state.get()->*member == value) return;
        state.get()->*member = value;
        changed();
    };
    using PointerHandler = xaml::Input::PointerEventHandler;
    control.AddHandler(xaml::UIElement::PointerEnteredEvent(), winrt::box_value(PointerHandler([set](auto&&, xaml::Input::PointerRoutedEventArgs const& args) {
        if (args.Pointer().PointerDeviceType() == PointerKind::Mouse) set(&StyleState::hover, true);
    })), true);
    control.AddHandler(xaml::UIElement::PointerExitedEvent(), winrt::box_value(PointerHandler([set](auto&&, auto&&) {
        set(&StyleState::hover, false);
        set(&StyleState::pressed, false);
    })), true);
    control.AddHandler(xaml::UIElement::PointerPressedEvent(), winrt::box_value(PointerHandler([set](auto&&, auto&&) { set(&StyleState::pressed, true); })), true);
    control.AddHandler(xaml::UIElement::PointerReleasedEvent(), winrt::box_value(PointerHandler([set](auto&&, auto&&) { set(&StyleState::pressed, false); })), true);
    control.AddHandler(xaml::UIElement::PointerCaptureLostEvent(), winrt::box_value(PointerHandler([set](auto&&, auto&&) { set(&StyleState::pressed, false); })), true);
    control.AddHandler(xaml::UIElement::PointerCanceledEvent(), winrt::box_value(PointerHandler([set](auto&&, auto&&) { set(&StyleState::pressed, false); })), true);
    using KeyHandler = xaml::Input::KeyEventHandler;
    control.AddHandler(xaml::UIElement::KeyDownEvent(), winrt::box_value(KeyHandler([set](auto&&, xaml::Input::KeyRoutedEventArgs const& args) {
        auto key = args.Key();
        if (key == winrt::Windows::System::VirtualKey::Space || key == winrt::Windows::System::VirtualKey::Enter) set(&StyleState::pressed, true);
    })), true);
    control.AddHandler(xaml::UIElement::KeyUpEvent(), winrt::box_value(KeyHandler([set](auto&&, auto&&) { set(&StyleState::pressed, false); })), true);
    control.GotFocus([set, weak = winrt::make_weak(control)](auto&&, auto&&) {
        auto element = weak.get();
        auto focusable = element ? element.try_as<controls::Control>() : nullptr;
        set(&StyleState::focus, focusable && focusable.FocusState() == xaml::FocusState::Keyboard);
    });
    control.LostFocus([set](auto&&, auto&&) { set(&StyleState::focus, false); });
    if (auto control2 = control.try_as<controls::Control>())
        control2.IsEnabledChanged([set, weak = winrt::make_weak(control2)](auto&&, auto&&) {
            if (auto element = weak.get()) set(&StyleState::disabled, !element.IsEnabled());
        });
}

/// The lightweight resources of a styled button for `prefix` (`Button`):
/// background, foreground and border in each state that differs from the
/// platform's.
inline std::vector<std::pair<std::wstring, Paint>> ButtonResources(StyleDef const& style, std::wstring const& prefix) {
    std::vector<std::pair<std::wstring, Paint>> entries;
    struct State {
        wchar_t const* suffix;
        bool hover, pressed, disabled;
    };
    for (auto const& state : {State{L"", false, false, false}, State{L"PointerOver", true, false, false}, State{L"Pressed", true, true, false},
             State{L"Disabled", false, false, true}}) {
        bool quiet = false, system = false;
        auto look = Merged(style, false, state.hover, state.pressed, false, state.disabled, quiet, system);
        // `system` keeps the platform look of this state; `none` shows the
        // resting look.
        if (system && state.suffix[0] != 0) continue;
        if (quiet) look = Merged(style, false, false, false, false, false, quiet, system);
        auto background = look.fill ? look.fill : look.background;
        auto foreground = look.foreground ? look.foreground : PairOf(background);
        if (background) entries.emplace_back(prefix + L"Background" + state.suffix, *background);
        if (foreground) entries.emplace_back(prefix + L"Foreground" + state.suffix, *foreground);
        if (look.border) entries.emplace_back(prefix + L"BorderBrush" + state.suffix, *look.border);
        else if (background) entries.emplace_back(prefix + L"BorderBrush" + state.suffix, Paint{"transparent"});
    }
    return entries;
}

}

// ---------------------------------------------------------------------------
// Shared elements, scroll effects and element decoration

namespace B {

/// Elements with a `hero` key: the last place of those that just left,
/// for the next one with the same key to grow from.
namespace Hero {
struct Departure {
    Win* win = nullptr;
    Rect rect{};
    xaml::UIElement element{nullptr};
    /// The leaving element, moved above the content while it fades.
    xaml::UIElement ghost{nullptr};
};
inline std::map<std::string, Departure> departed;

/// Where `element` lies in its window's content, without Bern's moves.
inline std::optional<Rect> PlaceIn(Win* win, xaml::UIElement const& element) {
    auto framework = element.try_as<xaml::FrameworkElement>();
    if (!win || !win->root || !framework || framework.ActualWidth() <= 0 || framework.ActualHeight() <= 0) return std::nullopt;
    try {
        auto origin = element.TransformToVisual(win->root).TransformPoint({0, 0});
        float3 translation{0, 0, 0};
        try {
            translation = element.Translation();
        } catch (winrt::hresult_error const&) {
        }
        // What it shows: a text stretched across a column is as wide as its
        // lines.
        auto shown = Unwrapped(element).DesiredSize();
        float w = static_cast<float>(framework.ActualWidth()), h = static_cast<float>(framework.ActualHeight());
        if (shown.Width > 0) w = (std::min)(w, shown.Width);
        if (shown.Height > 0) h = (std::min)(h, shown.Height);
        return Rect{origin.X - translation.x, origin.Y - translation.y, w, h};
    } catch (winrt::hresult_error const&) {
        return std::nullopt;
    }
}

inline void DropGhost(Departure& departure) {
    if (!departure.ghost || !departure.win || !departure.win->overlay) return;
    uint32_t index;
    if (departure.win->overlay.Children().IndexOf(departure.ghost, index)) departure.win->overlay.Children().RemoveAt(index);
    departure.ghost = nullptr;
}

/// `element` leaves: remember where it was. When it goes off its list, it
/// moves above the content at that place, to fade as the next one grows.
inline void Leave(std::string const& key, xaml::UIElement const& element, Win* win) {
    auto rect = PlaceIn(win, element);
    if (!rect) return;
    departed[key] = Departure{win, *rect, element, nullptr};
    if (!LeaveHookOf(element)) {
        auto hook = winrt::make_self<LeaveHook>();
        hook->leave = [key, element, win](std::function<void()> done) {
            done();
            auto found = departed.find(key);
            if (found == departed.end() || found->second.element != element || !win->overlay) return;
            auto framework = element.as<xaml::FrameworkElement>();
            framework.Width(found->second.rect.Width);
            framework.Height(found->second.rect.Height);
            controls::Canvas::SetLeft(element, found->second.rect.X);
            controls::Canvas::SetTop(element, found->second.rect.Y);
            element.IsHitTestVisible(false);
            win->overlay.Children().Append(element);
            found->second.ghost = element;
        };
        element.as<xaml::FrameworkElement>().Resources().Insert(winrt::box_value(L"bern.leave"), hook.as<IInspectable>());
    }
    // Nothing took its place: the copy goes.
    Host::After(std::chrono::milliseconds(400), [key, element] {
        auto found = departed.find(key);
        if (found == departed.end() || found->second.element != element) return;
        DropGhost(found->second);
        departed.erase(found);
    });
}

/// The new element grows from the old one's place and size while the old
/// one's copy moves along and fades out: one spring drives both.
inline void Grow(xaml::UIElement const& element, Departure from, Motion const* motion) {
    auto to = PlaceIn(from.win, element);
    if (!to || Reduced()) {
        DropGhost(from);
        return;
    }
    auto a = from.rect;
    double dx = a.X + a.Width / 2 - (to->X + to->Width / 2);
    double dy = a.Y + a.Height / 2 - (to->Y + to->Height / 2);
    double sx = a.Width / to->Width, sy = a.Height / to->Height;
    auto layer = LayersOf(element)->Get("hero");
    auto ghost = from.ghost;
    auto progress = Spring::Make(0, [layer, ghost, dx, dy, sx, sy](double t) {
        double u = 1 - t;
        layer->x->Set(dx * u);
        layer->y->Set(dy * u);
        layer->sx->Set(1 + (sx - 1) * u);
        layer->sy->Set(1 + (sy - 1) * u);
        layer->opacity->Set((std::clamp)(t, 0.0, 1.0));
        if (ghost) {
            auto framework = ghost.as<xaml::FrameworkElement>();
            ghost.CenterPoint(float3{static_cast<float>(framework.Width() / 2), static_cast<float>(framework.Height() / 2), 0});
            ghost.Translation(float3{static_cast<float>(-dx * t), static_cast<float>(-dy * t), 0});
            ghost.Scale(float3{static_cast<float>(1 + (1 / sx - 1) * t), static_cast<float>(1 + (1 / sy - 1) * t), 1});
            ghost.Opacity((std::clamp)(1 - t, 0.0, 1.0));
        }
    });
    progress->apply(0);
    auto win = from.win;
    progress->To(1, motion, [progress, ghost, win] {
        Departure gone{win, {}, nullptr, ghost};
        DropGhost(gone);
    });
}

/// `element` with `key` mounted: once laid out, grow from a departure.
inline void Enter(std::string const& key, xaml::UIElement const& element, Win* win, Motion const* motion) {
    auto framework = element.as<xaml::FrameworkElement>();
    auto token = std::make_shared<winrt::event_token>();
    *token = framework.SizeChanged([key, weak = winrt::make_weak(element), win, motion, token](auto&&, auto&&) {
        auto element = weak.get();
        if (!element) return;
        element.as<xaml::FrameworkElement>().SizeChanged(*token);
        auto found = departed.find(key);
        if (found == departed.end() || found->second.win != win || found->second.element == element) return;
        auto departure = found->second;
        departed.erase(found);
        Grow(element, departure, motion);
    });
}
}

/// The scroll view content is in: its viewer, axis, and the Composition
/// property set of its scrolling.
struct ScrollInfo {
    controls::ScrollViewer viewer{nullptr};
    bool horizontal = false;
    composition::CompositionPropertySet scrolling{nullptr};
    /// Rows of the lists inside, by key, for snapping and `position`.
    std::vector<std::pair<std::string, winrt::weak_ref<xaml::UIElement>>> items;
};

/// Scale and opacity by the distance of the element's center from the
/// middle of its scroll view: at the middle `scale` and full opacity; half
/// a viewport plus half its size away, plain size and `opacity`. A
/// Composition expression follows the scrolling.
inline void ScrollEffect(controls::Grid const& wrapper, std::shared_ptr<ScrollInfo> const& scroll, double scale, double opacity, double originX = 0.5, double originY = 0.5) {
    if (!scroll || !scroll->viewer || Reduced()) return;
    auto visual = xaml::Hosting::ElementCompositionPreview::GetElementVisual(wrapper);
    auto compositor = visual.Compositor();
    if (!scroll->scrolling) scroll->scrolling = xaml::Hosting::ElementCompositionPreview::GetScrollViewerManipulationPropertySet(scroll->viewer);
    auto props = compositor.CreatePropertySet();
    props.InsertScalar(L"center", 0);
    props.InsertScalar(L"port", 0);
    props.InsertScalar(L"reach", 1);
    std::wstring axis = scroll->horizontal ? L"X" : L"Y";
    std::wstring closeness = L"(1 - Min(1, Abs(props.center + scroll.Translation." + axis + L" - props.port / 2) / Max(1, props.reach)))";
    auto scaling = compositor.CreateExpressionAnimation(L"Vector3(1 + (" + std::to_wstring(scale) + L" - 1) * " + closeness + L", 1 + (" +
        std::to_wstring(scale) + L" - 1) * " + closeness + L", 1)");
    scaling.SetReferenceParameter(L"props", props);
    scaling.SetReferenceParameter(L"scroll", scroll->scrolling);
    auto fading = compositor.CreateExpressionAnimation(std::to_wstring(opacity) + L" + (1 - " + std::to_wstring(opacity) + L") * " + closeness);
    fading.SetReferenceParameter(L"props", props);
    fading.SetReferenceParameter(L"scroll", scroll->scrolling);
    visual.StartAnimation(L"Scale", scaling);
    visual.StartAnimation(L"Opacity", fading);
    auto viewer = scroll->viewer;
    bool horizontal = scroll->horizontal;
    auto update = [weak = winrt::make_weak(wrapper), viewer = winrt::make_weak(viewer), props, visual, horizontal, originX, originY] {
        auto element = weak.get();
        auto scroller = viewer.get();
        if (!element || !scroller || !scroller.Content()) return;
        try {
            auto content = scroller.Content().as<xaml::UIElement>();
            float w = static_cast<float>(element.ActualWidth()), h = static_cast<float>(element.ActualHeight());
            auto at = element.TransformToVisual(content).TransformPoint({w / 2, h / 2});
            float port = static_cast<float>(horizontal ? scroller.ViewportWidth() : scroller.ViewportHeight());
            props.InsertScalar(L"center", horizontal ? at.X : at.Y);
            props.InsertScalar(L"port", port);
            props.InsertScalar(L"reach", (port + (horizontal ? w : h)) / 2);
            visual.CenterPoint(float3{w * static_cast<float>(originX), h * static_cast<float>(originY), 0});
        } catch (winrt::hresult_error const&) {
        }
    };
    wrapper.LayoutUpdated([update](auto&&, auto&&) { update(); });
}

struct DecorateProps {
    Transition const* appear = nullptr;
    Transition const* disappear = nullptr;
    char const* hero = nullptr;
    Get heroKey;
    Motion const* heroMotion = nullptr;
    bool reveal = false;
    char const* appearance = nullptr;
    Get nudge;
    Motion const* nudgeMotion = nullptr;
    /// `nudge_key`: a nudge each time the value changes; `nudge_scale` how
    /// large it grows.
    Get nudgeKey;
    std::optional<double> nudgeScale;
    /// `appear_delay`: the entrance waits this much more, and plays on the
    /// first display too.
    std::optional<double> appearDelay;
    /// `reduce_motion`: the whole application runs with reduced motion
    /// while this holds and the element is shown.
    Get reduceMotion;
    std::optional<double> scrollScale;
    std::optional<double> scrollOpacity;
    /// The point of the element the scroll scale grows from, as fractions
    /// of its size; the center when not given.
    std::optional<double> scrollOriginX;
    std::optional<double> scrollOriginY;
};

inline xaml::ElementTheme ThemeNamed(std::string_view name) {
    if (name == "dark") return xaml::ElementTheme::Dark;
    if (name == "light") return xaml::ElementTheme::Light;
    return xaml::ElementTheme::Default;
}

/// The motion and appearance props any element takes, applied to the
/// element `build` makes: `appear` and `disappear` transitions, a `hero`
/// key, `reveal: hover`, `appearance`, `nudge`, and following scroll with
/// `scroll_scale` and `scroll_opacity`.
inline void Decorate(Parent const& p, Ctx const& ctx, DecorateProps props, Build const& build) {
    if (props.reduceMotion) {
        auto token = nextReduceHolder++;
        auto reduce = props.reduceMotion;
        Reactive::Run([ctx, reduce, token] { reduceHolders[token] = R::Truthy(reduce(ctx)); });
        Reactive::OnCleanup([token] { reduceHolders.erase(token); });
    }
    // Elements inside this one enter with it, not on their own; a
    // staggering container's clock is only for its direct children.
    bool animated = Structure::entering;
    auto clock = ctx.clock;
    Ctx inner = ctx;
    inner.clock = nullptr;
    if (props.appearance) inner.appearance = props.appearance;
    controls::Grid holder;
    Parent target = Parent::Of(holder);
    target.isList = p.isList;
    {
        Structure::Scope quiet(false);
        build(target, inner);
    }
    std::vector<xaml::UIElement> made;
    for (auto child : holder.Children()) made.push_back(child);
    holder.Children().Clear();
    for (auto const& element : made) {
        if (props.appearance)
            if (auto framework = element.try_as<xaml::FrameworkElement>()) framework.RequestedTheme(ThemeNamed(props.appearance));
        xaml::UIElement shown = element;
        if ((props.scrollScale || props.scrollOpacity) && ctx.scroll) {
            auto wrapper = Wrap(shown);
            ScrollEffect(wrapper, ctx.scroll, props.scrollScale.value_or(1), props.scrollOpacity.value_or(1),
                props.scrollOriginX.value_or(0.5), props.scrollOriginY.value_or(0.5));
            shown = wrapper;
        }
        // What moves goes on a wrapper of its own: the one that follows
        // scrolling belongs to its Composition expressions.
        bool moves = props.appear || props.disappear || props.hero || props.nudge || props.nudgeKey || props.reveal || props.scrollScale ||
            props.scrollOpacity;
        if (moves) shown = Wrap(shown);
        p.Add(shown);
        // With a delay the entrance plays on the first display too.
        if (props.appear && (animated || (clock && clock->play) || props.appearDelay))
            PlayTransition(shown, *props.appear, true, nullptr, (clock && clock->play ? Turn(clock) : 0) + props.appearDelay.value_or(0));
        if (props.disappear) {
            auto hook = winrt::make_self<LeaveHook>();
            auto transition = *props.disappear;
            hook->leave = [weak = winrt::make_weak(shown), transition](std::function<void()> done) {
                auto element = weak.get();
                if (!element) return done();
                element.IsHitTestVisible(false);
                PlayTransition(element, transition, false, done);
            };
            shown.as<xaml::FrameworkElement>().Resources().Insert(winrt::box_value(L"bern.leave"), hook.as<IInspectable>());
        }
        if (props.hero && ctx.win) {
            std::string key = props.hero;
            if (props.heroKey) key += "-" + R::KeyText(Reactive::Untrack([&] { return props.heroKey(ctx); }));
            Hero::Enter(key, shown, ctx.win, props.heroMotion);
            Reactive::OnCleanup([key, weak = winrt::make_weak(shown), win = ctx.win] {
                if (auto element = weak.get()) Hero::Leave(key, element, win);
            });
        }
        if (props.nudgeKey) {
            auto key = props.nudgeKey;
            auto motion = props.nudgeMotion;
            double scale = props.nudgeScale.value_or(1.06);
            auto before = std::make_shared<Value>(Reactive::Untrack([&] { return key(ctx); }));
            Reactive::Run([key, ctx, before, motion, scale, weak = winrt::make_weak(shown)] {
                auto now = key(ctx);
                if (!(now == *before))
                    if (auto element = weak.get()) Reactive::Untrack([&] {
                        NudgeOnce(element, motion, scale);
                        return 0;
                    });
                *before = now;
            });
        }
        if (props.nudge) {
            auto when = props.nudge;
            auto motion = props.nudgeMotion;
            double scale = props.nudgeScale.value_or(1.06);
            auto before = std::make_shared<bool>(Reactive::Untrack([&] { return R::Truthy(when(ctx)); }));
            Reactive::Run([when, ctx, before, motion, scale, weak = winrt::make_weak(shown)] {
                bool now = R::Truthy(when(ctx));
                if (now && !*before)
                    if (auto element = weak.get()) Reactive::Untrack([&] {
                        NudgeOnce(element, motion, scale);
                        return 0;
                    });
                *before = now;
            });
        }
        if (props.reveal && p.panel) {
            // Shown while the pointer is over the container, or the
            // element has the keyboard focus.
            LayersOf(shown)->Get("reveal")->opacity->Set(0);
            static Motion const fade{0.2, 1};
            auto state = std::make_shared<std::pair<bool, bool>>(false, false);
            auto weak = winrt::make_weak(shown);
            auto show = [weak, state] {
                if (auto element = weak.get()) LayerTo(element, "reveal", 0, 0, 1, 1, state->first || state->second ? 1 : 0, &fade);
            };
            using PointerHandler = xaml::Input::PointerEventHandler;
            p.panel.AddHandler(xaml::UIElement::PointerEnteredEvent(), winrt::box_value(PointerHandler([state, show](auto&&, auto&&) {
                state->first = true;
                show();
            })), true);
            p.panel.AddHandler(xaml::UIElement::PointerExitedEvent(), winrt::box_value(PointerHandler([state, show](auto&&, auto&&) {
                state->first = false;
                show();
            })), true);
            shown.GotFocus([state, show](auto&&, auto&&) {
                state->second = true;
                show();
            });
            shown.LostFocus([state, show](auto&&, auto&&) {
                state->second = false;
                show();
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Rails

/// A strip that places each child at a fraction of its length, or between
/// two. Across the rail the children share one line, so it is as thick as
/// the thickest. Along it the rail asks for no length where one is given:
/// it takes the length it is placed with, which rows and columns give by
/// its role and a stack by its other layers. The rail takes no clicks; its
/// children do.
struct RailPanel : controls::PanelT<RailPanel> {
    bool vertical = false;
    /// Which point of an element sits at its fraction: 0 its start, 0.5
    /// its center, 1 its end.
    double anchor = 0.5;
    /// How far the scale runs past each end.
    double overhang = 0;
    struct Spot {
        winrt::weak_ref<xaml::UIElement> element;
        bool span = false;
        double at = 0, from = 0, to = 0;
    };
    std::vector<std::shared_ptr<Spot>> spots;

    std::shared_ptr<Spot> SpotOf(xaml::UIElement const& child) const {
        for (auto const& spot : spots)
            if (spot->element.get() == child) return spot;
        return nullptr;
    }

    static double Fraction(double value) { return (std::clamp)(std::isfinite(value) ? value : 0.0, 0.0, 1.0); }

    Size MeasureOverride(Size available) {
        float length = vertical ? available.Height : available.Width;
        float across = vertical ? available.Width : available.Height;
        float thickest = 0, longest = 0;
        for (auto child : Children()) {
            auto spot = SpotOf(child);
            float along = INFINITY;
            if (spot && spot->span && !std::isinf(length)) along = static_cast<float>(std::abs(Fraction(spot->to) - Fraction(spot->from)) * (length + 2 * overhang));
            child.Measure(vertical ? Size{across, along} : Size{along, across});
            auto desired = child.DesiredSize();
            thickest = (std::max)(thickest, vertical ? desired.Width : desired.Height);
            longest = (std::max)(longest, vertical ? desired.Height : desired.Width);
        }
        // Asking for the room it was offered would make a stack as large
        // as that room, a progress bar wider than the picture it is on.
        float main = std::isinf(length) ? longest : 0;
        return vertical ? Size{thickest, main} : Size{main, thickest};
    }

    Size ArrangeOverride(Size final) {
        // The scale: the rail's length and the overhang past both ends.
        float past = static_cast<float>(overhang);
        float length = (vertical ? final.Height : final.Width) + 2 * past;
        float across = vertical ? final.Width : final.Height;
        for (auto child : Children()) {
            auto spot = SpotOf(child);
            auto desired = child.DesiredSize();
            float size = vertical ? desired.Height : desired.Width;
            float start = 0;
            if (spot && spot->span) {
                double a = Fraction(spot->from), b = Fraction(spot->to);
                start = static_cast<float>((std::min)(a, b) * length) - past;
                size = static_cast<float>(std::abs(b - a) * length);
            } else if (spot) {
                start = static_cast<float>(Fraction(spot->at) * length - anchor * size) - past;
            }
            child.Arrange(vertical ? Rect{0, start, across, size} : Rect{start, 0, size, across});
        }
        return final;
    }
};

struct RailProps {
    char const* axis = nullptr;
    char const* anchor = nullptr;
    Len overhang;
};

inline void Rail(Parent const& p, Ctx const& ctx, RailProps props, Build const& children) {
    auto impl = winrt::make_self<RailPanel>();
    impl->vertical = props.axis && std::string_view(props.axis) == "vertical";
    std::string_view anchor = props.anchor ? props.anchor : "center";
    impl->anchor = anchor == "start" ? 0.0 : anchor == "end" ? 1.0 : 0.5;
    controls::Panel panel = impl.as<controls::Panel>();
    if (props.overhang.Given()) {
        auto rail = impl.get();
        BindLen(ctx, props.overhang, nullptr, [rail, weak = winrt::make_weak(panel)](double value) {
            rail->overhang = value;
            if (auto panel = weak.get()) panel.InvalidateArrange();
        });
    }
    // It fills only along its axis; across, it is as thick as its children
    // unless one of them fills that way.
    SetRole(panel, impl->vertical ? L"bern-rail-h" : L"bern-rail-w");
    auto inner = ctx;
    inner.rail = std::make_shared<RailInfo const>(RailInfo{impl->vertical, std::string(anchor), impl.get(), panel});
    children(Parent::Of(panel), inner);
    p.Add(panel);
}

struct PlaceProps {
    Len at, from, to;
    Motion const* animate = nullptr;
};

/// Places the element `build` makes on the rail it stands in. Fractions
/// outside 0…1 stop at the ends; with `animate` the element springs to a
/// new place.
inline void Place(Parent const& p, Ctx const& ctx, PlaceProps props, Build const& build) {
    auto rail = ctx.rail;
    if (!rail || !rail->panel) {
        build(p, ctx);
        return;
    }
    std::vector<xaml::UIElement> before;
    for (auto child : rail->element.Children()) before.push_back(child);
    build(p, ctx);
    std::vector<xaml::UIElement> added;
    for (auto child : rail->element.Children())
        if (std::find(before.begin(), before.end(), child) == before.end()) added.push_back(child);
    for (auto const& element : added) {
        auto spot = std::make_shared<RailPanel::Spot>();
        spot->element = winrt::make_weak(element);
        spot->span = !props.at.Given();
        rail->panel->spots.push_back(spot);
        auto panel = rail->element;
        auto relayout = [panel] { panel.InvalidateMeasure(); };
        if (spot->span) {
            BindLen(ctx, props.from, props.animate, [spot, relayout](double v) { spot->from = v; relayout(); });
            BindLen(ctx, props.to, props.animate, [spot, relayout](double v) { spot->to = v; relayout(); });
        } else {
            BindLen(ctx, props.at, props.animate, [spot, relayout](double v) { spot->at = v; relayout(); });
        }
        RailPanel* owner = rail->panel;
        Reactive::OnCleanup([owner, spot, keep = winrt::make_weak(panel)] {
            if (keep.get()) std::erase(owner->spots, spot);
        });
    }
}

}
