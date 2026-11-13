// Full-screen watch panels and single-screen navigation. No phone app bars.
package dev.bern.runtime

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import androidx.wear.compose.foundation.requestFocusOnHierarchyActive
import androidx.wear.compose.foundation.rotary.RotaryScrollableDefaults
import androidx.wear.compose.foundation.rotary.rotaryScrollable
import androidx.wear.compose.material3.*
import kotlinx.coroutines.delay

@Composable
internal fun BernWearColumn(modifier: Modifier = Modifier, gap: Dp = 8.dp,
    state: ScrollState = rememberScrollState(), content: @Composable ColumnScope.() -> Unit) {
    val focus = remember { FocusRequester() }
    ScreenScaffold(scrollState = state, contentPadding = PaddingValues(horizontal = 18.dp, vertical = 30.dp),
        modifier = modifier.fillMaxSize()) { padding ->
        Column(Modifier.fillMaxSize()
            .requestFocusOnHierarchyActive()
            .rotaryScrollable(RotaryScrollableDefaults.behavior(state), focus)
            .verticalScroll(state).padding(padding),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(gap), content = content)
    }
}

@Composable
fun BernScrollColumn(modifier: Modifier, state: ScrollState, content: @Composable ColumnScope.() -> Unit) {
    val viewport = remember { BernViewport() }
    val room = remember { BernScrollRoom() }
    val focus = remember { FocusRequester() }
    CompositionLocalProvider(LocalBernViewport provides viewport, LocalBernScrollRoom provides room) {
        ScreenScaffold(scrollState = state, modifier = modifier) { padding ->
            Column(Modifier.fillMaxSize().onGloballyPositioned { viewport.bounds = it.boundsInRoot() }
                .bernScrollRoom(room, state)
                .requestFocusOnHierarchyActive()
                .rotaryScrollable(RotaryScrollableDefaults.behavior(state), focus)
                .bernVerticalScroll(state).bernScrollRoomContent(room).padding(padding), content = content)
        }
    }
}

@Composable
internal fun BernWearDialog(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    Dialog(visible = true, onDismissRequest = onDismiss) {
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
            BernPopupContent { BernWearColumn(content = content) }
        }
    }
}

@Composable
fun BernForm(gap: Dp?, rows: BernRows, content: @Composable ColumnScope.() -> Unit) {
    CompositionLocalProvider(LocalBernFormRows provides rows) { BernWearColumn(gap = gap ?: 12.dp, content = content) }
}

@Composable
fun BernModal(width: Dp, height: Dp, cover: Boolean, enter: EnterTransition?, exit: ExitTransition?,
    content: @Composable () -> Unit) {
    // Modal has no dismiss binding: like the phone implementation, its own
    // buttons dismiss it. The window close protocol must not be bypassed.
    // Wear Dialog always installs swipe-to-dismiss and requires removal after
    // dismissal. A non-dismissible Bern Modal therefore uses the underlying
    // system dialog window, still containing Wear controls and full-screen UI.
    androidx.compose.ui.window.Dialog(onDismissRequest = {},
        properties = DialogProperties(dismissOnBackPress = false, dismissOnClickOutside = false,
            usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        BernPopupContent {
            val shown = remember { androidx.compose.animation.core.MutableTransitionState(false).apply { targetState = true } }
            androidx.compose.animation.AnimatedVisibility(visibleState = shown,
                enter = enter ?: EnterTransition.None, exit = exit ?: ExitTransition.None) {
                Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) { content() }
            }
        }
    }
}

@Composable
fun BernControlGroup(label: String?, height: Dp?, content: @Composable RowScope.() -> Unit) {
    Row(Modifier.then(height?.let { Modifier.height(it) } ?: Modifier)
        .semantics { if (label != null) contentDescription = label },
        horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically, content = content)
}

@Composable
fun BernMenu(label: String, icon: ImageVector?, modifier: Modifier = Modifier, iconOnly: Boolean = false,
    shape: BernShape? = null, items: @Composable (dismiss: () -> Unit) -> Unit) {
    var open by remember { mutableStateOf(false) }
    if (icon != null && iconOnly) BernIconButton(label, icon, true, modifier, diameter = shape?.height) { open = true }
    else BernButton(label, icon, BernProminence.Standard, modifier = modifier, shape = shape) { open = true }
    if (open) BernWearDialog(onDismiss = { open = false }) { items { open = false } }
}

@Composable
fun BernMenuItem(label: String, icon: ImageVector?, enabled: Boolean, destructive: Boolean, onClick: () -> Unit) =
    BernButton(label, icon, BernProminence.Plain, destructive = destructive, enabled = enabled, fill = true, onClick = onClick)

@Composable
fun BernContextMenu(menu: @Composable (dismiss: () -> Unit) -> Unit, content: @Composable () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.bernMenuPress { open = true }) { content() }
    if (open) BernWearDialog(onDismiss = { open = false }) { menu { open = false } }
}

@Composable
fun BernInspector(presented: Boolean, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    if (presented) BernWearDialog(onDismiss, content)
}

@Composable
fun BernToast(text: String, icon: ImageVector?, key: Any, duration: Double, motion: BernMotion) {
    var shown by remember { mutableStateOf(true) }
    LaunchedEffect(key) { shown = true; delay((duration * 1000).toLong().coerceAtLeast(0)); shown = false }
    if (shown) Dialog(visible = true, onDismissRequest = { shown = false }) {
        Column(Modifier.fillMaxSize().padding(24.dp).semantics { liveRegion = LiveRegionMode.Polite },
            horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            icon?.let { Icon(it, null) }
            Text(text)
        }
    }
}

@Composable
fun BernSplitButton(target: String, label: String, icon: ImageVector?, more: String, checked: Boolean,
    onChange: (Boolean) -> Unit, shape: BernShape? = null, content: @Composable ColumnScope.() -> Unit) {
    var open by remember { mutableStateOf(false) }
    BernControl(target, "split_button", label, actions = BernControlActions(
        activate = { open = true }, setBool = onChange, value = { checked.toString() }))
    SplitSwitchButton(checked = checked, onCheckedChange = onChange, toggleContentDescription = label,
        onContainerClick = { open = true }, containerClickLabel = more,
        modifier = Modifier.testTag(target).then(shape?.height?.let { Modifier.height(it) } ?: Modifier)) {
        if (icon != null) Icon(icon, null, Modifier.size(20.dp))
        Text(label)
    }
    if (open) BernWearDialog(onDismiss = { open = false }, content = content)
}

@Composable
fun BernPopoverButton(target: String, label: String, icon: ImageVector?, presented: Boolean?,
    onPresented: ((Boolean) -> Unit)?, shortcut: String?, plain: Boolean, windowTheme: Boolean = false,
    style: BernStyle? = null, iconSize: Float? = null, content: @Composable ColumnScope.() -> Unit) {
    var own by remember { mutableStateOf(false) }
    val showing = presented ?: own
    val show: (Boolean) -> Unit = { if (onPresented != null) onPresented(it) else own = it }
    BernControl(target, "popover_button", label, actions = BernControlActions(
        activate = { show(true) }, value = { if (showing) "open" else "closed" }))
    if (shortcut != null) BernShortcut(shortcut) { show(true) }
    if (icon != null) BernIconButton(label, icon, true, Modifier.testTag(target), style, iconSize) { show(true) }
    else BernButton(label, null, if (plain) BernProminence.Plain else BernProminence.Standard,
        style = style, modifier = Modifier.testTag(target)) { show(true) }
    val dark = if (windowTheme) BernThemeState.appearance?.lowercase() != "light" else bernDark()
    if (showing) BernAppearance(dark) {
        BernWearDialog(onDismiss = { show(false) }) {
            val palette = bernPalette()
            val foreground = style?.base?.foreground?.let(palette::color) ?: LocalContentColor.current
            CompositionLocalProvider(LocalContentColor provides foreground) {
                Column(Modifier.fillMaxWidth().background(style?.base?.background?.let(palette::color) ?: Color.Transparent), content = content)
            }
        }
    }
}

@Composable
fun BernNavLink(onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    FilledTonalButton(onClick = onClick, modifier = modifier.fillMaxWidth()) { Column(content = content) }
}
val LocalBernRail = compositionLocalOf { false }
val LocalBernFloatingTabs = compositionLocalOf { false }
val LocalBernBarTop = compositionLocalOf<State<Float>?> { null }

@Composable
fun BernSplitView(title: String, showingDetail: MutableState<Boolean>, sidebarWidth: Dp = 280.dp,
    header: (@Composable () -> Unit)? = null, sidebar: @Composable (showDetail: () -> Unit) -> Unit,
    footer: (@Composable ColumnScope.() -> Unit)? = null, detail: @Composable () -> Unit) {
    if (showingDetail.value) {
        BernChrome(title = title, back = { showingDetail.value = false })
        detail()
    } else BernWearColumn {
        header?.invoke()
        sidebar { showingDetail.value = true }
        footer?.invoke(this)
    }
}

@Composable
fun BernSidebarSection(title: String?, content: @Composable () -> Unit) {
    if (title != null) Text(title, style = MaterialTheme.typography.labelMedium, modifier = Modifier.semantics { heading() })
    content()
}
@Composable
fun BernSidebarItem(label: String, icon: ImageVector?, selected: Boolean, badge: Long? = null, onClick: () -> Unit) {
    RadioButton(selected = selected, onSelect = onClick, modifier = Modifier.fillMaxWidth(),
        icon = icon?.let { { Icon(it, null) } },
        secondaryLabel = badge?.takeIf { it > 0 }?.let { { Text(it.toString()) } }) { Text(label) }
}

private val LocalWearChooseTab = compositionLocalOf<(() -> Unit)?> { null }

@Composable
fun BernTabs(floating: Boolean = false, content: @Composable () -> Unit,
    accessory: (@Composable () -> Unit)? = null, bar: @Composable ColumnScope.() -> Unit) {
    var choosing by remember { mutableStateOf(false) }
    val label = LocalBernWindow.current.title
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            Box(Modifier.weight(1f)) { content() }
            accessory?.invoke()
            FilledTonalIconButton(onClick = { choosing = true },
                modifier = Modifier.align(Alignment.CenterHorizontally).padding(bottom = 4.dp).size(36.dp)) {
                Icon(BernIcons.get("list.bullet"), label, modifier = Modifier.size(18.dp))
            }
        }
    }
    if (choosing) BernWearDialog(onDismiss = { choosing = false }) {
        CompositionLocalProvider(LocalWearChooseTab provides { choosing = false }) { bar() }
    }
}
@Composable
fun BernTabItem(label: String, icon: ImageVector?, selected: Boolean, iconOnly: Boolean = false, onClick: () -> Unit) {
    val dismiss = LocalWearChooseTab.current
    BernSidebarItem(label, icon, selected) { onClick(); dismiss?.invoke() }
}
@Composable
fun BernTabAccessory(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 2.dp), contentAlignment = Alignment.Center) { content() }
}

@Composable
fun BernToolbarSearch(target: String, text: String, onText: (String) -> Unit, prompt: String,
    submit: (() -> Unit)?, results: @Composable ColumnScope.() -> Unit) =
    BernSearchPalette(target, text, onText, prompt, prompt, Dp.Unspecified, Dp.Unspecified, submit, results)

@Composable
fun BernSearchPalette(target: String, text: String, onText: (String) -> Unit, prompt: String,
    panelPrompt: String, width: Dp, height: Dp, submit: (() -> Unit)?, results: @Composable ColumnScope.() -> Unit) {
    var open by remember { mutableStateOf(false) }
    val opener = remember { { open = true } }
    DisposableEffect(target) {
        BernSearchPalettes.register(target, opener)
        onDispose { BernSearchPalettes.remove(target, opener) }
    }
    BernControl(target, "search", prompt, actions = BernControlActions(activate = opener, focus = opener,
        edit = onText, submit = submit, value = { if (open) "open: $text" else text }))
    BernIconButton(prompt, BernIcons.get("magnifyingglass"), true, Modifier.testTag(target), onClick = opener)
    if (open) BernWearDialog(onDismiss = { open = false }) {
        val state = remember { BernPaletteState { open = false } }
        CompositionLocalProvider(LocalBernSearchPalette provides state) {
            BernSearchField(text, onText, panelPrompt, submit)
            results()
        }
    }
}

@Composable
fun BernSearchItem(target: String, label: String, icon: ImageVector?, detail: String?, hint: String?,
    navigation: Boolean, actions: BernControlActions, content: (@Composable () -> Unit)? = null) {
    val palette = LocalBernSearchPalette.current
    val choose = { palette?.dismiss(); actions.activate?.invoke(); Unit }
    BernControl(target, "button", label, actions = BernControlActions(activate = choose,
        value = actions.value, setFile = actions.setFile?.let { give -> { url -> palette?.dismiss(); give(url) } },
        fileKinds = actions.fileKinds))
    FilledTonalButton(onClick = choose, modifier = Modifier.testTag(target).fillMaxWidth()) {
        if (content != null) content() else {
            icon?.let { Icon(it, null, Modifier.size(20.dp)) }
            Column {
                Text(label)
                if (detail != null) Text(detail, style = MaterialTheme.typography.bodySmall)
                if (hint != null) Text(hint, style = MaterialTheme.typography.labelSmall)
            }
        }
    }
}
