// Native Wear Material 3 window roots. Shared host/state/JNI remain in BernRuntime.
package dev.bern.runtime

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.input.pointer.*
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.*
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.wear.compose.foundation.LocalScreenIsActive
import androidx.wear.compose.foundation.hierarchicalFocusGroup
import androidx.wear.compose.foundation.lazy.*
import androidx.wear.compose.material3.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest

// Bern's headline roles map to Wear's display styles, not phone type sizes.
internal val Typography.headlineLarge: TextStyle get() = displayLarge
internal val Typography.headlineSmall: TextStyle get() = displaySmall

@Composable
fun bernSurfaceVariant(): Color = MaterialTheme.colorScheme.surfaceContainerHigh

@Composable
fun BernScheme(dark: Boolean, content: @Composable () -> Unit) {
    val context = LocalContext.current
    val configuration = LocalConfiguration.current
    val accent = BernThemeState.accent
    val colors = remember(configuration, dark, accent) {
        val base = dynamicColorScheme(context) ?: ColorScheme()
        val appearance = if (dark) base else base.copy(
            background = Color(0xFFFAFAFA), onBackground = Color(0xFF171717),
            surfaceContainerLow = Color(0xFFF2F2F2), surfaceContainer = Color(0xFFE9E9E9),
            surfaceContainerHigh = Color(0xFFDDDDDD), onSurface = Color(0xFF171717),
            onSurfaceVariant = Color(0xFF454545), outline = Color(0xFF777777), outlineVariant = Color(0xFFBBBBBB),
        )
        if (accent == null) appearance else {
            val color = BernPalette(appearance, dark).color(accent)
            val on = if (color.luminance() > 0.179f) Color.Black else Color.White
            val surface = appearance.surfaceContainer
            fun mix(amount: Float) = Color(
                red = surface.red + (color.red - surface.red) * amount,
                green = surface.green + (color.green - surface.green) * amount,
                blue = surface.blue + (color.blue - surface.blue) * amount,
            )
            val container = mix(if (dark) 0.25f else 0.16f)
            val onContainer = appearance.onSurface
            appearance.copy(primary = color, primaryDim = color.copy(alpha = 0.8f), onPrimary = on,
                primaryContainer = container, onPrimaryContainer = onContainer,
                secondary = color, secondaryDim = color.copy(alpha = 0.8f), onSecondary = on,
                secondaryContainer = mix(if (dark) 0.18f else 0.12f), onSecondaryContainer = onContainer,
                tertiary = color, tertiaryDim = color.copy(alpha = 0.8f), onTertiary = on,
                tertiaryContainer = mix(if (dark) 0.14f else 0.09f), onTertiaryContainer = onContainer)
        }
    }
    CompositionLocalProvider(LocalBernDark provides dark) { MaterialTheme(colorScheme = colors, content = content) }
}

@Composable
fun BernTheme(content: @Composable () -> Unit) {
    // Wear's system theme is dark. A deliberately requested light appearance
    // is still honored; it does not change the system watch face or settings.
    val context = LocalContext.current
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) BernMotionState.readSystem(context)
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
    BernScheme(BernThemeState.appearance?.lowercase() != "light", content)
}

@Composable
fun BernVirtualHost(window: @Composable (String) -> Unit) {
    AppScaffold(timeText = { TimeText() }, modifier = Modifier.semantics { testTagsAsResourceId = true }) {
        if (BernHost.primaryOpen) window(BernHost.primary) else {
            BernLazyList {
                item {
                    val title = BernHost.kinds.firstOrNull { it.id == BernHost.primary }?.let { BernText.string(it.title) } ?: ""
                    Button(onClick = { BernHost.open(BernHost.primary) }) {
                        Text(BernText.format("bern.window.open", listOf(title)))
                    }
                }
            }
        }
        for (kind in BernHost.sheets) {
            androidx.compose.runtime.key(kind) {
                // A window may veto closing or pop an inner route. Unlike a
                // dismissible Dialog, its SwipeToDismissBox can remain mounted.
                Box(Modifier.fillMaxSize()) { window(kind) }
            }
        }
    }
}

@Composable
fun BernWindowRoot(
    context: BernWindowContext,
    idleAfter: Double? = null,
    immersive: Boolean = false,
    idleEnabled: Boolean = true,
    landscape: Boolean? = null,
    unified: Boolean = false,
    content: @Composable () -> Unit,
) {
    // Watches keep the device's orientation; this property is not applicable,
    // as on a TV. Never crash when a shared declaration requests landscape.
    val onTop = (BernHost.sheets.lastOrNull() ?: BernHost.primary) == context.kindID
    var input by remember { mutableLongStateOf(0L) }
    if (idleAfter != null) LaunchedEffect(input, BernShortcuts.keys, idleEnabled) {
        context.idle = false
        if (idleEnabled) snapshotFlow { context.idleWait(idleAfter) }.collectLatest { wait ->
            context.idle = false
            if (wait != null) { delay((wait * 1000).toLong()); context.idle = true }
        }
    }
    DisposableEffect(context) {
        BernHost.attach(context)
        onDispose { BernHost.detach(context) }
    }
    val settled = remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { withFrameNanos {}; settled.value = true }
    val density = LocalDensity.current
    context.estimatedWidth = LocalConfiguration.current.screenWidthDp.toDouble()
    context.estimatedHeight = LocalConfiguration.current.screenHeightDp.toDouble()
    val editing = remember(context) { BernEditingRegions() }
    val pointer = remember(context) { BernPointer() }
    val safe = remember(context) { BernSafeArea() }
    val measured = Modifier.fillMaxSize().hierarchicalFocusGroup(onTop).background(MaterialTheme.colorScheme.background)
        .bernEditingPresses(editing).bernPointerWatch(pointer).onSizeChanged {
            context.measuredWidth = with(density) { it.width.toDp().value.toDouble() }
            context.measuredHeight = with(density) { it.height.toDp().value.toDouble() }
        }.pointerInput(context) {
            awaitPointerEventScope {
                var last: Offset? = null
                while (true) {
                    val event = awaitPointerEvent(PointerEventPass.Initial)
                    val change = event.changes.firstOrNull() ?: continue
                    context.mouse = change.type == PointerType.Mouse
                    if (event.type != PointerEventType.Move || change.position != last) input++
                    last = change.position
                }
            }
        }
    CompositionLocalProvider(LocalBernWindow provides context, LocalBernSettled provides settled,
        LocalBernEditingRegions provides editing, LocalBernPointer provides pointer, LocalBernSafeArea provides safe,
        LocalScreenIsActive provides onTop) {
        val back = context.chrome.lastOrNull { it.back != null }?.back
        val goBack = { if (back != null) back() else BernHost.close(context.kindID) }
        BackHandler(enabled = onTop && context.pendingClose == null) { goBack() }
        SwipeToDismissBox(onDismissed = goBack,
            userSwipeEnabled = onTop && context.pendingClose == null,
            modifier = measured) { background ->
            if (!background) {
                ScreenScaffold(contentPadding = PaddingValues(), timeText = if (immersive) ({}) else null) { padding ->
                    Box(Modifier.fillMaxSize().onPlaced { safe.window = it; safe.safe = it }) {
                        CompositionLocalProvider(LocalBernBarsPadding provides padding) {
                            val error = BernHost.startupError
                            if (error != null) BernLazyList { item { Text(BernText.string("bern.startup.failed")) }; item { Text(error) } }
                            else BernHeroRoot(content)
                        }
                    }
                }
                // Toolbar actions are still reachable, in a watch dialog rather
                // than a phone app bar taking the top third of the screen.
                if (context.chrome.any { it.leading != null || it.actions != null }) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                        BernMenu(context.chrome.lastOrNull { it.title != null }?.title ?: context.title, BernIcons.get("ellipsis"), iconOnly = true) { dismiss ->
                            context.chrome.forEach { entry ->
                                Row { entry.leading?.invoke(this); entry.actions?.invoke(this) }
                            }
                        }
                    }
                }
            }
        }
        context.pendingClose?.let { pending ->
            fun decide(choice: Int) = BernHost.confirmClose(context, pending.id, choice) { _, problem ->
                if (problem != null) BernHost.failure = BernFailure(context.id, problem.code, problem.message)
            }
            AlertDialog(visible = true, onDismissRequest = { decide(2) }, title = { Text(pending.message) }) {
                if (pending.canSave) item { Button(onClick = { decide(0) }, modifier = Modifier.fillMaxWidth()) { Text(BernText.string("bern.close.save")) } }
                item { FilledTonalButton(onClick = { decide(1) }, modifier = Modifier.fillMaxWidth()) {
                    Text(BernText.string(if (pending.canSave) "bern.close.dont_save" else "bern.close.discard"))
                } }
                item { ChildButton(onClick = { decide(2) }, modifier = Modifier.fillMaxWidth()) { Text(BernText.string("bern.close.cancel")) } }
            }
        }
        BernHost.failure?.takeIf { onTop && (it.window == null || it.window == context.id) }?.let { failure ->
            AlertDialog(visible = true, onDismissRequest = { BernHost.failure = null },
                title = { Text(BernText.string("bern.alert.failed")) },
                text = { Text(failure.message) }) {
                item {
                    Button(onClick = { BernHost.failure = null }, modifier = Modifier.fillMaxWidth()) {
                        Text(BernText.string("bern.alert.ok"))
                    }
                }
            }
        }
    }
}

@Composable
fun BernLazyList(
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(horizontal = 14.dp, vertical = 24.dp),
    content: ScalingLazyListScope.() -> Unit,
) {
    val state = rememberScalingLazyListState(initialCenterItemIndex = 0)
    ScreenScaffold(scrollState = state, contentPadding = contentPadding,
        modifier = modifier.fillMaxSize()) { padding ->
        ScalingLazyColumn(state = state, modifier = Modifier.fillMaxSize(), contentPadding = padding,
            rotaryScrollableBehavior = androidx.wear.compose.foundation.rotary.RotaryScrollableDefaults.behavior(state),
            content = content)
    }
}
