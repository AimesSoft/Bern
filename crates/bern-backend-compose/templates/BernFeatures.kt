// Bern Compose features. Generated build input; do not edit.
//
// What media applications need beyond the standard components: the theme
// chosen at run time, parts fixed to one appearance, keyboard shortcuts,
// lengths that follow state, panels, notices, files, scrolling that rests
// on items, gestures bound to values, rails and popover buttons.

package dev.bern.runtime

import android.content.Context
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.DecayAnimationSpec
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.calculateTargetValue
import androidx.compose.animation.rememberSplineBasedDecay
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.gestures.FlingBehavior
import androidx.compose.foundation.gestures.ScrollScope
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.unit.toSize
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.setProgress
import kotlinx.coroutines.flow.first
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconToggleButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.ripple
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.delay
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.TextButton
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.constrainHeight
import androidx.compose.ui.unit.constrainWidth
import kotlin.math.abs
import kotlin.math.roundToInt
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp

// MARK: - Theme

/// The accent and appearance the application chose at run time
/// (`WindowSet(accent:, appearance:)`); unset, the system's.
object BernThemeState {
    var accent by mutableStateOf<BernColor?>(null)
    var appearance by mutableStateOf<String?>(null)
}

/// Whether what is drawn here is dark: a part fixed to one appearance,
/// the application's choice, or the system's.
val LocalBernDark = compositionLocalOf<Boolean?> { null }

@Composable
fun bernDark(): Boolean = LocalBernDark.current ?: isSystemInDarkTheme()

/// Material colors for `dark`, with the application's accent as primary.
@Composable
fun BernScheme(dark: Boolean, content: @Composable () -> Unit) {
    val context = LocalContext.current
    var colors = if (Build.VERSION.SDK_INT >= 31) {
        if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    } else {
        if (dark) darkColorScheme() else lightColorScheme()
    }
    BernThemeState.accent?.let { accent ->
        val color = BernPalette(colors, dark).color(accent)
        val on = if (color.luminance() > 0.4f) Color(0xFF111114) else Color.White
        colors = colors.copy(primary = color, onPrimary = on, inversePrimary = color, surfaceTint = color)
    }
    CompositionLocalProvider(LocalBernDark provides dark) {
        MaterialTheme(colorScheme = colors, content = content)
    }
}

/// Content always drawn light or dark (`appearance:`), whatever the
/// window's appearance; what opens from it follows it.
@Composable
fun BernAppearance(dark: Boolean, content: @Composable () -> Unit) {
    BernScheme(dark) {
        CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface, content = content)
    }
}

/// The `WindowSet`'s accent and appearance, applied to every window.
@Composable
fun BernThemeChoice(accent: BernColor?, appearance: String?) {
    SideEffect {
        BernThemeState.accent = accent
        BernThemeState.appearance = appearance
    }
}

// MARK: - Shortcuts

/// Keyboard shortcuts of the controls on screen: `cmd` is Control here,
/// the primary modifier off Apple systems.
object BernShortcuts {
    private val entries = LinkedHashMap<Any, Pair<String, () -> Unit>>()

    fun register(token: Any, shortcut: String, action: () -> Unit) {
        entries[token] = shortcut to action
    }

    fun remove(token: Any) {
        entries.remove(token)
    }

    /// Runs the action whose shortcut `event` presses; whether one did.
    /// Counts every key pressed in the app, for `window.idle`.
    var keys by mutableLongStateOf(0L)
        private set

    fun pressed(event: android.view.KeyEvent) {
        if (event.action == android.view.KeyEvent.ACTION_DOWN) keys += 1
    }

    fun handle(event: KeyEvent): Boolean {
        if (event.type != KeyEventType.KeyDown) return false
        val key = keyName(event.nativeKeyEvent.keyCode) ?: return false
        val pressed = buildList {
            if (event.isCtrlPressed || event.isMetaPressed) add("cmd")
            if (event.isAltPressed) add("alt")
            if (event.isShiftPressed) add("shift")
            add(key)
        }.joinToString("+")
        val action = entries.values.lastOrNull { (shortcut, _) ->
            shortcut.replace("ctrl+", "cmd+").replace("cmd+cmd+", "cmd+") == pressed
        }?.second ?: return false
        action()
        return true
    }

    private fun keyName(code: Int): String? = when (code) {
        android.view.KeyEvent.KEYCODE_SPACE -> "space"
        android.view.KeyEvent.KEYCODE_ESCAPE -> "escape"
        android.view.KeyEvent.KEYCODE_ENTER -> "return"
        android.view.KeyEvent.KEYCODE_TAB -> "tab"
        android.view.KeyEvent.KEYCODE_DEL -> "delete"
        android.view.KeyEvent.KEYCODE_DPAD_UP -> "up"
        android.view.KeyEvent.KEYCODE_DPAD_DOWN -> "down"
        android.view.KeyEvent.KEYCODE_DPAD_LEFT -> "left"
        android.view.KeyEvent.KEYCODE_DPAD_RIGHT -> "right"
        android.view.KeyEvent.KEYCODE_MOVE_HOME -> "home"
        android.view.KeyEvent.KEYCODE_MOVE_END -> "end"
        android.view.KeyEvent.KEYCODE_PAGE_UP -> "pageup"
        android.view.KeyEvent.KEYCODE_PAGE_DOWN -> "pagedown"
        android.view.KeyEvent.KEYCODE_COMMA -> ","
        android.view.KeyEvent.KEYCODE_PERIOD -> "."
        android.view.KeyEvent.KEYCODE_MINUS -> "-"
        android.view.KeyEvent.KEYCODE_EQUALS -> "="
        android.view.KeyEvent.KEYCODE_SLASH -> "/"
        android.view.KeyEvent.KEYCODE_SEMICOLON -> ";"
        android.view.KeyEvent.KEYCODE_APOSTROPHE -> "'"
        android.view.KeyEvent.KEYCODE_LEFT_BRACKET -> "["
        android.view.KeyEvent.KEYCODE_RIGHT_BRACKET -> "]"
        android.view.KeyEvent.KEYCODE_BACKSLASH -> "\\"
        android.view.KeyEvent.KEYCODE_GRAVE -> "`"
        in android.view.KeyEvent.KEYCODE_A..android.view.KeyEvent.KEYCODE_Z ->
            ('a' + (code - android.view.KeyEvent.KEYCODE_A)).toString()
        in android.view.KeyEvent.KEYCODE_0..android.view.KeyEvent.KEYCODE_9 ->
            ('0' + (code - android.view.KeyEvent.KEYCODE_0)).toString()
        else -> null
    }
}

/// A control's keyboard shortcut while it is composed.
@Composable
fun BernShortcut(shortcut: String, action: () -> Unit) {
    val token = remember { Any() }
    SideEffect { BernShortcuts.register(token, shortcut, action) }
    DisposableEffect(token) { onDispose { BernShortcuts.remove(token) } }
}

// MARK: - Lengths

/// A length that follows state, moving to each new value with `motion`;
/// at once when the system turned animations off. A spring that overshoots
/// zero stops there, since lengths are never negative.
@Composable
fun bernLength(value: Dp, motion: BernMotion): Dp {
    if (bernReduceMotion()) return value
    val moving by animateDpAsState(value, motion.spec(), label = "length")
    return moving.coerceAtLeast(0.dp)
}

// MARK: - Shadows

/// The look's shadow, as if lifted off the page, growing and settling with
/// the state's spring.
@Composable
fun Modifier.bernShadow(look: BernLook, motion: BernMotion, shape: Shape): Modifier {
    val radius = look.shadowRadius ?: 0.0
    val elevation by animateDpAsState(
        radius.dp,
        if (bernReduceMotion()) snap() else motion.spec(),
        label = "shadow",
    )
    if (elevation.value <= 0f) return this
    val shade = Color.Black.copy(alpha = (look.shadowOpacity ?: 0.2).toFloat())
    return shadow(elevation, shape, clip = false, ambientColor = shade, spotColor = shade)
}

// MARK: - Buttons with content

/// A button that shows its content instead of a label: the style draws its
/// background, shadow and transforms; `label` names it for assistive
/// technologies.
@Composable
fun BernContentButton(
    label: String,
    enabled: Boolean,
    style: BernStyle?,
    modifier: Modifier = Modifier,
    shape: BernShape? = null,
    prominent: Boolean = false,
    onClick: () -> Unit,
    content: @Composable () -> Unit,
) {
    if (shape?.capsule == true) {
        // A capsule is the platform's button around the content, as it is
        // around a label.
        val height = shape.height
        val sized = (if (height != null) modifier.height(height) else modifier)
            .semantics(mergeDescendants = true) { contentDescription = label }
        val padding = if (height != null) PaddingValues(horizontal = 16.dp, vertical = 0.dp) else ButtonDefaults.ContentPadding
        if (prominent) {
            androidx.compose.material3.Button(onClick, sized, enabled, shape = CircleShape, contentPadding = padding) { content() }
        } else {
            FilledTonalButton(onClick, sized, enabled, shape = CircleShape, contentPadding = padding) { content() }
        }
        return
    }
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    val hovered by interaction.collectIsHoveredAsState()
    val (look, motion) = (style ?: BernStyle()).look(false, hovered, pressed, enabled)
    val palette = bernPalette()
    // A capsule's ends are half its height; a height of its own sizes it.
    val outline = if (shape?.capsule == true) CircleShape else RoundedCornerShape((style?.corner ?: 0.0).dp)
    val sized = shape?.height?.let { modifier.height(it) } ?: modifier
    Box(
        sized
            .bernTransform(look, motion)
            .bernShadow(look, motion, outline)
            .clip(outline)
            .background(look.background?.let(palette::color) ?: Color.Transparent)
            .clickable(interaction, ripple(), enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = label },
    ) { content() }
}

// MARK: - Toggle buttons

/// A button that stays pressed while on (`Switch(style: button)`).
@Composable
fun BernToggleButton(
    label: String,
    icon: ImageVector?,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    shape: BernShape? = null,
) {
    val height = shape?.height
    if (icon != null) {
        val sized = if (height != null) modifier.size(height) else modifier
        FilledTonalIconToggleButton(checked, onChange, sized.semantics { contentDescription = label }) {
            Icon(icon, contentDescription = null)
        }
    } else {
        val sized = if (height != null) modifier.height(height) else modifier
        FilterChip(
            selected = checked,
            onClick = { onChange(!checked) },
            label = { Text(label) },
            shape = if (shape?.capsule == true) CircleShape else FilterChipDefaults.shape,
            modifier = sized,
        )
    }
}

// MARK: - Search in the toolbar

/// A search the toolbar opens over the window: the field on top and its
/// results below it. External entries focus it with `ui.focus`.
@Composable
fun BernToolbarSearch(
    target: String,
    text: String,
    onText: (String) -> Unit,
    prompt: String,
    submit: (() -> Unit)?,
    results: @Composable ColumnScope.() -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    BernControl(
        target, "search", prompt,
        actions = BernControlActions(edit = onText, submit = submit, value = { text }, focus = { open = true }),
    )
    IconButton(onClick = { open = true }, modifier = Modifier.testTag(target)) {
        Icon(Icons.Filled.Search, contentDescription = prompt)
    }
    if (open) {
        Dialog(onDismissRequest = { open = false }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            val focus = remember { FocusRequester() }
            Surface(Modifier.fillMaxSize()) {
                Column(Modifier.fillMaxSize().statusBarsPadding()) {
                    Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                        TextField(
                            value = text,
                            onValueChange = onText,
                            placeholder = { Text(prompt) },
                            leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                            keyboardActions = KeyboardActions(onSearch = { submit?.invoke() }),
                            modifier = Modifier.weight(1f).focusRequester(focus),
                        )
                        IconButton(onClick = { open = false }) {
                            Icon(Icons.Filled.Close, contentDescription = BernText.string("bern.window.done"))
                        }
                    }
                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), content = results)
                }
            }
            LaunchedEffect(Unit) { focus.requestFocus() }
        }
    }
}

// MARK: - Side panel

/// The side panel of a container: a sheet from the bottom on a phone,
/// shown while `presented`; dismissing it sets `presented` to false.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BernInspector(presented: Boolean, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    if (!presented) return
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

// MARK: - Notices

/// A short notice that shows when it appears or `key` changes and hides
/// itself after `duration` seconds; read out politely.
@Composable
fun BernToast(text: String, icon: ImageVector?, key: Any, duration: Double, motion: BernMotion) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(key) {
        shown = true
        delay((duration * 1000).toLong())
        shown = false
    }
    AnimatedVisibility(
        shown,
        enter = fadeIn(if (bernReduceMotion()) tween(150) else motion.spec()) +
            slideInVertically(if (bernReduceMotion()) snap() else motion.spec()) { it / 2 },
        exit = fadeOut(tween(250)),
    ) {
        Surface(
            shape = RoundedCornerShape(50),
            tonalElevation = 3.dp,
            shadowElevation = 6.dp,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
        ) {
            Row(
                Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                icon?.let { Icon(it, contentDescription = null) }
                Text(text)
            }
        }
    }
}

// MARK: - Split buttons

/// A toggle linked with a button that shows `content` in a menu, such as
/// the usual settings of what the toggle turns on. External entries turn
/// it on or off with `set_bool` and open the menu with `activate`.
@Composable
fun BernSplitButton(
    target: String,
    label: String,
    icon: ImageVector?,
    more: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    shape: BernShape? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    var showing by remember { mutableStateOf(false) }
    BernControl(
        target, "split_button", label,
        actions = BernControlActions(activate = { showing = true }, setBool = onChange, value = { if (checked) "true" else "false" }),
    )
    Row(Modifier.testTag(target), verticalAlignment = Alignment.CenterVertically) {
        BernToggleButton(label, icon, checked, onChange, shape = shape)
        Box {
            IconButton(onClick = { showing = true }, modifier = shape?.height?.let { Modifier.size(it) } ?: Modifier) {
                Icon(Icons.Filled.ArrowDropDown, contentDescription = more)
            }
            DropdownMenu(expanded = showing, onDismissRequest = { showing = false }) {
                Column(
                    Modifier.padding(16.dp).widthIn(min = 240.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    content = content,
                )
            }
        }
    }
}

// MARK: - Pictures from the engine

/// Pictures read at run time from a `MediaSource` (a file, content or
/// http(s) URL), and the average color of each, for shapes filled from a
/// picture. Each is read once, off the main thread.
object BernPictures {
    private val bitmaps = mutableStateMapOf<String, ImageBitmap>()
    private val averages = mutableStateMapOf<String, Long>()
    private val loading = HashSet<String>()

    fun bitmap(context: Context, url: String?): ImageBitmap? {
        if (url.isNullOrEmpty()) return null
        bitmaps[url]?.let { return it }
        load(context, url)
        return null
    }

    fun average(context: Context, url: String?): BernColor? {
        if (url.isNullOrEmpty()) return null
        averages[url]?.let { return BernColor.Fixed(it, it) }
        load(context, url)
        return null
    }

    /// The URL of a declared resource.
    fun resource(name: String): String = Uri.fromFile(File(BernResources.root, name)).toString()

    private fun load(context: Context, url: String) {
        if (!loading.add(url)) return
        val app = context.applicationContext
        CoroutineScope(Dispatchers.IO).launch {
            val bitmap = runCatching { decode(app, url) }.getOrNull() ?: return@launch
            val average = mean(bitmap)
            withContext(Dispatchers.Main) {
                bitmaps[url] = bitmap.asImageBitmap()
                averages[url] = average
            }
        }
    }

    /// The mean color over the whole picture: a single scaled pixel would
    /// sample only its middle.
    private fun mean(bitmap: Bitmap): Long {
        val small = Bitmap.createScaledBitmap(bitmap, 32, 32, true)
        var red = 0L
        var green = 0L
        var blue = 0L
        for (y in 0 until 32) for (x in 0 until 32) {
            val pixel = small.getPixel(x, y)
            red += (pixel shr 16) and 0xFF
            green += (pixel shr 8) and 0xFF
            blue += pixel and 0xFF
        }
        val count = 32 * 32
        return ((red / count) shl 16) or ((green / count) shl 8) or (blue / count)
    }

    private fun decode(context: Context, url: String): Bitmap? {
        val uri = Uri.parse(url)
        return when (uri.scheme?.lowercase()) {
            "file" -> BitmapFactory.decodeFile(uri.path)
            "content" -> context.contentResolver.openInputStream(uri)?.use(BitmapFactory::decodeStream)
            "http", "https" -> java.net.URL(url).openStream().use(BitmapFactory::decodeStream)
            else -> null
        }
    }
}

/// A picture from a `MediaSource`; until it is read, its frame keeps the
/// declared size.
@Composable
fun BernImageURL(
    url: String?,
    label: String?,
    width: Dp?,
    height: Dp?,
    fit: ContentScale,
    corner: Dp? = null,
    template: Boolean = false,
    aspect: Float? = null,
) {
    val bitmap = BernPictures.bitmap(LocalContext.current, url)
    val ratio = aspect ?: bitmap?.let { it.width.toFloat() / maxOf(1, it.height) } ?: (2f / 3f)
    val modifier = bernPictureFrame(width, height, ratio, aspect != null, corner)
    if (bitmap != null) {
        Image(
            bitmap,
            contentDescription = label,
            modifier = modifier,
            contentScale = fit,
            colorFilter = if (template) androidx.compose.ui.graphics.ColorFilter.tint(LocalContentColor.current, androidx.compose.ui.graphics.BlendMode.SrcIn) else null,
        )
    } else {
        Box(modifier.semantics { if (label != null) contentDescription = label })
    }
}

// MARK: - Files

/// The kinds of file a control takes (`files:`), the system's document
/// picker, files dropped on the window and files other apps open with
/// this one.
object BernFiles {
    /// Extensions each category takes besides the system's own types.
    private val extensions: Map<String, List<String>> = mapOf(/*bern:file-extensions*/)
    private var pick: ActivityResultLauncher<Array<String>>? = null
    private var pickFolder: ActivityResultLauncher<Uri?>? = null
    private var picked: ((String) -> Unit)? = null
    private var context: Context? = null
    private val openers = LinkedHashMap<Any, Pair<String, (String) -> Unit>>()
    private val waiting = ArrayList<String>()

    /// Called from the activity's `onCreate`, before it starts.
    fun register(activity: ComponentActivity) {
        context = activity.applicationContext
        pick = activity.registerForActivityResult(ActivityResultContracts.OpenDocument()) { give(it) }
        pickFolder = activity.registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { give(it) }
        opened(activity.intent)
    }

    private fun give(uri: Uri?) {
        val then = picked
        picked = null
        if (uri == null || then == null) return
        // Readable after the app restarts too.
        runCatching {
            context?.contentResolver?.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        then(uri.toString())
    }

    private fun words(kinds: String) = kinds.split(' ').filter { it.isNotEmpty() }

    /// Asks for one file of `kinds` with the system's document picker.
    fun pick(kinds: String, then: (String) -> Unit) {
        picked = then
        if (words(kinds) == listOf("folder")) pickFolder?.launch(null) else pick?.launch(mimeTypes(kinds))
    }

    fun mimeTypes(kinds: String): Array<String> {
        val types = LinkedHashSet<String>()
        for (word in words(kinds)) {
            when {
                word.startsWith(".") -> types.add(mimeType(word.drop(1)))
                word == "video" -> types.add("video/*")
                word == "audio" -> types.add("audio/*")
                word == "picture" -> types.add("image/*")
                else -> types.add("*/*")
            }
            for (ext in extensions[word].orEmpty()) types.add(mimeType(ext))
        }
        return types.toTypedArray()
    }

    private fun mimeType(ext: String): String =
        MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"

    /// Whether `url` names a file of `kinds`: by its extension, or by the
    /// type the system knows for it.
    fun accepts(url: String, kinds: String): Boolean {
        val words = words(kinds)
        if ("any" in words) return true
        val uri = Uri.parse(url)
        val name = displayName(uri) ?: uri.lastPathSegment ?: ""
        val ext = name.substringAfterLast('.', "").lowercase()
        val type = context?.contentResolver?.takeIf { uri.scheme == "content" }?.getType(uri)
            ?: mimeType(ext)
        return words.any { word ->
            when {
                word.startsWith(".") -> ext == word.drop(1)
                word == "folder" -> type == android.provider.DocumentsContract.Document.MIME_TYPE_DIR || uri.path?.endsWith("/") == true
                else -> ext in extensions[word].orEmpty() || when (word) {
                    "video" -> type.startsWith("video/")
                    "audio" -> type.startsWith("audio/")
                    "picture" -> type.startsWith("image/")
                    else -> false
                }
            }
        }
    }

    private fun displayName(uri: Uri): String? {
        if (uri.scheme != "content") return null
        return runCatching {
            context?.contentResolver?.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        }.getOrNull()
    }

    fun register(token: Any, kinds: String, give: (String) -> Unit) {
        openers[token] = kinds to give
        deliver()
    }

    fun unregister(token: Any) {
        openers.remove(token)
    }

    /// A file another app opens with this one; it waits for a drop that
    /// takes its kind to be on screen.
    fun opened(intent: Intent?) {
        val uri = when (intent?.action) {
            Intent.ACTION_VIEW -> intent.data
            Intent.ACTION_SEND -> if (Build.VERSION.SDK_INT >= 33) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM)
            }
            else -> null
        } ?: return
        waiting.add(uri.toString())
        deliver()
    }

    private fun deliver() {
        if (openers.isEmpty()) return
        val urls = ArrayList(waiting)
        waiting.clear()
        for (url in urls) {
            openers.values.firstOrNull { accepts(url, it.first) }?.second?.invoke(url)
        }
    }
}

/// Content that takes a file of `kinds` dropped onto it. `over` follows
/// whether such a file is held above it; with `opens`, it also takes the
/// files other apps open with this one.
@Composable
fun BernDrop(
    kinds: String,
    over: ((Boolean) -> Unit)?,
    opens: Boolean,
    give: (String) -> Unit,
    content: @Composable () -> Unit,
) {
    val token = remember { Any() }
    if (opens) {
        SideEffect { BernFiles.register(token, kinds, give) }
        DisposableEffect(token) { onDispose { BernFiles.unregister(token) } }
    }
    val activity = LocalContext.current as? android.app.Activity
    val target = remember(kinds, give) {
        object : DragAndDropTarget {
            override fun onEntered(event: DragAndDropEvent) { over?.invoke(true) }
            override fun onExited(event: DragAndDropEvent) { over?.invoke(false) }
            override fun onEnded(event: DragAndDropEvent) { over?.invoke(false) }
            override fun onDrop(event: DragAndDropEvent): Boolean {
                over?.invoke(false)
                val drag = event.toAndroidDragEvent()
                activity?.requestDragAndDropPermissions(drag)
                val clip = drag.clipData ?: return false
                for (index in 0 until clip.itemCount) {
                    val url = clip.getItemAt(index).uri?.toString() ?: continue
                    if (BernFiles.accepts(url, kinds)) {
                        give(url)
                        return true
                    }
                }
                return false
            }
        }
    }
    Box(
        Modifier.dragAndDropTarget(
            shouldStartDragAndDrop = { event ->
                val types = event.mimeTypes()
                val wanted = BernFiles.mimeTypes(kinds)
                types.any { type ->
                    wanted.any { it == "*/*" || it == type || (it.endsWith("/*") && type.startsWith(it.dropLast(1))) }
                }
            },
            target = target,
        ),
    ) { content() }
}

// MARK: - Scrolling that rests on items

/// A scroll view's items, where they are, and where it stands. Items
/// report their bounds; a fling comes to rest with the item nearest the
/// anchor (the start or the center) at it, and `position` follows the item
/// there.
class BernScrollState(val horizontal: Boolean, val center: Boolean) {
    val scroll = ScrollState(0)
    /// The visible area, in the window.
    var viewport by mutableStateOf(Rect.Zero)
    /// The scrolled content, which items are measured against.
    var content: LayoutCoordinates? = null
    /// Each item's bounds in the content, by key: they stay put while the
    /// view scrolls.
    val items = mutableStateMapOf<Any, Rect>()
    /// The largest item along the axis, which sets the margins that let the
    /// first and last item reach the center.
    var largest by mutableStateOf(0f)
    /// Set while a scroll to a set position is under way.
    var scrolling by mutableStateOf(false)
    /// Set once the view stands at its first position.
    var placed by mutableStateOf(false)
    /// The item a set position scrolled to, and how many arrivals there
    /// have been.
    var arrived by mutableStateOf<Any?>(null)
    var arrivals by mutableStateOf(0)

    private fun along(rect: Rect) = if (horizontal) rect.left else rect.top
    private fun size(rect: Rect) = if (horizontal) rect.width else rect.height
    private val length get() = if (horizontal) viewport.width else viewport.height

    fun report(key: Any, item: LayoutCoordinates) {
        val content = content?.takeIf { it.isAttached } ?: return
        if (!item.isAttached) return
        val bounds = Rect(content.localPositionOf(item, androidx.compose.ui.geometry.Offset.Zero), item.size.toSize())
        if (items[key] != bounds) items[key] = bounds
        val extent = size(bounds)
        if (extent > largest) largest = extent
    }

    /// Where an item lies in the content, along the axis.
    private fun start(bounds: Rect): Float = along(bounds)

    /// The scroll offset that puts `key`'s item at the anchor.
    fun offset(key: Any): Float? {
        val bounds = items[key] ?: return null
        val at = start(bounds) - if (center) (length - size(bounds)) / 2 else 0f
        return at.coerceIn(0f, scroll.maxValue.toFloat())
    }

    /// The item nearest the anchor when the view stands at `offset`.
    fun nearest(offset: Float = scroll.value.toFloat()): Any? = items.entries.minByOrNull { (_, bounds) ->
        val anchor = if (center) start(bounds) + size(bounds) / 2 - length / 2 else start(bounds)
        kotlin.math.abs(anchor - offset)
    }?.key

    fun arrive(key: Any) {
        arrived = key
        arrivals += 1
    }
}

val LocalBernScroll = compositionLocalOf<BernScrollState?> { null }

/// Where a layout is in the window, whole: `boundsInRoot` would clip it to
/// the scroll view, and an item out of view would have no size.
fun LayoutCoordinates.bernBounds(): Rect = Rect(positionInRoot(), size.toSize())

/// Comes to rest where the platform would have stopped, moved to the item
/// nearest the anchor there.
private class BernSnapFling(val state: BernScrollState, val decay: DecayAnimationSpec<Float>) : FlingBehavior {
    override suspend fun ScrollScope.performFling(initialVelocity: Float): Float {
        val start = state.scroll.value.toFloat()
        val projected = decay.calculateTargetValue(start, initialVelocity)
        val target = state.nearest(projected)?.let(state::offset) ?: projected.coerceIn(0f, state.scroll.maxValue.toFloat())
        var last = start
        animate(start, target, initialVelocity) { value, _ ->
            scrollBy(value - last)
            last = value
        }
        return 0f
    }
}

/// A scroll view whose items it rests on (`snap`) and whose `position`
/// follows the item at the anchor; setting `position` scrolls there with
/// `motion`, and with `arrival` the item gives a nudge once there.
@Composable
fun BernScroll(
    horizontal: Boolean,
    snap: String?,
    position: Any?,
    onPosition: ((Any) -> Unit)?,
    motion: BernMotion,
    arrival: BernMotion?,
    gap: Dp,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val state = remember(horizontal) { BernScrollState(horizontal, snap == "center") }
    val decay = rememberSplineBasedDecay<Float>()
    val fling = remember(state, decay) { BernSnapFling(state, decay) }
    val density = LocalDensity.current
    val reduce = bernReduceMotion()
    val current by rememberUpdatedState(position)
    // Stand at the first position once its item and the view are laid
    // out.
    LaunchedEffect(state) {
        val first = current
        if (first != null) {
            snapshotFlow {
                state.viewport.width > 0f && (state.items[first]?.width ?: 0f) > 0f &&
                    state.scroll.maxValue != Int.MAX_VALUE
            }.first { it }
            // The margins that center the first and last item follow the
            // measured items; let them apply before standing at the item.
            repeat(2) { androidx.compose.runtime.withFrameNanos { } }
            state.offset(first)?.let { state.scroll.scrollTo(it.toInt()) }
        }
        state.placed = true
    }
    // Setting the position scrolls to its item.
    LaunchedEffect(position) {
        val wanted = position ?: return@LaunchedEffect
        if (!state.placed || wanted == state.nearest()) return@LaunchedEffect
        val target = state.offset(wanted) ?: return@LaunchedEffect
        state.scrolling = true
        if (reduce) state.scroll.scrollTo(target.toInt()) else state.scroll.animateScrollTo(target.toInt(), motion.spec())
        state.scrolling = false
        if (arrival != null) state.arrive(wanted)
    }
    // The person's scroll: once it rests, the position follows the item
    // at the anchor.
    LaunchedEffect(state) {
        snapshotFlow { state.scroll.isScrollInProgress to state.placed }.collect { (moving, placed) ->
            if (placed && !moving && !state.scrolling) {
                state.nearest()?.let { nearest -> if (nearest != current) onPosition?.invoke(nearest) }
            }
        }
    }
    val margin = with(density) {
        if (state.center) ((if (horizontal) state.viewport.width else state.viewport.height) - state.largest).coerceAtLeast(0f).toDp() / 2 else 0.dp
    }
    CompositionLocalProvider(LocalBernScroll provides state, LocalBernArrival provides arrival) {
        Box(modifier.onGloballyPositioned { state.viewport = it.boundsInRoot() }) {
            val flinging = if (snap != null) fling else null
            if (horizontal) {
                Row(
                    Modifier
                        .then(if (flinging != null) Modifier.horizontalScroll(state.scroll, flingBehavior = flinging) else Modifier.horizontalScroll(state.scroll))
                        .onGloballyPositioned { state.content = it }
                        .padding(PaddingValues(horizontal = margin)),
                    horizontalArrangement = Arrangement.spacedBy(gap),
                    verticalAlignment = Alignment.CenterVertically,
                ) { CompositionLocalProvider(LocalBernFlex provides this) { content() } }
            } else {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .then(if (flinging != null) Modifier.verticalScroll(state.scroll, flingBehavior = flinging) else Modifier.verticalScroll(state.scroll))
                        .onGloballyPositioned { state.content = it }
                        .padding(PaddingValues(vertical = margin)),
                    verticalArrangement = Arrangement.spacedBy(gap),
                ) { CompositionLocalProvider(LocalBernFlex provides this) { content() } }
            }
        }
    }
}

val LocalBernArrival = compositionLocalOf<BernMotion?> { null }

/// A keyed item of a scroll view that rests on items or follows a
/// position: it reports where it is, and nudges when a set position
/// arrives at it.
@Composable
fun Modifier.bernScrollItem(key: Any): Modifier {
    val state = LocalBernScroll.current ?: return this
    val arrival = LocalBernArrival.current
    val scale = remember { Animatable(1f) }
    val reduce = bernReduceMotion()
    if (arrival != null) {
        LaunchedEffect(state.arrivals) {
            if (state.arrivals > 0 && state.arrived == key && !reduce) {
                scale.animateTo(1.06f, tween(80))
                scale.animateTo(1f, arrival.spec())
            }
        }
    }
    return onGloballyPositioned { state.report(key, it) }
        .graphicsLayer {
            scaleX = scale.value
            scaleY = scale.value
        }
}

/// Follows scrolling: `scale` at the center of the scroll view around it,
/// 1 at its edges, growing from the point `originX`, `originY` of the
/// element; `opacity` at its edges, 1 at the center. Drawn each frame
/// without composing again; still with reduced motion.
@Composable
fun Modifier.bernScrollEffect(scale: Float, opacity: Float, originX: Float = 0.5f, originY: Float = 0.5f): Modifier {
    val state = LocalBernScroll.current ?: return this
    if (bernReduceMotion()) return this
    var bounds by remember { mutableStateOf(Rect.Zero) }
    return onGloballyPositioned { bounds = it.bernBounds() }
        .graphicsLayer {
            val viewport = state.viewport
            val horizontal = state.horizontal
            val reach = (if (horizontal) viewport.width + bounds.width else viewport.height + bounds.height) / 2
            val distance = if (horizontal) bounds.center.x - viewport.center.x else bounds.center.y - viewport.center.y
            val near = if (reach > 0f) 1f - (kotlin.math.abs(distance) / reach).coerceIn(0f, 1f) else 1f
            val factor = 1f + (scale - 1f) * near
            transformOrigin = TransformOrigin(originX, originY)
            scaleX = factor
            scaleY = factor
            alpha = opacity + (1f - opacity) * near
        }
}

// MARK: - Nudges

/// A short nudge each time `on` turns true: to `size` of its size, then
/// back with `motion`; still with reduced motion.
@Composable
fun Modifier.bernNudge(on: Boolean, motion: BernMotion, size: Float = 1.06f): Modifier {
    val scale = remember { Animatable(1f) }
    val reduce = bernReduceMotion()
    var before by remember { mutableStateOf(on) }
    LaunchedEffect(on) {
        if (on && !before && !reduce) {
            scale.animateTo(size, tween(80))
            scale.animateTo(1f, motion.spec())
        }
        before = on
    }
    return graphicsLayer {
        scaleX = scale.value
        scaleY = scale.value
    }
}

/// A nudge each time `key` changes (`nudge_key`), as [bernNudge].
@Composable
fun Modifier.bernNudgeKey(key: Any?, motion: BernMotion, size: Float = 1.06f): Modifier {
    val scale = remember { Animatable(1f) }
    val reduce = bernReduceMotion()
    var before by remember { mutableStateOf(key) }
    LaunchedEffect(key) {
        if (key != before && !reduce) {
            scale.animateTo(size, tween(80))
            scale.animateTo(1f, motion.spec())
        }
        before = key
    }
    return graphicsLayer {
        scaleX = scale.value
        scaleY = scale.value
    }
}

// MARK: - Gestures bound to values

/// Pinching the content scales `value` within `min` and `max`; `commit`
/// runs when the fingers lift. Assistive technologies adjust it directly.
@Composable
fun BernPinch(
    label: String,
    value: Double,
    onValue: (Double) -> Unit,
    min: Double,
    max: Double,
    commit: () -> Unit,
    content: @Composable () -> Unit,
) {
    val current by rememberUpdatedState(value)
    Box(
        Modifier
            .semantics {
                contentDescription = label
                progressBarRangeInfo = ProgressBarRangeInfo(value.toFloat(), min.toFloat()..max.toFloat())
                setProgress { target ->
                    onValue(target.toDouble().coerceIn(min, max))
                    commit()
                    true
                }
            }
            .pointerInput(min, max) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    var start: Double? = null
                    var zoom = 1f
                    do {
                        val event = awaitPointerEvent()
                        if (event.changes.count { it.pressed } >= 2) {
                            if (start == null) start = current
                            zoom *= event.calculateZoom()
                            onValue((start * zoom).coerceIn(min, max))
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                    if (start != null) commit()
                }
            },
    ) { content() }
}

/// Dragging across the content moves `value` within `min` and `max`: up or
/// right raises it, and `distance` dp cross the whole range; `commit` runs
/// when the finger lifts.
@Composable
fun BernSwipe(
    label: String,
    value: Double,
    onValue: (Double) -> Unit,
    min: Double,
    max: Double,
    horizontal: Boolean,
    distance: Double,
    commit: () -> Unit,
    content: @Composable () -> Unit,
) {
    val current by rememberUpdatedState(value)
    var start by remember { mutableStateOf(0.0) }
    var moved by remember { mutableStateOf(0f) }
    val gestures = Modifier.pointerInput(min, max, horizontal, distance) {
        val full = distance.dp.toPx()
        val span = max - min
        val move = { amount: Float ->
            moved += amount
            val raised = if (horizontal) moved else -moved
            onValue((start + raised / full * span).coerceIn(min, max))
        }
        if (horizontal) {
            detectHorizontalDragGestures(
                onDragStart = { start = current; moved = 0f },
                onDragEnd = commit,
                onDragCancel = commit,
            ) { change, amount -> change.consume(); move(amount) }
        } else {
            detectVerticalDragGestures(
                onDragStart = { start = current; moved = 0f },
                onDragEnd = commit,
                onDragCancel = commit,
            ) { change, amount -> change.consume(); move(amount) }
        }
    }
    Box(
        Modifier
            .semantics {
                contentDescription = label
                progressBarRangeInfo = ProgressBarRangeInfo(value.toFloat(), min.toFloat()..max.toFloat())
                setProgress { target ->
                    onValue(target.toDouble().coerceIn(min, max))
                    commit()
                    true
                }
            }
            .then(gestures),
    ) { content() }
}

// MARK: - Rails

/// Where a child of a `Rail` sits: at the fraction `at` of its length, or
/// between the fractions `from` and `to`.
data class BernRailSpot(val at: Float?, val from: Float, val to: Float)

/// Which point of a child placed `at` a fraction stands on it.
enum class BernRailAnchor { Start, Center, End }

/// A strip along one axis that places each child at a fraction of its
/// length, or between two. It takes the length it is given; across, it is
/// as thick as the thickest child, or as its parent makes it, and a child
/// between two fractions without a size of its own across fills that
/// thickness. Children are centered across, may stand past the ends and
/// are not clipped; the rail itself takes no touches. With `overhang` the
/// scale runs that far past both ends.
@Composable
fun BernRail(vertical: Boolean, anchor: BernRailAnchor, modifier: Modifier = Modifier, overhang: Dp = 0.dp, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val past = overhang.roundToPx()
        // Sizes along the axis and across it, whichever the axis is.
        fun along(width: Int, height: Int) = if (vertical) height else width
        fun across(width: Int, height: Int) = if (vertical) width else height
        val maxAcross = if (vertical) constraints.maxWidth else constraints.maxHeight
        val minAcross = if (vertical) constraints.minWidth else constraints.minHeight
        val spots = measurables.map { it.layoutId as? BernRailSpot ?: BernRailSpot(0f, 0f, 0f) }
        // A child at a point keeps its own size.
        val loose = if (vertical) Constraints(maxWidth = maxAcross) else Constraints(maxHeight = maxAcross)
        val points = measurables.mapIndexed { index, measurable ->
            if (spots[index].at != null) measurable.measure(loose) else null
        }
        val length = when {
            vertical && constraints.hasBoundedHeight -> constraints.maxHeight
            !vertical && constraints.hasBoundedWidth -> constraints.maxWidth
            else -> points.maxOfOrNull { it?.let { placed -> along(placed.width, placed.height) } ?: 0 } ?: 0
        }
        fun fraction(value: Float) = value.coerceIn(0f, 1f)
        val scale = length + 2 * past
        val spans = spots.map { spot -> (abs(fraction(spot.to) - fraction(spot.from)) * scale).roundToInt() }
        var thickness = minAcross
        points.forEach { placed -> if (placed != null) thickness = maxOf(thickness, across(placed.width, placed.height)) }
        measurables.forEachIndexed { index, measurable ->
            if (points[index] == null) {
                val own = if (vertical) measurable.maxIntrinsicWidth(spans[index]) else measurable.maxIntrinsicHeight(spans[index])
                thickness = maxOf(thickness, own)
            }
        }
        thickness = minOf(thickness, maxAcross)
        val placeables = measurables.mapIndexed { index, measurable ->
            points[index] ?: measurable.measure(
                if (vertical) {
                    Constraints(minWidth = 0, maxWidth = thickness, minHeight = spans[index], maxHeight = spans[index])
                } else {
                    Constraints(minWidth = spans[index], maxWidth = spans[index], minHeight = 0, maxHeight = thickness)
                },
            )
        }
        val width = constraints.constrainWidth(if (vertical) thickness else length)
        val height = constraints.constrainHeight(if (vertical) length else thickness)
        layout(width, height) {
            placeables.forEachIndexed { index, placeable ->
                val spot = spots[index]
                val size = along(placeable.width, placeable.height)
                val start = if (spot.at != null) {
                    val shift = when (anchor) {
                        BernRailAnchor.Start -> 0
                        BernRailAnchor.Center -> size / 2
                        BernRailAnchor.End -> size
                    }
                    (fraction(spot.at) * scale).roundToInt() - past - shift
                } else {
                    (minOf(fraction(spot.from), fraction(spot.to)) * scale).roundToInt() - past
                }
                val cross = (thickness - across(placeable.width, placeable.height)) / 2
                if (vertical) placeable.placeRelative(cross, start) else placeable.placeRelative(start, cross)
            }
        }
    }
}

/// An element on the rail it stands in, at `at` or between `from` and
/// `to`; with `motion` it springs to a new place.
@Composable
fun BernPlace(at: Double?, from: Double?, to: Double?, motion: BernMotion?, content: @Composable () -> Unit) {
    val spot = if (at != null) {
        BernRailSpot(bernFraction(at, motion), 0f, 0f)
    } else {
        BernRailSpot(null, bernFraction(from ?: 0.0, motion), bernFraction(to ?: 0.0, motion))
    }
    Box(Modifier.layoutId(spot), propagateMinConstraints = true) { content() }
}

/// A fraction that follows `value`, with `motion`'s spring when given.
@Composable
private fun bernFraction(value: Double, motion: BernMotion?): Float {
    val target = value.toFloat()
    if (motion == null) return target
    return animateFloatAsState(target, motion.spec(), label = "fraction").value
}

// MARK: - Popover buttons

/// A button that shows `content` in a popover, such as a volume slider;
/// the content is composed only while the popover shows. With `presented`
/// the popover follows a session field and writes it back; external
/// entries open it with `activate`. With an icon only the icon shows; in a
/// top bar (`plain`) a label shows as a text button.
@Composable
fun BernPopoverButton(
    target: String,
    label: String,
    icon: ImageVector?,
    presented: Boolean?,
    onPresented: ((Boolean) -> Unit)?,
    shortcut: String?,
    plain: Boolean,
    content: @Composable ColumnScope.() -> Unit,
) {
    var own by remember { mutableStateOf(false) }
    val showing = presented ?: own
    val show: (Boolean) -> Unit = { value -> if (onPresented != null) onPresented(value) else own = value }
    BernControl(
        target, "popover_button", label,
        actions = BernControlActions(activate = { show(true) }, value = { if (showing) "open" else "closed" }),
    )
    if (shortcut != null) BernShortcut(shortcut) { show(true) }
    Box(Modifier.testTag(target)) {
        when {
            icon != null -> IconButton(onClick = { show(!showing) }) { Icon(icon, contentDescription = label) }
            plain -> TextButton(onClick = { show(!showing) }) { Text(label) }
            else -> FilledTonalButton(onClick = { show(!showing) }) { Text(label) }
        }
        DropdownMenu(expanded = showing, onDismissRequest = { show(false) }) {
            Column(
                Modifier.padding(16.dp).widthIn(min = 240.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                content = content,
            )
        }
    }
}

// MARK: - Where a scroll view opens

/// A scroll state that opens with the point at `at` of the content at the
/// same fraction of the view: once the first layout knows how far it can
/// scroll, it stands at `at` of that.
@Composable
fun rememberBernScroll(at: Float): ScrollState {
    val state = rememberScrollState()
    LaunchedEffect(Unit) {
        val most = snapshotFlow { state.maxValue }.first { it in 1 until Int.MAX_VALUE }
        state.scrollTo((at.coerceIn(0f, 1f) * most).roundToInt())
    }
    return state
}
