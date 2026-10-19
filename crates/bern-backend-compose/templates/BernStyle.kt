// Bern Compose runtime: style overrides, motion and drawing. Generated
// build input; do not edit.
//
// Controls keep their Material look unless the application's `style!`
// says otherwise. The generated `BernStyles` describes each rule as a
// `BernStyle`: a look at rest, a look per state (hover, pressed, selected,
// disabled) and the springs that move between them. Colors are semantic
// roles mapped to the Material color scheme, or explicit pairs for light
// and dark. When the system turns animations off, transforms stay still.

package dev.bern.runtime

import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.AnimationVector
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.TwoWayConverter
import androidx.compose.animation.core.VectorizedFiniteAnimationSpec
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp

// MARK: - Motion

/// A spring from `style!`: undamped period in seconds and damping ratio.
data class BernMotion(val period: Double, val damping: Double) {
    fun <T> spec(): SpringSpec<T> {
        val stiffness = (2 * Math.PI / period).let { it * it }
        return spring(dampingRatio = damping.toFloat(), stiffness = stiffness.toFloat())
    }
}

/// Whether motion is reduced: the system turned animations off
/// (developer options or accessibility), or the application asks for it
/// (`reduce_motion`).
@Composable
fun bernReduceMotion(): Boolean {
    val context = LocalContext.current
    val system = remember {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    return system || BernMotionState.reduced
}

// MARK: - Colors

/// A semantic color role or an explicit pair for light and dark, as
/// `0xRRGGBB`.
sealed class BernColor {
    data class Role(val role: String) : BernColor()
    data class Fixed(val light: Long, val dark: Long) : BernColor()
}

/// Roles resolved against the Material color scheme in use.
class BernPalette(private val scheme: ColorScheme, private val dark: Boolean) {
    fun color(color: BernColor): Color = when (color) {
        is BernColor.Fixed -> Color(0xFF000000 or (if (dark) color.dark else color.light))
        is BernColor.Role -> role(color.role)
    }

    fun role(role: String): Color = when (role) {
        "accent", "focus.indicator" -> scheme.primary
        "on_accent" -> scheme.onPrimary
        "text.primary", "icon.primary" -> scheme.onSurface
        "text.secondary", "icon.secondary" -> scheme.onSurfaceVariant
        "text.placeholder" -> scheme.onSurfaceVariant.copy(alpha = 0.6f)
        "status.success" -> if (dark) Color(0xFF81C784) else Color(0xFF2E7D32)
        "status.warning" -> if (dark) Color(0xFFFFB74D) else Color(0xFFE65100)
        "status.error" -> scheme.error
        "fill.subtle", "disabled.surface" -> scheme.onSurface.copy(alpha = 0.06f)
        "fill.strong", "control.track" -> scheme.onSurface.copy(alpha = 0.12f)
        "control.thumb" -> Color.White
        "surface.canvas" -> scheme.background
        "surface.panel" -> scheme.surfaceContainer
        "surface.raised" -> scheme.surfaceContainerHigh
        "control.border" -> scheme.outlineVariant
        "selection.background" -> scheme.secondaryContainer
        "selection.foreground" -> scheme.onSecondaryContainer
        "disabled.foreground" -> scheme.onSurface.copy(alpha = 0.38f)
        else -> scheme.onSurface
    }

    /// Text and icons drawn on this color: the paired role, or black or
    /// white, whichever reads better.
    fun onTop(color: BernColor): Color = when {
        color == BernColor.Role("accent") -> scheme.onPrimary
        color == BernColor.Role("selection.background") -> scheme.onSecondaryContainer
        color is BernColor.Role && color.role.startsWith("status.") -> Color.White
        color is BernColor.Role -> scheme.onSurface
        else -> if (color(color).luminance() > 0.4f) Color(0xFF111114) else Color.White
    }
}

@Composable
fun bernPalette(): BernPalette = BernPalette(MaterialTheme.colorScheme, bernDark())

// MARK: - Styles

enum class BernEdge { Leading, Trailing, Top, Bottom }

enum class BernControlSize { Small, Regular, Large, ExtraLarge }

/// How a control looks in one state. Unset fields keep the look below it.
data class BernLook(
    val background: BernColor? = null,
    val foreground: BernColor? = null,
    val border: BernColor? = null,
    val borderWidth: Double = 1.0,
    /// A selected item filled with this color.
    val fill: BernColor? = null,
    /// A selected item marked by a bar on one edge.
    val indicator: BernColor? = null,
    val indicatorEdge: BernEdge = BernEdge.Leading,
    val indicatorWidth: Double = 3.0,
    val scaleX: Float? = null,
    val scaleY: Float? = null,
    val offsetX: Float? = null,
    val offsetY: Float? = null,
    val opacity: Float? = null,
    /// A shadow below, as if lifted: blur radius, offset down, opacity.
    val shadowRadius: Double? = null,
    val shadowY: Double? = null,
    val shadowOpacity: Double? = null,
    /// The spring entering this state.
    val motion: BernMotion? = null,
    /// `none`: this state shows nothing of its own.
    val nothing: Boolean = false,
) {
    /// `other` laid over this look.
    fun coveredBy(other: BernLook): BernLook {
        if (other.nothing) return this
        return copy(
            background = other.background ?: background,
            foreground = other.foreground ?: foreground,
            border = other.border ?: border,
            borderWidth = if (other.border != null) other.borderWidth else borderWidth,
            fill = other.fill ?: fill,
            indicator = other.indicator ?: indicator,
            indicatorEdge = if (other.indicator != null) other.indicatorEdge else indicatorEdge,
            indicatorWidth = if (other.indicator != null) other.indicatorWidth else indicatorWidth,
            scaleX = other.scaleX ?: scaleX,
            scaleY = other.scaleY ?: scaleY,
            offsetX = other.offsetX ?: offsetX,
            offsetY = other.offsetY ?: offsetY,
            opacity = other.opacity ?: opacity,
            shadowRadius = other.shadowRadius ?: shadowRadius,
            shadowY = if (other.shadowRadius != null) other.shadowY else shadowY,
            shadowOpacity = if (other.shadowRadius != null) other.shadowOpacity else shadowOpacity,
            motion = other.motion ?: motion,
        )
    }
}

/// A control's style overrides from `style!`.
data class BernStyle(
    val base: BernLook = BernLook(),
    val corner: Double? = null,
    val padding: Double? = null,
    val size: BernControlSize? = null,
    val hover: BernLook? = null,
    val pressed: BernLook? = null,
    val selected: BernLook? = null,
    val disabled: BernLook? = null,
    /// The spring back to rest.
    val settle: BernMotion = BernMotion(0.45, 1.0),
) {
    /// Whether the style draws the control itself instead of keeping the
    /// Material look, which only moves.
    val drawsLook: Boolean
        get() = corner != null || padding != null ||
            listOfNotNull(base, hover, pressed, disabled).any {
                it.background != null || it.foreground != null || it.border != null
            }

    /// The look for a combination of states and the spring reaching it:
    /// disabled covers pressed, which covers hover; selection lies below.
    fun look(selected: Boolean, hover: Boolean, pressed: Boolean, enabled: Boolean): Pair<BernLook, BernMotion> {
        var look = base
        var motion: BernMotion? = null
        for ((active, state) in listOf(
            selected to this.selected,
            (hover && enabled) to this.hover,
            (pressed && enabled) to this.pressed,
            !enabled to this.disabled,
        )) {
            if (active && state != null) {
                look = look.coveredBy(state)
                motion = state.motion ?: motion
            }
        }
        return look to (motion ?: settle)
    }

    /// Padding of a drawn control at its size.
    val insets: PaddingValues
        get() {
            padding?.let { return PaddingValues(horizontal = it.dp, vertical = (it / 2).dp) }
            return when (size) {
                BernControlSize.Small -> PaddingValues(horizontal = 10.dp, vertical = 4.dp)
                BernControlSize.Large -> PaddingValues(horizontal = 22.dp, vertical = 12.dp)
                BernControlSize.ExtraLarge -> PaddingValues(horizontal = 28.dp, vertical = 16.dp)
                else -> PaddingValues(horizontal = 16.dp, vertical = 8.dp)
            }
        }
}

/// The look's scale, offset and opacity, moved by the state's spring;
/// still when the system turned animations off.
@Composable
fun Modifier.bernTransform(look: BernLook, motion: BernMotion): Modifier {
    if (bernReduceMotion()) return this
    val spec = motion.spec<Float>()
    val scaleX by animateFloatAsState(look.scaleX ?: 1f, spec, label = "scaleX")
    val scaleY by animateFloatAsState(look.scaleY ?: 1f, spec, label = "scaleY")
    val offsetX by animateFloatAsState(look.offsetX ?: 0f, spec, label = "offsetX")
    val offsetY by animateFloatAsState(look.offsetY ?: 0f, spec, label = "offsetY")
    val alpha by animateFloatAsState(look.opacity ?: 1f, tween(150), label = "alpha")
    return graphicsLayer {
        this.scaleX = scaleX
        this.scaleY = scaleY
        translationX = offsetX * density
        translationY = offsetY * density
        this.alpha = alpha
    }
}

/// A button the style draws itself, or a Material button whose press and
/// hover follow the style's transforms.
@Composable
fun BernStyledButton(
    style: BernStyle,
    enabled: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
    material: @Composable (Modifier, MutableInteractionSource?) -> Unit,
    content: @Composable RowScope.() -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val hovered by interaction.collectIsHoveredAsState()
    val (look, motion) = style.look(false, hovered, pressed, enabled)
    val shape = RoundedCornerShape((style.corner ?: 6.0).dp)
    if (!style.drawsLook) {
        material(modifier.bernTransform(look, motion).bernShadow(look, motion, shape), interaction)
        return
    }
    val palette = bernPalette()
    val foreground = look.foreground?.let(palette::color)
        ?: look.background?.let(palette::onTop)
        ?: MaterialTheme.colorScheme.onSurface
    Row(
        modifier
            .bernTransform(look, motion)
            .bernShadow(look, motion, shape)
            .clip(shape)
            .background(look.background?.let(palette::color) ?: Color.Transparent)
            .border(look.borderWidth.dp, look.border?.let(palette::color) ?: Color.Transparent, shape)
            .clickable(interaction, ripple(), enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(style.insets),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CompositionLocalProvider(LocalContentColor provides foreground) { content() }
    }
}

/// A sidebar destination the style draws: hover, press and selection
/// follow the style instead of the Material highlight.
@Composable
fun BernStyledSidebarItem(
    label: String,
    icon: ImageVector?,
    selected: Boolean,
    style: BernStyle,
    badge: Long? = null,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val hovered by interaction.collectIsHoveredAsState()
    val (look, motion) = style.look(selected, hovered, pressed, true)
    val palette = bernPalette()
    val fill = if (selected) look.fill else null
    val shape = RoundedCornerShape((style.corner ?: 28.0).dp)
    val foreground = look.foreground?.let(palette::color)
        ?: fill?.let(palette::onTop)
        ?: look.background?.let(palette::onTop)
        ?: MaterialTheme.colorScheme.onSurfaceVariant
    val indicator = if (selected) look.indicator?.let(palette::color) else null
    val padding = (style.padding ?: 16.0).dp
    Row(
        modifier
            .fillMaxWidth()
            .bernTransform(look, motion)
            .clip(shape)
            .background((fill ?: look.background)?.let(palette::color) ?: Color.Transparent)
            .drawBehind {
                if (indicator == null) return@drawBehind
                val width = look.indicatorWidth.dp.toPx()
                val inset = 6.dp.toPx()
                val (topLeft, size) = when (look.indicatorEdge) {
                    BernEdge.Leading -> Offset(0f, inset) to Size(width, size.height - 2 * inset)
                    BernEdge.Trailing -> Offset(size.width - width, inset) to Size(width, size.height - 2 * inset)
                    BernEdge.Top -> Offset(inset, 0f) to Size(size.width - 2 * inset, width)
                    BernEdge.Bottom -> Offset(inset, size.height - width) to Size(size.width - 2 * inset, width)
                }
                drawRoundRect(indicator, topLeft, size, CornerRadius(width / 2, width / 2))
            }
            .clickable(interaction, ripple(), role = Role.Tab, onClick = onClick)
            .padding(horizontal = padding, vertical = padding * 0.75f),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = foreground)
            Spacer(Modifier.width(12.dp))
        }
        Text(label, color = foreground, style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f))
        // A live count beside the page, hidden at zero.
        if (badge != null && badge > 0) {
            Text("$badge", color = foreground, style = MaterialTheme.typography.labelLarge)
        }
    }
}

// MARK: - Drawing

/// A band of a pattern: `width` dp of `color`, or a gap when `color` is
/// null.
data class BernBand(val color: BernColor?, val width: Double, val opacity: Double)

/// A woven check: the bands repeat across and down, and their colors add
/// up where they cross; `twill` lays fine diagonal lines over it.
data class BernPatternDef(val bands: List<BernBand>, val twill: Boolean)

/// What `Canvas` children draw, in canvas dp.
object BernDraw {
    fun shape(
        scope: DrawScope, palette: BernPalette, kind: String,
        x: Double, y: Double, width: Double?, height: Double?, corner: Double,
        fill: BernColor?, stroke: BernColor?, strokeWidth: Double, opacity: Double,
        fade: String? = null, dash: Double? = null, dashGap: Double? = null,
    ) = with(scope) {
        val left = x.dp.toPx()
        val top = y.dp.toPx()
        val rect = Rect(
            left, top,
            width?.let { left + it.dp.toPx() } ?: size.width,
            height?.let { top + it.dp.toPx() } ?: size.height,
        )
        val radius = when (kind) {
            "rounded" -> corner.dp.toPx()
            "capsule" -> minOf(rect.width, rect.height) / 2
            else -> 0f
        }
        val alpha = opacity.toFloat()
        val paint = if (fill != null || stroke == null) palette.color(fill ?: BernColor.Role("fill.strong")) else null
        if (paint != null && fade != null) {
            // The fill fades out toward the named edge.
            val (start, end) = when (fade) {
                "up" -> Offset(rect.center.x, rect.bottom) to Offset(rect.center.x, rect.top)
                "left" -> Offset(rect.right, rect.center.y) to Offset(rect.left, rect.center.y)
                "right" -> Offset(rect.left, rect.center.y) to Offset(rect.right, rect.center.y)
                else -> Offset(rect.center.x, rect.top) to Offset(rect.center.x, rect.bottom)
            }
            val brush = androidx.compose.ui.graphics.Brush.linearGradient(listOf(paint, paint.copy(alpha = 0f)), start, end)
            if (kind == "ellipse") {
                drawOval(brush, rect.topLeft, rect.size, alpha)
            } else {
                drawRoundRect(brush, rect.topLeft, rect.size, CornerRadius(radius, radius), alpha = alpha)
            }
        } else if (paint != null) {
            if (kind == "ellipse") {
                drawOval(paint, rect.topLeft, rect.size, alpha)
            } else {
                drawRoundRect(paint, rect.topLeft, rect.size, CornerRadius(radius, radius), alpha = alpha)
            }
        }
        if (stroke != null) {
            // Dashes `dash` long with gaps of `dashGap`.
            val dashes = dash?.let {
                androidx.compose.ui.graphics.PathEffect.dashPathEffect(
                    floatArrayOf(it.dp.toPx(), (dashGap ?: it).dp.toPx()))
            }
            val line = Stroke(strokeWidth.dp.toPx(), pathEffect = dashes)
            if (kind == "ellipse") {
                drawOval(palette.color(stroke), rect.topLeft, rect.size, alpha, line)
            } else {
                drawRoundRect(palette.color(stroke), rect.topLeft, rect.size, CornerRadius(radius, radius), line, alpha)
            }
        }
    }

    /// Values across the full width, scaled so `max` (or the largest value)
    /// reaches the top.
    fun series(
        scope: DrawScope, palette: BernPalette, values: List<Double>, max: Double?, area: Boolean,
        fill: BernColor?, stroke: BernColor?, strokeWidth: Double, opacity: Double,
    ) = with(scope) {
        if (values.isEmpty()) return@with
        val peak = max ?: values.maxOrNull() ?: 1.0
        val scale = if (peak > 0) peak else 1.0
        val step = if (values.size > 1) size.width / (values.size - 1) else 0f
        val points = values.mapIndexed { index, value ->
            Offset(
                if (values.size > 1) index * step else size.width / 2,
                size.height - size.height * (value / scale).coerceIn(0.0, 1.0).toFloat(),
            )
        }
        val line = Path().apply {
            moveTo(points[0].x, points[0].y)
            for (point in points.drop(1)) lineTo(point.x, point.y)
        }
        val alpha = opacity.toFloat()
        val style = Stroke(strokeWidth.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
        if (area) {
            val shape = Path().apply {
                addPath(line)
                lineTo(points.last().x, size.height)
                lineTo(points.first().x, size.height)
                close()
            }
            drawPath(shape, palette.color(fill ?: BernColor.Role("accent")), alpha)
            if (stroke != null) drawPath(line, palette.color(stroke), alpha, style)
        } else {
            drawPath(line, palette.color(stroke ?: BernColor.Role("accent")), alpha, style)
        }
    }

    fun pattern(scope: DrawScope, palette: BernPalette, pattern: BernPatternDef, opacity: Double) = with(scope) {
        val sett = pattern.bands.sumOf { maxOf(it.width, 0.0) }
        if (sett <= 0) return@with
        val alpha = opacity.toFloat()
        clipRect {
            for (across in listOf(true, false)) {
                val extent = if (across) size.width else size.height
                var offset = 0f
                while (offset < extent) {
                    for (band in pattern.bands) {
                        if (band.width <= 0) continue
                        val width = band.width.dp.toPx()
                        band.color?.let { color ->
                            val paint = palette.color(color).copy(alpha = band.opacity.toFloat())
                            if (across) {
                                drawRect(paint, Offset(offset, 0f), Size(width, size.height), alpha)
                            } else {
                                drawRect(paint, Offset(0f, offset), Size(size.width, width), alpha)
                            }
                        }
                        offset += width
                        if (offset >= extent) break
                    }
                }
            }
            if (pattern.twill) {
                val spacing = 3.dp.toPx()
                val line = palette.role("text.primary").copy(alpha = 0.05f)
                var start = -size.height
                while (start < size.width) {
                    drawLine(line, Offset(start, size.height), Offset(start + size.height, 0f), 0.5.dp.toPx(), alpha = alpha)
                    start += spacing
                }
            }
        }
    }
}

// MARK: - Text marks

/// `Text(mark:)`: a capsule of the color under each line, from 74% to 98%
/// of the line's height and 0.4 of a character past its end, behind the
/// glyphs and outside the layout.
@Composable
fun BernMarkedText(text: String, style: TextStyle, color: Color, mark: BernColor, lines: Int? = null) {
    val palette = bernPalette()
    val band = palette.color(mark)
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    Text(
        text,
        style = style,
        color = color,
        maxLines = lines ?: Int.MAX_VALUE,
        overflow = if (lines != null) androidx.compose.ui.text.style.TextOverflow.Ellipsis else androidx.compose.ui.text.style.TextOverflow.Clip,
        onTextLayout = { layout = it },
        modifier = Modifier.drawBehind {
            val result = layout ?: return@drawBehind
            val em = style.fontSize.toPx()
            for (line in 0 until result.lineCount) {
                val top = result.getLineTop(line)
                val height = result.getLineBottom(line) - top
                val left = result.getLineLeft(line)
                val right = result.getLineRight(line)
                val bandHeight = height * 0.24f
                drawRoundRect(
                    band,
                    Offset(left, top + height * 0.74f),
                    Size(right - left + em * 0.4f, bandHeight),
                    CornerRadius(bandHeight / 2, bandHeight / 2),
                )
            }
        },
    )
}

/// A canvas's frame: its declared sizes, filling the space otherwise.
fun Modifier.bernFrame(width: Dp?, height: Dp?): Modifier {
    val across = if (width != null) width(width) else fillMaxWidth()
    return if (height != null) across.height(height) else across.fillMaxHeight()
}

// MARK: - Transitions

/// How an element appears or disappears: a fade, growing from a fraction
/// of its size, or sliding in from an edge.
sealed class BernTransition {
    data object Fade : BernTransition()
    data class Pop(val from: Float) : BernTransition()
    data class Slide(val edge: BernEdge, val distance: Float) : BernTransition()
    /// Grows along one axis from nothing, `edge` holding still, without
    /// fading.
    data class Grow(val edge: BernEdge) : BernTransition()
}

/// Where a growing element holds still.
fun BernEdge.origin(): androidx.compose.ui.graphics.TransformOrigin = when (this) {
    BernEdge.Leading -> androidx.compose.ui.graphics.TransformOrigin(0f, 0.5f)
    BernEdge.Trailing -> androidx.compose.ui.graphics.TransformOrigin(1f, 0.5f)
    BernEdge.Top -> androidx.compose.ui.graphics.TransformOrigin(0.5f, 0f)
    BernEdge.Bottom -> androidx.compose.ui.graphics.TransformOrigin(0.5f, 1f)
}

/// The transition into view, moved by its spring; a short fade when the
/// system turned animations off. After `delay` seconds: until then the
/// element keeps its starting look, hidden, in its place.
@Composable
fun bernEnter(transition: BernTransition?, motion: BernMotion?, delay: Double = 0.0): EnterTransition {
    if (transition == null) return EnterTransition.None
    if (bernReduceMotion() || motion == null) return fadeIn(tween(150))
    val density = LocalDensity.current
    fun <T> spec(): FiniteAnimationSpec<T> = motion.spec<T>().let { if (delay > 0) BernDelayed(it, delay) else it }
    return when (transition) {
        is BernTransition.Fade -> fadeIn(spec())
        is BernTransition.Pop -> scaleIn(spec(), initialScale = transition.from) + fadeIn(spec())
        is BernTransition.Slide -> {
            val distance = with(density) { transition.distance.dp.roundToPx() }
            slideIn(spec()) { slideOffset(transition.edge, distance) } + fadeIn(spec())
        }
        // Where a branch changes, the room grows from the edge; an element
        // that appears on its own scales instead (`BernAppear`).
        is BernTransition.Grow -> when (transition.edge) {
            BernEdge.Leading -> expandHorizontally(spec(), Alignment.Start)
            BernEdge.Trailing -> expandHorizontally(spec(), Alignment.End)
            BernEdge.Top -> expandVertically(spec(), Alignment.Top)
            BernEdge.Bottom -> expandVertically(spec(), Alignment.Bottom)
        }
    }
}

/// `inner` after `seconds`: the value stays at its start until then.
/// Compose's springs take no delay of their own.
private class BernDelayed<T>(val inner: FiniteAnimationSpec<T>, val seconds: Double) : FiniteAnimationSpec<T> {
    override fun <V : AnimationVector> vectorize(converter: TwoWayConverter<T, V>): VectorizedFiniteAnimationSpec<V> =
        Vectorized(inner.vectorize(converter), (seconds * 1_000_000_000).toLong())

    private class Vectorized<V : AnimationVector>(
        val inner: VectorizedFiniteAnimationSpec<V>,
        val delay: Long,
    ) : VectorizedFiniteAnimationSpec<V> {
        override fun getValueFromNanos(playTimeNanos: Long, initialValue: V, targetValue: V, initialVelocity: V): V =
            if (playTimeNanos < delay) initialValue
            else inner.getValueFromNanos(playTimeNanos - delay, initialValue, targetValue, initialVelocity)

        override fun getVelocityFromNanos(playTimeNanos: Long, initialValue: V, targetValue: V, initialVelocity: V): V =
            if (playTimeNanos < delay) initialVelocity
            else inner.getVelocityFromNanos(playTimeNanos - delay, initialValue, targetValue, initialVelocity)

        override fun getDurationNanos(initialValue: V, targetValue: V, initialVelocity: V): Long =
            delay + inner.getDurationNanos(initialValue, targetValue, initialVelocity)
    }
}

/// The transition out of view, the reverse of [bernEnter].
@Composable
fun bernExit(transition: BernTransition?, motion: BernMotion?): ExitTransition {
    if (transition == null) return ExitTransition.None
    if (bernReduceMotion() || motion == null) return fadeOut(tween(150))
    val density = LocalDensity.current
    return when (transition) {
        is BernTransition.Fade -> fadeOut(motion.spec())
        is BernTransition.Pop -> scaleOut(motion.spec(), targetScale = transition.from) + fadeOut(motion.spec())
        is BernTransition.Slide -> {
            val distance = with(density) { transition.distance.dp.roundToPx() }
            slideOut(motion.spec()) { slideOffset(transition.edge, distance) } + fadeOut(motion.spec())
        }
        is BernTransition.Grow -> when (transition.edge) {
            BernEdge.Leading -> shrinkHorizontally(motion.spec(), Alignment.Start)
            BernEdge.Trailing -> shrinkHorizontally(motion.spec(), Alignment.End)
            BernEdge.Top -> shrinkVertically(motion.spec(), Alignment.Top)
            BernEdge.Bottom -> shrinkVertically(motion.spec(), Alignment.Bottom)
        }
    }
}

private fun slideOffset(edge: BernEdge, distance: Int): IntOffset = when (edge) {
    BernEdge.Leading -> IntOffset(-distance, 0)
    BernEdge.Trailing -> IntOffset(distance, 0)
    BernEdge.Top -> IntOffset(0, -distance)
    BernEdge.Bottom -> IntOffset(0, distance)
}

/// Whether the window showed its first frame: elements that appear later
/// play their transition, those of the first display do not.
val LocalBernSettled = staticCompositionLocalOf { mutableStateOf(true) }

/// An element that plays its appearing transition when it is added after
/// the window's first display, as an item added to a list. A `staggered`
/// one is a direct child of a container that declares `stagger`: it waits
/// for its turn on the container's clock, unless the system turned
/// animations off. With a `delay` (`appear_delay`) it waits that much more,
/// and plays on the window's first display too.
@Composable
fun BernAppear(
    transition: BernTransition?,
    motion: BernMotion?,
    staggered: Boolean = false,
    delay: Double? = null,
    content: @Composable () -> Unit,
) {
    val settled = LocalBernSettled.current.value
    val clock = if (staggered) LocalBernStagger.current else null
    val reduced = bernReduceMotion()
    val plays = remember { settled || delay != null }
    val wait = remember { if (plays && !reduced) (if (settled) clock?.turn() ?: 0.0 else 0.0) + (delay ?: 0.0) else 0.0 }
    val inner: @Composable () -> Unit = {
        // The clock is only for the container's direct children.
        if (clock != null) BernUnstaggered(content) else content()
    }
    // Growing scales along one axis from the edge that holds still.
    if (transition is BernTransition.Grow && motion != null && !reduced) {
        val progress = remember { androidx.compose.animation.core.Animatable(if (plays) 0f else 1f) }
        LaunchedEffect(Unit) {
            if (!plays) return@LaunchedEffect
            if (wait > 0) kotlinx.coroutines.delay((wait * 1000).toLong())
            progress.animateTo(1f, motion.spec())
        }
        val horizontal = transition.edge == BernEdge.Leading || transition.edge == BernEdge.Trailing
        Box(
            Modifier.graphicsLayer {
                transformOrigin = transition.edge.origin()
                if (horizontal) scaleX = progress.value.coerceAtLeast(0f) else scaleY = progress.value.coerceAtLeast(0f)
            },
        ) { inner() }
        return
    }
    val state = remember { MutableTransitionState(!plays).apply { targetState = true } }
    AnimatedVisibility(visibleState = state, enter = bernEnter(transition, motion, wait)) { inner() }
}

// MARK: - Stagger

/// The clock of a container that declares `stagger`: its direct children
/// that appear take turns, `step` seconds apart. The turns of one update
/// are those composed before the main thread moves on; the count then
/// starts over, so children added by a later update start at once.
class BernStagger(private val step: Double) {
    private var next = 0
    private var resetting = false

    /// The delay of the next child to appear, in seconds.
    fun turn(): Double {
        val delay = next * step
        next += 1
        if (!resetting) {
            resetting = true
            main.post {
                next = 0
                resetting = false
            }
        }
        return delay
    }

    private companion object {
        val main = android.os.Handler(android.os.Looper.getMainLooper())
    }
}

/// The clock of the staggering container whose direct children are being
/// composed, if any.
val LocalBernStagger = staticCompositionLocalOf<BernStagger?> { null }

@Composable
fun rememberBernStagger(step: Double): BernStagger = remember { BernStagger(step) }

/// The direct children of a container that declares `stagger`.
@Composable
fun BernStaggered(clock: BernStagger, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalBernStagger provides clock, content = content)
}

/// Content below a staggering container's direct children, which no
/// longer takes turns.
@Composable
fun BernUnstaggered(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalBernStagger provides null, content = content)
}

/// A container whose size follows its children with a spring.
@Composable
fun Modifier.bernReflow(motion: BernMotion): Modifier =
    if (bernReduceMotion()) this else animateContentSize(motion.spec())

// MARK: - Shared elements

/// The shared transition of the window the elements pair up in.
@OptIn(ExperimentalSharedTransitionApi::class)
val LocalBernShared = staticCompositionLocalOf<SharedTransitionScope?> { null }

/// The animated branch an element appears or disappears with.
val LocalBernVisibility = staticCompositionLocalOf<AnimatedVisibilityScope?> { null }

/// The window's root of shared elements.
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun BernHeroRoot(content: @Composable () -> Unit) {
    SharedTransitionLayout {
        CompositionLocalProvider(LocalBernShared provides this) { content() }
    }
}

/// One of the elements sharing `key` in a window: when one replaces
/// another, it grows from where the other was, moved by `motion`. Each is
/// laid out at its own size and scaled to the moving bounds, so text does
/// not wrap on the way, and the two cross-fade.
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun BernHero(key: String, motion: BernMotion, content: @Composable () -> Unit) {
    val shared = LocalBernShared.current
    val visibility = LocalBernVisibility.current
    if (shared == null || visibility == null || bernReduceMotion()) {
        content()
        return
    }
    with(shared) {
        Box(
            Modifier.sharedBounds(
                rememberSharedContentState(key),
                visibility,
                enter = fadeIn(motion.spec()),
                exit = fadeOut(motion.spec()),
                boundsTransform = { _, _ -> motion.spec() },
                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(),
            ),
        ) { content() }
    }
}
