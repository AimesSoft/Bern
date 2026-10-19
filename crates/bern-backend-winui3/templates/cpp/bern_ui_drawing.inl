// Bern runtime for Windows, XAML part 3 of 7: drawing. A `Canvas` draws
// its shapes, series and patterns with Direct2D into a bitmap at the
// display's scale, shown by an `Image`: the same code in both front ends,
// redrawn when the state it shows, its size, its theme or the accent
// changes. Text marks are shapes behind the lines of a text block, and
// the shadows of styled buttons are blurred shapes behind them.
// Generated build input; do not edit.

namespace B {

namespace Draw {

inline winrt::com_ptr<ID2D1Factory1> const& Factory() {
    static winrt::com_ptr<ID2D1Factory1> factory = [] {
        winrt::com_ptr<ID2D1Factory1> made;
        D2D1_FACTORY_OPTIONS options{};
        D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, __uuidof(ID2D1Factory1), &options, made.put_void());
        return made;
    }();
    return factory;
}

inline winrt::com_ptr<IWICImagingFactory> const& Imaging() {
    static winrt::com_ptr<IWICImagingFactory> factory = [] {
        winrt::com_ptr<IWICImagingFactory> made;
        CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(made.put()));
        return made;
    }();
    return factory;
}

inline D2D1_COLOR_F ColorF(winrt::Windows::UI::Color color, double opacity = 1) {
    return D2D1::ColorF(color.R / 255.0f, color.G / 255.0f, color.B / 255.0f, static_cast<float>(color.A / 255.0 * (std::clamp)(opacity, 0.0, 1.0)));
}

/// A surface to draw on: a WIC bitmap with a Direct2D render target, at
/// `scale` pixels per unit, copied into a XAML bitmap when done.
struct Surface {
    UINT width = 0, height = 0;
    winrt::com_ptr<IWICBitmap> bitmap;
    winrt::com_ptr<ID2D1RenderTarget> target;
    winrt::com_ptr<ID2D1DeviceContext> context;

    bool Begin(double w, double h, double scale) {
        width = static_cast<UINT>((std::max)(1L, std::lround(w * scale)));
        height = static_cast<UINT>((std::max)(1L, std::lround(h * scale)));
        if (!Imaging() || !Factory()) return false;
        if (FAILED(Imaging()->CreateBitmap(width, height, GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad, bitmap.put()))) return false;
        auto properties = D2D1::RenderTargetProperties(D2D1_RENDER_TARGET_TYPE_DEFAULT,
            D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), static_cast<float>(96 * scale), static_cast<float>(96 * scale));
        if (FAILED(Factory()->CreateWicBitmapRenderTarget(bitmap.get(), properties, target.put()))) return false;
        context = target.try_as<ID2D1DeviceContext>();
        target->BeginDraw();
        target->Clear(D2D1::ColorF(0, 0, 0, 0));
        return true;
    }

    /// Finish drawing and copy the pixels into `into`, made anew when its
    /// size differs.
    bool End(media::Imaging::WriteableBitmap& into) {
        if (FAILED(target->EndDraw())) return false;
        if (!into || static_cast<UINT>(into.PixelWidth()) != width || static_cast<UINT>(into.PixelHeight()) != height)
            into = media::Imaging::WriteableBitmap(static_cast<int32_t>(width), static_cast<int32_t>(height));
        WICRect rect{0, 0, static_cast<INT>(width), static_cast<INT>(height)};
        winrt::com_ptr<IWICBitmapLock> lock;
        if (FAILED(bitmap->Lock(&rect, WICBitmapLockRead, lock.put()))) return false;
        UINT stride = 0, size = 0;
        BYTE* data = nullptr;
        lock->GetStride(&stride);
        lock->GetDataPointer(&size, &data);
        auto buffer = into.PixelBuffer();
        uint8_t* pixels = buffer.data();
        for (UINT row = 0; row < height; ++row) std::memcpy(pixels + row * width * 4, data + row * stride, width * 4);
        into.Invalidate();
        return true;
    }
};

}

// ---------------------------------------------------------------------------
// The average color of pictures, for shapes filled from a picture

namespace Pictures {

/// Read a picture's bytes: a `file:` URL from disk, an http(s) one over
/// the network.
inline winrt::Windows::Foundation::IAsyncOperation<winrt::Windows::Storage::Streams::IBuffer> Read(std::string url) {
    if (auto path = Host::FilePath(url)) {
        auto file = co_await winrt::Windows::Storage::StorageFile::GetFileFromPathAsync(*path);
        co_return co_await winrt::Windows::Storage::FileIO::ReadBufferAsync(file);
    }
    auto reference = winrt::Windows::Storage::Streams::RandomAccessStreamReference::CreateFromUri(winrt::Windows::Foundation::Uri(Wide(url)));
    auto stream = co_await reference.OpenReadAsync();
    winrt::Windows::Storage::Streams::Buffer buffer(static_cast<uint32_t>(stream.Size()));
    co_return co_await stream.ReadAsync(buffer, buffer.Capacity(), winrt::Windows::Storage::Streams::InputStreamOptions::None);
}

/// The mean color of encoded picture bytes, over a 32 × 32 copy.
inline std::optional<winrt::Windows::UI::Color> Average(uint8_t const* bytes, size_t size) {
    auto wic = Draw::Imaging();
    if (!wic || !bytes || !size) return std::nullopt;
    winrt::com_ptr<IStream> stream;
    stream.attach(SHCreateMemStream(bytes, static_cast<UINT>(size)));
    if (!stream) return std::nullopt;
    winrt::com_ptr<IWICBitmapDecoder> decoder;
    if (FAILED(wic->CreateDecoderFromStream(stream.get(), nullptr, WICDecodeMetadataCacheOnDemand, decoder.put()))) return std::nullopt;
    winrt::com_ptr<IWICBitmapFrameDecode> frame;
    if (FAILED(decoder->GetFrame(0, frame.put()))) return std::nullopt;
    winrt::com_ptr<IWICBitmapScaler> scaler;
    if (FAILED(wic->CreateBitmapScaler(scaler.put())) || FAILED(scaler->Initialize(frame.get(), 32, 32, WICBitmapInterpolationModeFant))) return std::nullopt;
    winrt::com_ptr<IWICFormatConverter> converter;
    if (FAILED(wic->CreateFormatConverter(converter.put())) ||
        FAILED(converter->Initialize(scaler.get(), GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, nullptr, 0, WICBitmapPaletteTypeCustom)))
        return std::nullopt;
    std::vector<uint8_t> pixels(32 * 32 * 4);
    if (FAILED(converter->CopyPixels(nullptr, 32 * 4, static_cast<UINT>(pixels.size()), pixels.data()))) return std::nullopt;
    uint64_t red = 0, green = 0, blue = 0;
    for (size_t at = 0; at < pixels.size(); at += 4) {
        blue += pixels[at];
        green += pixels[at + 1];
        red += pixels[at + 2];
    }
    uint64_t count = pixels.size() / 4;
    return winrt::Windows::UI::Color{255, static_cast<uint8_t>(red / count), static_cast<uint8_t>(green / count), static_cast<uint8_t>(blue / count)};
}

struct Entry {
    Cell<std::optional<winrt::Windows::UI::Color>> average;
    bool started = false;
};

inline std::map<std::string, std::shared_ptr<Entry>>& Entries() {
    static std::map<std::string, std::shared_ptr<Entry>> entries;
    return entries;
}

inline winrt::fire_and_forget Load(std::string url, std::shared_ptr<Entry> entry) {
    winrt::apartment_context ui;
    try {
        auto buffer = co_await Read(url);
        co_await winrt::resume_background();
        auto color = Average(buffer.data(), buffer.Length());
        co_await ui;
        if (color) entry->average.Set(color);
    } catch (winrt::hresult_error const& error) {
        Report("picture " + url + ": " + Utf8(error.message()));
    }
}

/// The average color of the picture at `url`, once read; nothing before.
/// Each URL is read once.
inline std::optional<winrt::Windows::UI::Color> AverageColor(std::optional<std::string> const& url) {
    if (!url) return std::nullopt;
    auto& entries = Entries();
    auto found = entries.find(*url);
    if (found == entries.end()) found = entries.emplace(*url, std::make_shared<Entry>()).first;
    auto entry = found->second;
    if (!entry->started) {
        entry->started = true;
        Load(*url, entry);
    }
    return entry->average.Get();
}

}

/// A declared resource as a `file:` URL, next to the executable.
inline std::optional<std::string> ResourceUrl(char const* name) {
    std::wstring path(MAX_PATH, L'\0');
    for (;;) {
        auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
        if (length < path.size()) {
            path.resize(length);
            break;
        }
        path.resize(path.size() * 2);
    }
    std::wstring file = path.substr(0, path.find_last_of(L'\\')) + L"\\resources\\" + std::wstring(Wide(name));
    std::replace(file.begin(), file.end(), L'/', L'\\');
    auto url = Host::FileUrl(file);
    if (url.empty()) return std::nullopt;
    return url;
}

// ---------------------------------------------------------------------------
// Canvas

/// A shape in canvas units; without a size it fills the canvas.
struct ShapeOp {
    char const* kind = "rect";
    std::optional<double> x, y, width, height, corner;
    std::optional<Paint> fill;
    std::optional<std::string> fillFrom;
    char const* fade = nullptr;
    std::optional<Paint> stroke;
    std::optional<double> strokeWidth;
    /// Dashes of the stroke that long, with gaps of `dashGap`, by default
    /// as long; none draws it whole.
    std::optional<double> dash, dashGap;
    std::optional<double> opacity;
};

/// Numbers across the canvas width as an area or a line.
struct SeriesOp {
    std::vector<double> values;
    std::optional<double> max;
    bool area = false;
    std::optional<Paint> fill, stroke;
    std::optional<double> strokeWidth, opacity;
};

/// A pattern of `style!` tiled over the canvas.
struct PatternOp {
    PatternDef const* pattern = nullptr;
    std::optional<double> opacity;
};

using DrawOp = std::variant<ShapeOp, SeriesOp, PatternOp>;

namespace Draw {

/// The colors of a drawing: roles in the theme the canvas shows.
struct Colors {
    Win* win = nullptr;
    bool dark = false;
    D2D1_COLOR_F Of(Paint const& paint, double opacity = 1) const { return ColorF(ColorOf(win, dark, paint), opacity); }
};

inline winrt::com_ptr<ID2D1Geometry> ShapeGeometry(ShapeOp const& op, D2D1_RECT_F rect) {
    winrt::com_ptr<ID2D1Geometry> geometry;
    std::string_view kind = op.kind ? op.kind : "rect";
    float width = rect.right - rect.left, height = rect.bottom - rect.top;
    if (kind == "ellipse") {
        winrt::com_ptr<ID2D1EllipseGeometry> ellipse;
        Factory()->CreateEllipseGeometry(D2D1::Ellipse(D2D1::Point2F(rect.left + width / 2, rect.top + height / 2), (std::max)(0.0f, width / 2), (std::max)(0.0f, height / 2)), ellipse.put());
        geometry = ellipse;
    } else {
        float radius = kind == "rounded" ? static_cast<float>(op.corner.value_or(8)) : kind == "capsule" ? (std::min)(width, height) / 2 : 0.0f;
        radius = (std::max)(0.0f, (std::min)({radius, width / 2, height / 2}));
        winrt::com_ptr<ID2D1RoundedRectangleGeometry> rounded;
        Factory()->CreateRoundedRectangleGeometry(D2D1::RoundedRect(rect, radius, radius), rounded.put());
        geometry = rounded;
    }
    return geometry;
}

inline void DrawShape(ID2D1RenderTarget* target, ShapeOp const& op, Colors const& colors, float w, float h) {
    float x = static_cast<float>(op.x.value_or(0)), y = static_cast<float>(op.y.value_or(0));
    float width = static_cast<float>(op.width.value_or(w - x)), height = static_cast<float>(op.height.value_or(h - y));
    auto rect = D2D1::RectF(x, y, x + width, y + height);
    auto geometry = ShapeGeometry(op, rect);
    if (!geometry) return;
    double opacity = op.opacity.value_or(1);
    std::optional<D2D1_COLOR_F> filled;
    if (op.fillFrom) {
        if (auto average = Pictures::AverageColor(op.fillFrom)) filled = ColorF(*average, opacity);
    }
    if (!filled && (op.fill || !op.stroke)) filled = colors.Of(op.fill.value_or(Paint{"fill.strong"}), opacity);
    if (filled) {
        if (op.fade) {
            std::string_view fade = op.fade;
            D2D1_POINT_2F start{x, y}, end{x, y + height};
            if (fade == "up") {
                start = {x, y + height};
                end = {x, y};
            } else if (fade == "left") {
                start = {x + width, y};
                end = {x, y};
            } else if (fade == "right") {
                end = {x + width, y};
            }
            auto clear = *filled;
            clear.a = 0;
            D2D1_GRADIENT_STOP stops[2] = {{0, *filled}, {1, clear}};
            winrt::com_ptr<ID2D1GradientStopCollection> collection;
            target->CreateGradientStopCollection(stops, 2, collection.put());
            winrt::com_ptr<ID2D1LinearGradientBrush> brush;
            target->CreateLinearGradientBrush(D2D1::LinearGradientBrushProperties(start, end), collection.get(), brush.put());
            if (brush) target->FillGeometry(geometry.get(), brush.get());
        } else {
            winrt::com_ptr<ID2D1SolidColorBrush> brush;
            target->CreateSolidColorBrush(*filled, brush.put());
            target->FillGeometry(geometry.get(), brush.get());
        }
    }
    if (op.stroke) {
        winrt::com_ptr<ID2D1SolidColorBrush> brush;
        target->CreateSolidColorBrush(colors.Of(*op.stroke, opacity), brush.put());
        float line = static_cast<float>(op.strokeWidth.value_or(1));
        // Inside the bounds, as the stroke of a frame is.
        auto inset = D2D1::RectF(x + line / 2, y + line / 2, x + width - line / 2, y + height - line / 2);
        auto stroked = ShapeGeometry(op, inset);
        winrt::com_ptr<ID2D1StrokeStyle> style;
        if (op.dash && line > 0) {
            // Direct2D counts dashes in stroke widths.
            float dashes[2] = {static_cast<float>(*op.dash) / line, static_cast<float>(op.dashGap.value_or(*op.dash)) / line};
            static_cast<ID2D1Factory*>(Factory().get())
                ->CreateStrokeStyle(D2D1::StrokeStyleProperties(D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_FLAT, D2D1_CAP_STYLE_FLAT, D2D1_LINE_JOIN_MITER, 10.0f, D2D1_DASH_STYLE_CUSTOM, 0.0f), dashes, 2, style.put());
        }
        target->DrawGeometry(stroked.get(), brush.get(), line, style.get());
    }
}

inline void DrawSeries(ID2D1RenderTarget* target, SeriesOp const& op, Colors const& colors, float w, float h) {
    if (op.values.empty()) return;
    double peak = op.max.value_or(*std::max_element(op.values.begin(), op.values.end()));
    double scale = peak > 0 ? peak : 1;
    size_t count = op.values.size();
    float step = count > 1 ? w / static_cast<float>(count - 1) : 0;
    std::vector<D2D1_POINT_2F> points;
    for (size_t i = 0; i < count; ++i)
        points.push_back({count > 1 ? i * step : w / 2, h - h * static_cast<float>((std::clamp)(op.values[i] / scale, 0.0, 1.0))});
    auto path = [&](bool closed) {
        winrt::com_ptr<ID2D1PathGeometry> geometry;
        Factory()->CreatePathGeometry(geometry.put());
        winrt::com_ptr<ID2D1GeometrySink> sink;
        geometry->Open(sink.put());
        sink->BeginFigure(points[0], closed ? D2D1_FIGURE_BEGIN_FILLED : D2D1_FIGURE_BEGIN_HOLLOW);
        for (size_t i = 1; i < points.size(); ++i) sink->AddLine(points[i]);
        if (closed) {
            sink->AddLine({points.back().x, h});
            sink->AddLine({points.front().x, h});
        }
        sink->EndFigure(closed ? D2D1_FIGURE_END_CLOSED : D2D1_FIGURE_END_OPEN);
        sink->Close();
        return geometry;
    };
    double opacity = op.opacity.value_or(1);
    winrt::com_ptr<ID2D1StrokeStyle> round;
    static_cast<ID2D1Factory*>(Factory().get())
        ->CreateStrokeStyle(D2D1::StrokeStyleProperties(D2D1_CAP_STYLE_ROUND, D2D1_CAP_STYLE_ROUND, D2D1_CAP_STYLE_ROUND, D2D1_LINE_JOIN_ROUND), nullptr, 0,
            round.put());
    float line = static_cast<float>(op.strokeWidth.value_or(2));
    if (op.area) {
        winrt::com_ptr<ID2D1SolidColorBrush> brush;
        target->CreateSolidColorBrush(colors.Of(op.fill.value_or(Paint{"accent"}), opacity), brush.put());
        target->FillGeometry(path(true).get(), brush.get());
        if (op.stroke) {
            winrt::com_ptr<ID2D1SolidColorBrush> stroke;
            target->CreateSolidColorBrush(colors.Of(*op.stroke, opacity), stroke.put());
            target->DrawGeometry(path(false).get(), stroke.get(), line, round.get());
        }
    } else {
        winrt::com_ptr<ID2D1SolidColorBrush> brush;
        target->CreateSolidColorBrush(colors.Of(op.stroke.value_or(Paint{"accent"}), opacity), brush.put());
        target->DrawGeometry(path(false).get(), brush.get(), line, round.get());
    }
}

/// A woven check: the bands repeat across and down, and where colored
/// bands cross both show; with `twill`, fine diagonal lines over it.
inline void DrawPattern(ID2D1RenderTarget* target, PatternOp const& op, Colors const& colors, float w, float h) {
    if (!op.pattern) return;
    double sett = 0;
    for (auto const& band : op.pattern->bands) sett += (std::max)(band.width, 0.0);
    if (sett <= 0) return;
    double alpha = op.opacity.value_or(1);
    for (bool across : {true, false}) {
        float extent = across ? w : h;
        float offset = 0;
        while (offset < extent) {
            for (auto const& band : op.pattern->bands) {
                if (band.width <= 0) continue;
                if (band.color.Given()) {
                    winrt::com_ptr<ID2D1SolidColorBrush> brush;
                    target->CreateSolidColorBrush(colors.Of(band.color, alpha * band.opacity), brush.put());
                    float size = static_cast<float>(band.width);
                    target->FillRectangle(across ? D2D1::RectF(offset, 0, offset + size, h) : D2D1::RectF(0, offset, w, offset + size), brush.get());
                }
                offset += static_cast<float>(band.width);
                if (offset >= extent) break;
            }
        }
    }
    if (op.pattern->twill) {
        winrt::com_ptr<ID2D1SolidColorBrush> brush;
        target->CreateSolidColorBrush(colors.Of(Paint{"text.primary"}, alpha * 0.05), brush.put());
        for (float start = -h; start < w; start += 3) target->DrawLine({start, h}, {start + h, 0}, brush.get(), 0.5f);
    }
}

}

struct CanvasProps {
    Len width, height;
    Str label;
    Motion const* animate = nullptr;
};

/// The scale of pixels to units where `element` is shown.
inline double ScaleOf(xaml::UIElement const& element) {
    if (auto root = element.XamlRoot()) return root.RasterizationScale();
    return 1;
}

/// `Canvas`: shapes, series and patterns drawn in order, in units. `ops`
/// reads the state the drawing shows, so it redraws when that changes, as
/// it does when the canvas resizes or its colors change.
inline void Canvas(Parent const& p, Ctx const& ctx, CanvasProps props, std::function<std::vector<DrawOp>(Ctx const&)> ops) {
    // The drawing has no size of its own: the image sits in a XAML canvas,
    // which asks for none, and takes the frame's size.
    controls::Grid frame;
    controls::Canvas surface;
    controls::Image image;
    image.Stretch(media::Stretch::Fill);
    image.IsHitTestVisible(false);
    surface.IsHitTestVisible(false);
    surface.Children().Append(image);
    frame.Children().Append(surface);
    bool sizedWidth = props.width.Given(), sizedHeight = props.height.Given();
    // Without a size it fills where it stands; among a stack's layers it
    // takes the size of the others, as their backdrop.
    if (!sizedWidth && !sizedHeight) SetRole(frame, L"bern-canvas");
    else if (!sizedWidth) SetRole(frame, L"bern-fill-width");
    else if (!sizedHeight) SetRole(frame, L"bern-fill-height");
    if (!sizedWidth) frame.HorizontalAlignment(xaml::HorizontalAlignment::Stretch);
    if (!sizedHeight) frame.VerticalAlignment(xaml::VerticalAlignment::Stretch);
    BindLen(ctx, props.width, props.animate, [frame](double v) { frame.Width((std::max)(0.0, v)); });
    BindLen(ctx, props.height, props.animate, [frame](double v) { frame.Height((std::max)(0.0, v)); });
    if (props.label) {
        auto label = props.label;
        Reactive::Run([frame, label, ctx] { xaml::Automation::AutomationProperties::SetName(frame, label(ctx)); });
    } else {
        xaml::Automation::AutomationProperties::SetAccessibilityView(frame, xaml::Automation::Peers::AccessibilityView::Raw);
    }
    // What the drawing depends on besides the state it shows.
    Cell<std::array<double, 3>> size(std::array<double, 3>{0, 0, 1});
    Cell<bool> dark(false);
    frame.SizeChanged([size, image, weak = winrt::make_weak(frame)](auto&&, xaml::SizeChangedEventArgs const& args) mutable {
        auto element = weak.get();
        image.Width(args.NewSize().Width);
        image.Height(args.NewSize().Height);
        size.Set({args.NewSize().Width, args.NewSize().Height, element ? ScaleOf(element) : 1.0});
    });
    frame.ActualThemeChanged([dark](xaml::FrameworkElement const& element, auto&&) mutable { dark.Set(IsDark(element)); });
    frame.Loaded([dark, size](IInspectable const& sender, auto&&) mutable {
        auto element = sender.as<xaml::FrameworkElement>();
        dark.Set(IsDark(element));
        auto current = size.Peek();
        current[2] = ScaleOf(element);
        size.Set(current);
    });
    auto bitmap = std::make_shared<media::Imaging::WriteableBitmap>(nullptr);
    Reactive::Run([ctx, ops, size, dark, image, bitmap] {
        auto list = ops(ctx);
        auto [w, h, scale] = size.Get();
        bool night = dark.Get();
        themeTick.Get();
        if (w <= 0 || h <= 0) return;
        Draw::Surface surface;
        if (!surface.Begin(w, h, scale)) return;
        Draw::Colors colors{ctx.win, night};
        auto* target = surface.target.get();
        for (auto const& op : list) {
            if (auto shape = std::get_if<ShapeOp>(&op)) Draw::DrawShape(target, *shape, colors, static_cast<float>(w), static_cast<float>(h));
            else if (auto series = std::get_if<SeriesOp>(&op)) Draw::DrawSeries(target, *series, colors, static_cast<float>(w), static_cast<float>(h));
            else if (auto pattern = std::get_if<PatternOp>(&op)) Draw::DrawPattern(target, *pattern, colors, static_cast<float>(w), static_cast<float>(h));
        }
        auto previous = *bitmap;
        if (surface.End(*bitmap) && *bitmap != previous) image.Source(*bitmap);
    });
    p.Add(frame);
}

// ---------------------------------------------------------------------------
// Text marks

/// A capsule of the mark color under each line of `block`, from 74 % to
/// 98 % of the line's height and 0.4 em longer than the line, behind the
/// text without taking space.
inline void DrawMarks(controls::TextBlock const& block, controls::Canvas const& layer, Paint const& paint) {
    layer.Children().Clear();
    auto start = block.ContentStart(), end = block.ContentEnd();
    if (!start || !end) return;
    struct Line {
        float top, height, left, right;
    };
    std::vector<Line> lines;
    auto add = [&](Rect rect) {
        if (rect.Height <= 0) return;
        for (auto& line : lines)
            if (std::abs(line.top - rect.Y) < 1) {
                line.left = (std::min)(line.left, rect.X);
                line.right = (std::max)(line.right, rect.X + rect.Width);
                line.height = (std::max)(line.height, rect.Height);
                return;
            }
        lines.push_back({rect.Y, rect.Height, rect.X, rect.X + rect.Width});
    };
    using Direction = xaml::Documents::LogicalDirection;
    for (int offset = 0; offset < 100000; ++offset) {
        auto pointer = start.GetPositionAtOffset(offset, Direction::Forward);
        if (!pointer || pointer.Offset() > end.Offset()) break;
        add(pointer.GetCharacterRect(Direction::Forward));
        add(pointer.GetCharacterRect(Direction::Backward));
        if (pointer.Offset() >= end.Offset()) break;
    }
    float em = static_cast<float>(block.FontSize());
    for (auto const& line : lines) {
        if (line.right - line.left <= 0) continue;
        float band = line.height * 0.24f;
        xaml::Shapes::Rectangle mark;
        mark.Width(line.right - line.left + em * 0.4f);
        mark.Height(band);
        mark.RadiusX(band / 2);
        mark.RadiusY(band / 2);
        FillShape(mark, paint);
        controls::Canvas::SetLeft(mark, line.left);
        controls::Canvas::SetTop(mark, line.top + line.height * 0.74f);
        layer.Children().Append(mark);
    }
}

// ---------------------------------------------------------------------------
// Shadows of styled controls

/// A shadow below a control, as if lifted off the page: `radius` of blur,
/// `y` down, black at `opacity`; drawn only outside the control's shape.
struct ShadowImage {
    controls::Image image;
    double radius = 0, y = 0, opacity = 0, corner = 4;
    double width = 0, height = 0, scale = 1;
    media::Imaging::WriteableBitmap bitmap{nullptr};

    double Pad() const { return radius * 2 + std::abs(y); }

    void Draw() {
        if (width <= 0 || height <= 0 || radius < 0) return;
        double pad = Pad();
        Draw::Surface surface;
        if (!surface.Begin(width + pad * 2, height + pad * 2, scale)) return;
        auto rect = D2D1::RectF(static_cast<float>(pad), static_cast<float>(pad + y), static_cast<float>(pad + width), static_cast<float>(pad + y + height));
        float r = static_cast<float>((std::min)({corner, width / 2, height / 2}));
        if (surface.context) {
            // The shape on a layer of its own, blurred by the shadow effect.
            winrt::com_ptr<ID2D1Bitmap1> shape;
            auto properties = D2D1::BitmapProperties1(D2D1_BITMAP_OPTIONS_TARGET,
                D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), static_cast<float>(96 * scale), static_cast<float>(96 * scale));
            if (SUCCEEDED(surface.context->CreateBitmap(D2D1::SizeU(surface.width, surface.height), nullptr, 0, properties, shape.put()))) {
                winrt::com_ptr<ID2D1Image> previous;
                surface.context->GetTarget(previous.put());
                surface.context->SetTarget(shape.get());
                surface.context->Clear(D2D1::ColorF(0, 0, 0, 0));
                winrt::com_ptr<ID2D1SolidColorBrush> black;
                surface.context->CreateSolidColorBrush(D2D1::ColorF(0, 0, 0, 1), black.put());
                surface.context->FillRoundedRectangle(D2D1::RoundedRect(rect, r, r), black.get());
                surface.context->SetTarget(previous.get());
                winrt::com_ptr<ID2D1Effect> shadow;
                if (SUCCEEDED(surface.context->CreateEffect(CLSID_D2D1Shadow, shadow.put()))) {
                    shadow->SetInput(0, shape.get());
                    shadow->SetValue(D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION, static_cast<float>(radius / 2));
                    shadow->SetValue(D2D1_SHADOW_PROP_COLOR, D2D1::Vector4F(0, 0, 0, static_cast<float>(opacity)));
                    surface.context->DrawImage(shadow.get());
                }
                // Only outside the control: clear its shape.
                winrt::com_ptr<ID2D1SolidColorBrush> clear;
                surface.context->CreateSolidColorBrush(D2D1::ColorF(0, 0, 0, 0), clear.put());
                auto hole = D2D1::RectF(static_cast<float>(pad), static_cast<float>(pad), static_cast<float>(pad + width), static_cast<float>(pad + height));
                surface.context->SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_COPY);
                surface.context->FillRoundedRectangle(D2D1::RoundedRect(hole, r, r), clear.get());
                surface.context->SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);
            }
        }
        auto previous = bitmap;
        if (surface.End(bitmap) && bitmap != previous) image.Source(bitmap);
        image.Margin(xaml::Thickness{-pad, -pad, -pad, -pad});
    }
};

}
