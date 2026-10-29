// Bern Compose runtime: page-level layout, fixed scenes, modals, forms,
// the search palette and the shapes of system controls. Generated build
// input; do not edit.
//
// Scenes keep one coordinate space and scale it as a whole, so pictures,
// text and touch targets stay registered. Modals are Material dialogs over
// the window; the search palette is one too. Controls keep their Material
// look; `shape: capsule` and `height` only size them.

package dev.bern.runtime

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.FlowRowScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialogDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.HorizontalAlignmentLine
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.constrainHeight
import androidx.compose.ui.unit.constrainWidth
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.offset
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// MARK: - Text styles

/// The weights and sizes `text { … }` in `style!` gives text styles, by
/// style name: a weight and a size in points, or null for Material's.
object BernTypography {
    var styles: Map<String, Pair<String, Double?>> = emptyMap()
}

private fun bernWeight(name: String): FontWeight = when (name) {
    "ultralight" -> FontWeight.W100
    "thin" -> FontWeight.W200
    "light" -> FontWeight.W300
    "medium" -> FontWeight.W500
    "semibold" -> FontWeight.W600
    "bold" -> FontWeight.W700
    "heavy" -> FontWeight.W800
    "black" -> FontWeight.W900
    else -> FontWeight.W400
}

/// The text shadow of what a `Marquee` carries, which runs over pictures.
val LocalBernTextShadow = staticCompositionLocalOf<Shadow?> { null }

/// The column or row a component is called in (its `ColumnScope` or
/// `RowScope`), so what the component lays out can take the free room
/// there, as it would written in place.
val LocalBernFlex = staticCompositionLocalOf<Any?> { null }

/// Takes the free room along the column (`column`) or row (`row`) it is
/// laid out in, as a scroll view does on the other targets.
@Composable
fun Modifier.bernFlexible(column: Boolean = true, row: Boolean = true): Modifier =
    when (val scope = LocalBernFlex.current) {
        is ColumnScope -> if (column) with(scope) { this@bernFlexible.weight(1f) } else this
        is RowScope -> if (row) with(scope) { this@bernFlexible.weight(1f) } else this
        else -> this
    }

/// Material's type style for a Bern text style, with the weight and size
/// `style!` gives it, and a text's own `font_size` and `line_height` in
/// points. A size of its own takes the font's natural line height; a line
/// height splits its extra room above and below the line, as CSS does.
@Composable
fun bernTextStyle(style: String, size: Float? = null, lineHeight: Float? = null): TextStyle {
    val typography = MaterialTheme.typography
    var text = when (style) {
        "large_title" -> typography.headlineLarge
        "title" -> typography.headlineSmall
        "title2" -> typography.titleLarge
        "title3" -> typography.titleMedium
        "headline" -> typography.titleSmall
        "subheadline" -> typography.bodyMedium
        "callout" -> typography.bodyMedium
        "footnote" -> typography.bodySmall
        "caption" -> typography.labelMedium
        "caption2" -> typography.labelSmall
        // `control`: a Material button's title.
        "control" -> typography.labelLarge
        "code" -> typography.bodyMedium.copy(fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace)
        else -> typography.bodyLarge
    }
    BernTypography.styles[style]?.let { (weight, points) ->
        text = text.copy(fontWeight = bernWeight(weight))
        if (points != null) text = text.copy(fontSize = points.sp, lineHeight = TextUnit.Unspecified, letterSpacing = 0.sp)
    }
    if (size != null) text = text.copy(fontSize = size.sp, lineHeight = TextUnit.Unspecified)
    if (lineHeight != null) {
        text = text.copy(
            lineHeight = lineHeight.sp,
            lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None),
        )
    }
    LocalBernTextShadow.current?.let { text = text.copy(shadow = it) }
    return text
}

/// A `Text`: at most `lines` lines, the last ending with an ellipsis.
@Composable
fun BernText(text: String, style: TextStyle, color: Color, lines: Int? = null) {
    Text(
        text,
        style = style,
        color = color,
        maxLines = lines ?: Int.MAX_VALUE,
        overflow = if (lines != null) TextOverflow.Ellipsis else TextOverflow.Clip,
    )
}

// MARK: - Control shapes

/// `shape: capsule` and `height` of a system control: Material's buttons
/// are capsules already; a height sets the control's own.
class BernShape(val capsule: Boolean, val height: Dp?)

/// The size of an icon set at `points`: Material's icons leave a sixth of
/// their box free, as a symbol's line does.
fun bernIconSize(points: Float): Dp = (points * 1.2f).dp

/// A picker option's name for external entries: an enum's variant name,
/// else the value as text.
fun bernOptionName(value: Any?): String = when (value) {
    null -> ""
    is Enum<*> -> value.name
    else -> value.toString()
}

// MARK: - Reduced motion

/// `reduce_motion`: while an element holding it is shown and it is true,
/// the whole application runs with reduced motion, as when the system
/// turns animations off. Each is one state: every animated element reads
/// them as it is composed.
object BernMotionState {
    private val holders = HashMap<Any, Boolean>()
    var reduced by mutableStateOf(false)
        private set
    /// The system turned animations off, read when the app starts and
    /// each time it comes back to the front.
    var system by mutableStateOf(false)
        private set

    fun set(token: Any, on: Boolean) {
        if (holders.put(token, on) != on) reduced = holders.values.any { it }
    }

    fun remove(token: Any) {
        if (holders.remove(token) != null) reduced = holders.values.any { it }
    }

    fun readSystem(context: android.content.Context) {
        val off = android.provider.Settings.Global.getFloat(
            context.contentResolver, android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 1f,
        ) == 0f
        if (off != system) system = off
    }
}

@Composable
fun BernReduceMotion(on: Boolean) {
    val token = remember { Any() }
    SideEffect { BernMotionState.set(token, on) }
    DisposableEffect(token) { onDispose { BernMotionState.remove(token) } }
}

// MARK: - Stacks

/// A `Stack` with a style's surface — its background, the color of text
/// and icons in it, and corners that clip its layers — or one that takes
/// no input (`enabled: false`): a cover above its layers takes every
/// touch, and the layers stay as they are.
@Composable
fun BernStack(
    modifier: Modifier,
    alignment: Alignment,
    style: BernStyle? = null,
    enabled: Boolean = true,
    content: @Composable BoxScope.() -> Unit,
) {
    val palette = bernPalette()
    var surface = modifier
    val shape = style?.corner?.let { RoundedCornerShape(it.dp) }
    if (shape != null) surface = surface.clip(shape)
    style?.base?.background?.let { surface = surface.background(palette.color(it)) }
    val foreground = style?.base?.foreground?.let(palette::color)
    Box(surface, contentAlignment = alignment) {
        if (foreground != null) {
            CompositionLocalProvider(LocalContentColor provides foreground) { content() }
        } else {
            content()
        }
        if (!enabled) {
            Box(
                Modifier.matchParentSize().pointerInput(Unit) {
                    awaitPointerEventScope {
                        while (true) {
                            awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() }
                        }
                    }
                },
            )
        }
    }
}

// MARK: - Scenes

/// `ScaleBox`: its children stacked from the top leading corner in a
/// space of `width` × `height`, scaled as a whole to fit the room it is
/// given and centered there; what stands outside the box is clipped. The
/// scene's layers learn the room around the scene, which a layer that
/// bleeds reaches into.
@Composable
fun BernScaleBox(width: Dp, height: Dp, content: @Composable () -> Unit) {
    SubcomposeLayout(Modifier.clipToBounds()) { constraints ->
        val w = width.roundToPx().coerceAtLeast(1)
        val h = height.roundToPx().coerceAtLeast(1)
        val scale = when {
            constraints.hasBoundedWidth && constraints.hasBoundedHeight ->
                min(constraints.maxWidth.toFloat() / w, constraints.maxHeight.toFloat() / h)
            constraints.hasBoundedWidth -> constraints.maxWidth.toFloat() / w
            constraints.hasBoundedHeight -> constraints.maxHeight.toFloat() / h
            else -> 1f
        }
        val outWidth = if (constraints.hasBoundedWidth) constraints.maxWidth else (w * scale).roundToInt()
        val outHeight = if (constraints.hasBoundedHeight) constraints.maxHeight else (h * scale).roundToInt()
        val fits = scale > 0f && scale.isFinite()
        val room = BernSceneRoom(
            width = width,
            height = height,
            x = if (fits) ((outWidth / scale - w) / 2f).coerceAtLeast(0f).toDp() else 0.dp,
            y = if (fits) ((outHeight / scale - h) / 2f).coerceAtLeast(0f).toDp() else 0.dp,
        )
        val placeable = subcompose(Unit) {
            CompositionLocalProvider(LocalBernSceneRoom provides room) {
                Box(contentAlignment = Alignment.TopStart) { content() }
            }
        }.first().measure(Constraints.fixed(w, h))
        layout(outWidth, outHeight) {
            placeable.placeWithLayer(0, 0) {
                transformOrigin = TransformOrigin(0f, 0f)
                scaleX = scale
                scaleY = scale
                translationX = (outWidth - w * scale) / 2f
                translationY = (outHeight - h * scale) / 2f
            }
        }
    }
}

/// A `ScaleBox` scene's size and the room the box has around it on each
/// side; none inside a layer, which places its own layers in its space.
data class BernSceneRoom(val width: Dp, val height: Dp, val x: Dp, val y: Dp)

val LocalBernSceneRoom = compositionLocalOf<BernSceneRoom?> { null }

/// Where a layer that bleeds is going (its place before any motion) and
/// whether it bleeds there, with the motion its sides follow.
data class BernBleed(val on: Boolean, val x: Dp, val y: Dp, val width: Dp, val height: Dp, val motion: BernMotion?)

/// How far a side of a layer reaches past its scene, moving on the layer's
/// motion.
@Composable
private fun bernBleedSide(target: Dp, motion: BernMotion?): Dp {
    if (motion == null || bernReduceMotion()) return target
    val moving by animateDpAsState(target, motion.spec(), label = "bleed")
    return moving.coerceAtLeast(0.dp)
}

/// `Positioned`: its content in the rectangle `x`, `y`, `width`,
/// `height` of the space it stands in, centered there. It takes all the
/// room its stack offers, as a layer does. With `bleed`, each side that
/// reaches its scene's edge continues to the edge of the room around the
/// scene.
@Composable
fun BernPositioned(x: Dp, y: Dp, width: Dp, height: Dp, bleed: BernBleed? = null, content: @Composable () -> Unit) {
    val room = LocalBernSceneRoom.current?.takeIf { bleed?.on == true }
    val half = 0.5.dp
    val left = bernBleedSide(if (room != null && bleed!!.x <= half) room.x else 0.dp, bleed?.motion)
    val right = bernBleedSide(if (room != null && bleed!!.x + bleed.width >= room.width - half) room.x else 0.dp, bleed?.motion)
    val top = bernBleedSide(if (room != null && bleed!!.y <= half) room.y else 0.dp, bleed?.motion)
    val bottom = bernBleedSide(if (room != null && bleed!!.y + bleed.height >= room.height - half) room.y else 0.dp, bleed?.motion)
    Layout(content = {
        Box(contentAlignment = Alignment.Center) {
            // The layer's own layers are placed in its space.
            CompositionLocalProvider(LocalBernSceneRoom provides null) { content() }
        }
    }) { measurables, constraints ->
        val w = (width + left + right).roundToPx().coerceAtLeast(0)
        val h = (height + top + bottom).roundToPx().coerceAtLeast(0)
        val atX = (x - left).roundToPx()
        val atY = (y - top).roundToPx()
        val placeable = measurables.first().measure(Constraints.fixed(w, h))
        val outWidth = if (constraints.hasBoundedWidth) constraints.maxWidth else max(0, atX + w)
        val outHeight = if (constraints.hasBoundedHeight) constraints.maxHeight else max(0, atY + h)
        layout(outWidth, outHeight) { placeable.place(atX, atY) }
    }
}

/// `Marquee`: its content at its natural width runs from beyond the right
/// edge to beyond the left one every `duration` seconds, starting `phase`
/// of the way; `paused` holds it where it is. With reduced motion it
/// stands still at 28% of the width. Text in it carries a dark shadow,
/// since it runs over pictures.
@Composable
fun BernMarquee(duration: Double, phase: Double, paused: Boolean, opacity: Float, content: @Composable () -> Unit) {
    val reduce = bernReduceMotion()
    var elapsed by remember { mutableDoubleStateOf(0.0) }
    LaunchedEffect(paused, reduce) {
        if (paused || reduce) return@LaunchedEffect
        val base = elapsed
        val start = withFrameNanos { it }
        while (true) {
            withFrameNanos { now -> elapsed = base + (now - start) / 1e9 }
        }
    }
    val density = LocalDensity.current
    val shadow = Shadow(Color.Black, Offset(0f, with(density) { 1.dp.toPx() }), with(density) { 1.dp.toPx() })
    Layout(
        content = { CompositionLocalProvider(LocalBernTextShadow provides shadow) { Box { content() } } },
        modifier = Modifier.clipToBounds().graphicsLayer { alpha = opacity.coerceIn(0f, 1f) },
    ) { measurables, constraints ->
        val placeable = measurables.first().measure(Constraints(maxHeight = constraints.maxHeight))
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth else placeable.width
        val height = placeable.height.coerceIn(constraints.minHeight, constraints.maxHeight)
        layout(width, height) {
            placeable.placeWithLayer(0, 0) {
                translationX = if (reduce) {
                    width * 0.28f
                } else {
                    val progress = ((phase + elapsed / max(0.001, duration)) % 1.0).toFloat()
                    width - (width + placeable.width) * progress
                }
            }
        }
    }
}

/// `Accordion`: columns side by side, one open into the width the others
/// leave; each closed column is `collapsedWidth` wide. A weight from 0 to
/// 1 per column follows the selection with `motion`, so an interrupted
/// change keeps every column's width; the widths always fill the row.
/// Each column is clipped to `corner`.
@Composable
fun BernAccordion(
    count: Int,
    selected: Int,
    collapsedWidth: Dp,
    gap: Dp,
    height: Dp?,
    corner: Dp,
    motion: BernMotion?,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val columns = count.coerceAtLeast(0)
    val selection = selected.coerceIn(0, max(0, columns - 1))
    val reduce = bernReduceMotion()
    val weights = remember(columns) { List(columns) { Animatable(if (it == selection) 1f else 0f) } }
    LaunchedEffect(selection, columns) {
        weights.forEachIndexed { index, weight ->
            val target = if (index == selection) 1f else 0f
            launch {
                if (motion == null || reduce) weight.snapTo(target) else weight.animateTo(target, motion.spec())
            }
        }
    }
    val shape = RoundedCornerShape(corner)
    val sized = if (height == null) modifier.fillMaxSize() else modifier.fillMaxWidth().height(height)
    Layout(content, sized) { measurables, constraints ->
        val n = measurables.size
        val gapPx = gap.toPx()
        val heightPx = constraints.constrainHeight(height?.roundToPx()
            ?: if (constraints.hasBoundedHeight) constraints.maxHeight else 560.dp.roundToPx())
        val total = if (constraints.hasBoundedWidth) {
            constraints.maxWidth.toFloat()
        } else {
            collapsedWidth.toPx() * n + gapPx * max(0, n - 1)
        }
        val available = max(0f, total - gapPx * max(0, n - 1))
        val collapsed = if (n > 0) min(collapsedWidth.toPx(), available / n) else 0f
        val extra = max(0f, available - collapsed * n)
        var widths = (0 until n).map { index -> max(0f, collapsed + extra * (weights.getOrNull(index)?.value ?: 0f)) }
        // A spring overshooting zero still fits the row.
        val sum = widths.sum()
        if (sum > 0f) widths = widths.map { it * available / sum }
        val placeables = measurables.mapIndexed { index, measurable ->
            measurable.measure(Constraints.fixed(widths[index].roundToInt(), heightPx))
        }
        layout(total.roundToInt(), heightPx) {
            var x = 0f
            placeables.forEachIndexed { index, placeable ->
                placeable.placeWithLayer(x.roundToInt(), 0) {
                    this.shape = shape
                    clip = true
                }
                x += widths[index] + gapPx
            }
        }
    }
}

/// `Flow`: items at their own width from the leading edge, wrapping onto
/// a new row where the width runs out; each row centres its items, and a
/// spacer (`weight`) takes the room left on its row.
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BernFlow(gap: Dp, content: @Composable FlowRowScope.() -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(gap),
        verticalArrangement = Arrangement.spacedBy(gap),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) { content() }
}

// MARK: - Modals

/// `Modal`: a Material dialog over the window, which dims what lies below
/// and takes no input there: a card of `width` × `height` (without a
/// height, as tall as its content), at least 16 points from the window's
/// edges, or with `cover` the whole window where
/// it is compact. The element's `appear` and `disappear` play on the card;
/// inside an animated branch the branch's clock drives them, so the card
/// leaves with its transition before the dialog closes.
@OptIn(androidx.compose.animation.ExperimentalSharedTransitionApi::class)
@Composable
fun BernModal(
    width: Dp,
    height: Dp,
    cover: Boolean,
    enter: EnterTransition?,
    exit: ExitTransition?,
    content: @Composable () -> Unit,
) {
    val window = LocalBernWindow.current
    val covering = cover && window.compact
    val visibility = LocalBernVisibility.current
    val dark = bernDark()
    Dialog(
        onDismissRequest = {},
        properties = DialogProperties(
            dismissOnBackPress = false,
            dismissOnClickOutside = false,
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
        ),
    ) {
        val view = LocalView.current
        SideEffect {
            (view.parent as? DialogWindowProvider)?.window?.let { dialogWindow ->
                dialogWindow.setDimAmount(0.24f)
                dialogWindow.setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
                // A modal has its own Window; the activity's system-bar
                // appearance does not carry into an edge-to-edge dialog.
                val controller = androidx.core.view.WindowCompat.getInsetsController(dialogWindow, view)
                controller.isAppearanceLightStatusBars = !dark
                controller.isAppearanceLightNavigationBars = !dark
            }
        }
        // Measure the card in the space above the keyboard, so scrollable
        // fields can shrink while the dialog's footer remains reachable.
        val viewport = Modifier.fillMaxSize().imePadding()
            .then(if (covering) Modifier else Modifier.safeDrawingPadding())
        BoxWithConstraints(viewport, contentAlignment = Alignment.Center) {
            val card = if (covering) {
                Modifier.fillMaxSize()
            } else {
                val cardWidth = min(width.value, maxWidth.value - 32f).coerceAtLeast(0f).dp
                val most = (maxHeight.value - 32f).coerceAtLeast(0f)
                if (height == Dp.Unspecified) Modifier.width(cardWidth).bernFitHeight(most.dp)
                else Modifier.size(cardWidth, min(height.value, most).dp)
            }
            val surface: @Composable (Modifier) -> Unit = { modifier ->
                Surface(
                    modifier = modifier,
                    shape = if (covering) RectangleShape else AlertDialogDefaults.shape,
                    color = AlertDialogDefaults.containerColor,
                    tonalElevation = AlertDialogDefaults.TonalElevation,
                    shadowElevation = if (covering) 0.dp else 8.dp,
                ) {
                    // The dialog is a window of its own: its elements cannot
                    // pair with the shared elements of the window below,
                    // whose layout is another hierarchy.
                    Box(if (covering) Modifier.statusBarsPadding().navigationBarsPadding() else Modifier) {
                        CompositionLocalProvider(LocalBernShared provides null) { content() }
                    }
                }
            }
            if (visibility != null) {
                val moving = with(visibility) {
                    Modifier.animateEnterExit(enter = enter ?: EnterTransition.None, exit = exit ?: ExitTransition.None)
                }
                surface(card.then(moving))
            } else {
                val state = remember { MutableTransitionState(enter == null).apply { targetState = true } }
                AnimatedVisibility(visibleState = state, enter = enter ?: EnterTransition.None) { surface(card) }
            }
        }
    }
}

/// A modal card without a height: as tall as its content at its width
/// (a scroll view as tall as what it scrolls, a spacer nothing), at most
/// `most`. Content that cannot say its height before it is measured, such
/// as a lazy list, takes `most`.
private fun Modifier.bernFitHeight(most: Dp) = layout { measurable, constraints ->
    val room = constraints.constrainHeight(most.roundToPx())
    val natural = try {
        measurable.maxIntrinsicHeight(constraints.maxWidth)
    } catch (_: IllegalStateException) {
        room
    }
    val height = natural.coerceIn(constraints.minHeight, room)
    val placeable = measurable.measure(constraints.copy(minHeight = height, maxHeight = height))
    layout(placeable.width, placeable.height) { placeable.place(0, 0) }
}

// MARK: - Timers

/// `Timer`: runs `action` every `interval` seconds (at least 0.05) while
/// composed and enabled; turning it off or leaving cancels the next one.
@Composable
fun BernTimer(interval: Double, enabled: Boolean, action: () -> Unit) {
    val current by rememberUpdatedState(action)
    LaunchedEffect(enabled, interval) {
        if (!enabled) return@LaunchedEffect
        val millis = (max(0.05, interval) * 1000).toLong()
        while (true) {
            delay(millis)
            current()
        }
    }
}

// MARK: - Watch

/// `Watch`: runs its action when composed and whenever `value` changes.
@Composable
fun BernWatch(value: Any?, action: () -> Unit) {
    val current by rememberUpdatedState(action)
    LaunchedEffect(value) { current() }
}

// MARK: - Control groups

/// `ControlGroup`: related controls on one capsule of the tonal surface,
/// 40 points tall unless `height` says, named by `label`.
@Composable
fun BernControlGroup(label: String?, height: Dp?, content: @Composable RowScope.() -> Unit) {
    Surface(
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.height(height ?: 40.dp).semantics { if (label != null) contentDescription = label },
    ) {
        Row(
            Modifier.padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(2.dp),
            content = content,
        )
    }
}

// MARK: - Forms

/// The rows of a form's sections: their least height and their insets
/// across and along, from `Form` or `Section`.
class BernRows(val height: Dp?, val x: Dp?, val y: Dp?)

val LocalBernFormRows = staticCompositionLocalOf<BernRows?> { null }

/// Whether a view stands in a section's rows, which give it its height and
/// insets: a row there adds no room of its own.
val LocalBernInRows = staticCompositionLocalOf { false }

/// The top edge of a plain section. A form's sections pass it up, so the
/// form sees whether one stands at the top of its rows, wherever it came
/// from: a slot, a component, a condition.
private val BernHeadingTop = HorizontalAlignmentLine(::min)

/// A scrolling form: its sections `gap` apart (20 unless given). One
/// opening with a plain section starts at the top of its page.
@Composable
fun BernForm(gap: Dp?, rows: BernRows, content: @Composable ColumnScope.() -> Unit) {
    CompositionLocalProvider(LocalBernFormRows provides rows) {
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(start = 16.dp, end = 16.dp, bottom = 16.dp)
                .bernFormTop(16.dp),
            verticalArrangement = Arrangement.spacedBy(gap ?: 20.dp),
            content = content,
        )
    }
}

/// Room `top` above a form's rows, unless a plain section opens them. The
/// form keeps at least the height it is asked for: a smaller one would be
/// centered in it, moving the rows down.
private fun Modifier.bernFormTop(top: Dp) = layout { measurable, constraints ->
    val room = top.roundToPx()
    val rows = measurable.measure(constraints.offset(vertical = -room))
    val above = if (rows[BernHeadingTop] == 0) 0 else room
    layout(constraints.constrainWidth(rows.width), constraints.constrainHeight(rows.height + above)) {
        rows.place(0, above)
    }
}

/// A section of a form: its title, its rows and its footer. With row
/// metrics (its own `padding_x`, `padding_y`, or the form's, and the form's
/// `row_height`), each child is a row inset by them and at least that
/// tall, its content centered along; without, the children stand 8 apart.
/// A plain section is a heading on the page, as Material's sections are:
/// its rows keep the page's layout, whatever the form gives its groups,
/// only its own insets apply, and they stand without room between them.
@Composable
fun BernSection(
    title: String?,
    footer: String?,
    plain: Boolean = false,
    paddingX: Dp? = null,
    paddingY: Dp? = null,
    content: @Composable () -> Unit,
) {
    val form = LocalBernFormRows.current?.takeUnless { plain }
    val x = paddingX ?: form?.x
    val y = paddingY ?: form?.y
    val height = form?.height
    val heading = if (plain) Modifier.bernHeading() else Modifier
    Column(Modifier.fillMaxWidth().then(heading), verticalArrangement = Arrangement.spacedBy(if (plain) 0.dp else 8.dp)) {
        if (title != null) {
            Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
        if (!plain && x == null && y == null && height == null) {
            Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) { content() }
        } else {
            CompositionLocalProvider(LocalBernInRows provides true) {
                BernRowsLayout(height, x ?: 0.dp, y ?: 0.dp, content)
            }
        }
        if (footer != null) {
            Text(footer, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/// Marks a plain section's top edge for the form around it.
private fun Modifier.bernHeading() = layout { measurable, constraints ->
    val section = measurable.measure(constraints)
    layout(section.width, section.height, mapOf(BernHeadingTop to 0)) { section.place(0, 0) }
}

/// Each child a row the full width: inset `x` across and `y` along, at
/// least `height` tall, its content centered along.
@Composable
private fun BernRowsLayout(height: Dp?, x: Dp, y: Dp, content: @Composable () -> Unit) {
    Layout(content, Modifier.fillMaxWidth()) { measurables, constraints ->
        val inX = x.roundToPx()
        val inY = y.roundToPx()
        val least = height?.roundToPx() ?: 0
        val width = constraints.maxWidth
        val inner = max(0, width - 2 * inX)
        val placeables = measurables.map { it.measure(Constraints(minWidth = 0, maxWidth = inner)) }
        val rows = placeables.map { max(least, it.height + 2 * inY) }
        layout(width, rows.sum()) {
            var top = 0
            placeables.forEachIndexed { index, placeable ->
                placeable.place(inX, top + (rows[index] - placeable.height) / 2)
                top += rows[index]
            }
        }
    }
}

// MARK: - Search palette

/// One result a palette moves to with the arrow keys: where it is on
/// screen and what choosing it does.
class BernPaletteEntry(var top: Float, var activate: () -> Unit)

/// An open palette: its results in screen order, the one the keys stand
/// at, and how it closes.
class BernPaletteState(val dismiss: () -> Unit) {
    val entries = mutableStateMapOf<Any, BernPaletteEntry>()
    var current by mutableStateOf<Any?>(null)

    private fun ordered(): List<Any> = entries.entries.sortedBy { it.value.top }.map { it.key }

    fun move(step: Int) {
        val order = ordered()
        if (order.isEmpty()) return
        val at = order.indexOf(current)
        current = when {
            at < 0 -> if (step > 0) order.first() else order.last()
            else -> order[(at + step).coerceIn(0, order.size - 1)]
        }
    }

    /// Chooses the current result; false when there is none.
    fun choose(): Boolean {
        val entry = current?.let { entries[it] } ?: return false
        entry.activate()
        return true
    }
}

val LocalBernSearchPalette = compositionLocalOf<BernPaletteState?> { null }

/// The palettes on screen by target, so a shortcut opens one.
object BernSearchPalettes {
    private val openers = HashMap<String, () -> Unit>()

    fun register(target: String, open: () -> Unit) {
        openers[target] = open
    }

    fun remove(target: String, open: () -> Unit) {
        if (openers[target] === open) openers.remove(target)
    }

    fun open(target: String) {
        openers[target]?.invoke()
    }
}

/// `Search(style: palette)`: a search button that opens the search over
/// the window, a Material dialog: the field and a way to close it on top,
/// the results below, built only while it is open. A compact window shows
/// it full screen, a wider one as a card of `width` × `height`. The arrow
/// keys move among the results, Return chooses the current one or else
/// submits, Escape or the system back closes it. External entries open it
/// with `activate` or `focus`.
@Composable
fun BernSearchPalette(
    target: String,
    text: String,
    onText: (String) -> Unit,
    prompt: String,
    panelPrompt: String,
    width: Dp,
    height: Dp,
    submit: (() -> Unit)?,
    results: @Composable ColumnScope.() -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val opener = remember { { open = true } }
    DisposableEffect(target, opener) {
        BernSearchPalettes.register(target, opener)
        onDispose { BernSearchPalettes.remove(target, opener) }
    }
    BernControl(
        target, "search", prompt,
        actions = BernControlActions(
            activate = opener, edit = onText, submit = submit, value = { if (open) "open: $text" else text }, focus = opener,
        ),
    )
    IconButton(onClick = opener, modifier = Modifier.testTag(target)) {
        Icon(Icons.Filled.Search, contentDescription = prompt)
    }
    if (!open) return
    val window = LocalBernWindow.current
    val compact = window.compact
    Dialog(
        onDismissRequest = { open = false },
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = !compact),
    ) {
        val state = remember { BernPaletteState { open = false } }
        val focus = remember { FocusRequester() }
        BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = if (compact) Alignment.TopCenter else Alignment.Center) {
            val card = if (compact) {
                Modifier.fillMaxSize()
            } else {
                Modifier.size(min(width.value, maxWidth.value - 32f).coerceAtLeast(0f).dp, min(height.value, maxHeight.value - 32f).coerceAtLeast(0f).dp)
            }
            Surface(
                modifier = card.onPreviewKeyEvent { event ->
                    if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    when (event.key) {
                        Key.DirectionDown -> { state.move(1); true }
                        Key.DirectionUp -> { state.move(-1); true }
                        Key.Enter, Key.NumPadEnter -> {
                            if (!state.choose()) submit?.invoke()
                            true
                        }
                        Key.Escape -> { open = false; true }
                        else -> false
                    }
                },
                shape = if (compact) RectangleShape else AlertDialogDefaults.shape,
                color = AlertDialogDefaults.containerColor,
                tonalElevation = AlertDialogDefaults.TonalElevation,
                shadowElevation = if (compact) 0.dp else 8.dp,
            ) {
                CompositionLocalProvider(LocalBernSearchPalette provides state) {
                    Column(Modifier.fillMaxSize().then(if (compact) Modifier.statusBarsPadding().navigationBarsPadding() else Modifier)) {
                        Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            TextField(
                                value = text,
                                onValueChange = onText,
                                placeholder = { Text(panelPrompt) },
                                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                                singleLine = true,
                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                                keyboardActions = KeyboardActions(onSearch = { if (!state.choose()) submit?.invoke() }),
                                colors = TextFieldDefaults.colors(
                                    focusedIndicatorColor = Color.Transparent,
                                    unfocusedIndicatorColor = Color.Transparent,
                                ),
                                shape = CircleShape,
                                modifier = Modifier.weight(1f).focusRequester(focus),
                            )
                            IconButton(onClick = { open = false }) {
                                Icon(Icons.Filled.Close, contentDescription = BernText.string("bern.window.done"))
                            }
                        }
                        HorizontalDivider()
                        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(vertical = 4.dp), content = results)
                    }
                }
            }
        }
        LaunchedEffect(Unit) { focus.requestFocus() }
    }
}

/// `SearchItem`: a result row with its icon, label, detail and hint, or
/// the content given instead. Choosing it closes the palette it stands in
/// and then runs its action, for the user and for external entries alike;
/// with `navigation` the arrow keys stop at it, and the one they stand at
/// is highlighted.
@Composable
fun BernSearchItem(
    target: String,
    label: String,
    icon: ImageVector?,
    detail: String?,
    hint: String?,
    navigation: Boolean,
    actions: BernControlActions,
    content: (@Composable () -> Unit)? = null,
) {
    val palette = LocalBernSearchPalette.current
    val choose: () -> Unit = {
        palette?.dismiss()
        actions.activate?.invoke()
    }
    BernControl(
        target, "button", label,
        actions = BernControlActions(
            activate = choose,
            value = actions.value,
            setFile = actions.setFile?.let { set -> { url: String -> palette?.dismiss(); set(url) } },
            fileKinds = actions.fileKinds,
        ),
    )
    val token = remember { Any() }
    val entry = remember { BernPaletteEntry(0f, choose) }
    entry.activate = choose
    if (palette != null && navigation) {
        DisposableEffect(palette, token) {
            palette.entries[token] = entry
            onDispose {
                palette.entries.remove(token)
                if (palette.current == token) palette.current = null
            }
        }
    }
    val current = palette != null && palette.current == token
    val row = Modifier
        .fillMaxWidth()
        .testTag(target)
        .onGloballyPositioned { entry.top = it.positionInRoot().y }
        .pointerInput(palette, navigation) {
            if (palette == null || !navigation) return@pointerInput
            awaitPointerEventScope {
                while (true) {
                    val event = awaitPointerEvent()
                    if (event.type == PointerEventType.Enter || event.type == PointerEventType.Move) palette.current = token
                }
            }
        }
        .clickable(onClick = choose)
    if (content != null) {
        Box(
            row
                .background(if (current) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent)
                .padding(horizontal = 16.dp, vertical = 8.dp),
        ) { content() }
        return
    }
    ListItem(
        headlineContent = { Text(label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = detail?.let { { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) } },
        leadingContent = icon?.let { { Icon(it, contentDescription = null) } },
        trailingContent = hint?.let { { Text(it, style = MaterialTheme.typography.labelMedium) } },
        colors = ListItemDefaults.colors(
            containerColor = if (current) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent,
        ),
        modifier = row,
    )
}

// MARK: - Search fields of a set size

/// The search field standing in a page at its own `height` and text
/// `fontSize`: Material's search field shape, a capsule of the tonal
/// surface with the search icon, the prompt while empty and a way to clear
/// it.
@Composable
fun BernSizedSearchField(
    value: String,
    onChange: (String) -> Unit,
    prompt: String,
    submit: (() -> Unit)?,
    modifier: Modifier,
    width: Dp?,
    height: Dp?,
    fontSize: Float?,
) {
    val style = MaterialTheme.typography.bodyLarge.let { base ->
        if (fontSize != null) base.copy(fontSize = fontSize.sp, lineHeight = TextUnit.Unspecified) else base
    }.copy(color = MaterialTheme.colorScheme.onSurface)
    val iconSize = bernIconSize((fontSize ?: 16f) * 1.1f)
    Surface(
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = modifier
            .then(if (width != null) Modifier.width(width) else Modifier.fillMaxWidth())
            .height(height ?: 56.dp),
    ) {
        Row(Modifier.padding(horizontal = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Search, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(iconSize))
            Spacer(Modifier.width(6.dp))
            BasicTextField(
                value = value,
                onValueChange = onChange,
                singleLine = true,
                textStyle = style,
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { submit?.invoke() }),
                modifier = Modifier.weight(1f).semantics { contentDescription = prompt },
                decorationBox = { field ->
                    Box(contentAlignment = Alignment.CenterStart) {
                        if (value.isEmpty()) Text(prompt, style = style, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                        field()
                    }
                },
            )
            if (value.isNotEmpty()) {
                Icon(
                    Icons.Filled.Close,
                    contentDescription = BernText.string("bern.window.done"),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(iconSize).clip(CircleShape).clickable { onChange("") },
                )
            }
        }
    }
}
