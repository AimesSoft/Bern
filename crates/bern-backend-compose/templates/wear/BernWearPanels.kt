// Full-screen watch panels and single-screen navigation. No phone app bars.
package dev.bern.runtime

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.layout.ContentScale
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
        ScreenScaffold(scrollState = state, contentPadding = PaddingValues(), modifier = modifier) { padding ->
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
fun BernMenuItem(
    label: String,
    icon: ImageVector?,
    enabled: Boolean,
    destructive: Boolean,
    checked: Boolean? = null,
    onClick: () -> Unit,
) = BernButton(
    label,
    icon ?: if (checked == true) BernIcons.get("checkmark") else null,
    BernProminence.Plain,
    destructive = destructive,
    enabled = enabled,
    fill = true,
    onClick = onClick,
)

@Composable
fun BernContextMenu(menu: @Composable (dismiss: () -> Unit) -> Unit, content: @Composable () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.bernMenuPress { open = true }) { content() }
    if (open) BernWearDialog(onDismiss = { open = false }) { menu { open = false } }
}

/// The pointer events of an element that is not a button: a double tap, and
/// the `ContextMenu` a long press opens. Single taps still reach the element.
@Composable
fun BernPointerEvents(
    modifier: Modifier = Modifier,
    onDoubleTap: (() -> Unit)? = null,
    onWheelUp: (() -> Unit)? = null,
    onWheelDown: (() -> Unit)? = null,
    menu: (@Composable (dismiss: () -> Unit) -> Unit)? = null,
    content: @Composable () -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    val openMenu = { open = true }
    BernPointerBox(modifier, onDoubleTap, if (menu != null) openMenu else null, onWheelUp, onWheelDown, content)
    if (open && menu != null) BernWearDialog(onDismiss = { open = false }) { menu { open = false } }
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
    style: BernStyle? = null, iconSize: Float? = null, shape: BernShape? = null, content: @Composable ColumnScope.() -> Unit) {
    var own by remember { mutableStateOf(false) }
    val showing = presented ?: own
    val show: (Boolean) -> Unit = { if (onPresented != null) onPresented(it) else own = it }
    BernControl(target, "popover_button", label, actions = BernControlActions(
        activate = { show(true) }, value = { if (showing) "open" else "closed" }))
    if (shortcut != null) BernShortcut(shortcut) { show(true) }
    if (icon != null) BernIconButton(label, icon, true, Modifier.testTag(target), style, iconSize, diameter = shape?.height,
        prominence = if (plain) BernProminence.Plain else BernProminence.Standard) { show(true) }
    else BernButton(label, null, if (plain) BernProminence.Plain else BernProminence.Standard,
        style = style, shape = shape, modifier = Modifier.testTag(target)) { show(true) }
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
    sidebarStyle: BernStyle? = null, detailStyle: BernStyle? = null,
    collapsed: Boolean = false, onCollapsed: ((Boolean) -> Unit)? = null,
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

@Composable
fun BernTooltip(content: String, child: @Composable () -> Unit) {
    child()
}

@Composable
fun BernBanner(
    severity: String,
    title: String?,
    content: String,
    icon: ImageVector?,
    dismissLabel: String?,
    dismissTarget: String?,
    onDismiss: (() -> Unit)?,
    actions: @Composable RowScope.() -> Unit,
) {
    if (dismissTarget != null && onDismiss != null) {
        BernControl(dismissTarget, "button", dismissLabel ?: content, actions = BernControlActions(activate = onDismiss))
    }
    Column(Modifier.fillMaxWidth().padding(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (!title.isNullOrEmpty()) Text(title, style = MaterialTheme.typography.titleSmall)
        Text(content, style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), content = actions)
        if (onDismiss != null) {
            TextButton(onClick = onDismiss) { Text(dismissLabel ?: "Dismiss") }
        }
    }
}

@Composable
fun BernDisclosure(
    target: String,
    label: String,
    icon: ImageVector?,
    externalExpanded: Boolean?,
    onToggle: ((Boolean) -> Unit)?,
    content: @Composable ColumnScope.() -> Unit,
) {
    var localExpanded by remember { mutableStateOf(false) }
    val expanded = externalExpanded ?: localExpanded
    val setExpanded: (Boolean) -> Unit = { next ->
        if (externalExpanded != null) onToggle?.invoke(next)
        else { localExpanded = next; onToggle?.invoke(next) }
    }
    BernControl(target, "disclosure", label, actions = BernControlActions(
        activate = { setExpanded(!expanded) }, setBool = { setExpanded(it) },
        value = { if (expanded) "true" else "false" }))
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        FilledTonalButton(onClick = { setExpanded(!expanded) }, modifier = Modifier.fillMaxWidth()) {
            icon?.let { Icon(it, null, Modifier.size(18.dp)) }
            Text(label)
        }
        if (expanded) content()
    }
}

@Composable
fun BernNumberInput(
    target: String,
    label: String,
    value: Double,
    onCommit: (Double) -> Unit,
    min: Double?,
    max: Double?,
    step: Double,
    precision: Int,
    enabled: Boolean,
    labelHidden: Boolean,
    modifier: Modifier = Modifier,
) {
    val clampedPrecision = precision.coerceIn(0, 6)
    fun clampRound(raw: Double): Double {
        if (!raw.isFinite()) return value
        var v = raw
        if (min != null) v = maxOf(v, min)
        if (max != null) v = minOf(v, max)
        val factor = Math.pow(10.0, clampedPrecision.toDouble())
        return Math.round(v * factor) / factor
    }
    BernControl(target, "number_input", label, enabled = enabled, actions = BernControlActions(
        edit = { it.trim().toDoubleOrNull()?.let { v -> if (v.isFinite()) onCommit(clampRound(v)) } },
        setNumber = { v -> if (v.isFinite()) onCommit(clampRound(v)) },
        value = { BernText.text(value) }))
    Column(modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilledTonalButton(onClick = { onCommit(clampRound(value - step)) }, enabled = enabled) { Text("-") }
            Text(if (clampedPrecision == 0) Math.round(value).toString() else String.format(java.util.Locale.US, "%.${clampedPrecision}f", value))
            FilledTonalButton(onClick = { onCommit(clampRound(value + step)) }, enabled = enabled) { Text("+") }
        }
    }
}

@Composable
fun BernAlert(
    title: String,
    message: String?,
    actions: List<BernAlertActionSpec>,
    onDismiss: (() -> Unit)?,
) {
    for (act in actions) {
        BernControl(act.target, "button", act.label, enabled = act.enabled, actions = BernControlActions(activate = act.onClick))
    }
    val cancel = actions.firstOrNull { it.role == "cancel" }
    BernWearDialog(onDismiss = {
        if (onDismiss != null) onDismiss()
        else if (cancel != null && cancel.enabled) cancel.onClick()
    }) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        if (!message.isNullOrEmpty()) Text(message, style = MaterialTheme.typography.bodySmall)
        for (act in actions) {
            BernButton(act.label, null, BernProminence.Standard, destructive = act.role == "destructive", enabled = act.enabled, fill = true, onClick = act.onClick)
        }
    }
}

@Composable
fun BernSelectable(content: @Composable () -> Unit) {
    content()
}

@Composable
fun BernDatePicker(
    target: String,
    label: String,
    year: Long,
    month: Long,
    day: Long,
    onCommit: (Long, Long, Long) -> Unit,
    minDate: Triple<Long, Long, Long>? = null,
    maxDate: Triple<Long, Long, Long>? = null,
    enabled: Boolean = true,
    labelHidden: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val iso = String.format(java.util.Locale.US, "%04d-%02d-%02d", year.coerceIn(1L, 9999L), month.coerceIn(1L, 12L), day.coerceIn(1L, 31L))
    fun stepDay(delta: Int) {
        val cal = java.util.Calendar.getInstance(java.util.TimeZone.getTimeZone("UTC"))
        cal.clear()
        cal.set(year.coerceIn(1L, 9999L).toInt(), (month.coerceIn(1L, 12L) - 1L).toInt(), day.coerceIn(1L, 31L).toInt())
        cal.add(java.util.Calendar.DAY_OF_MONTH, delta)
        onCommit(cal.get(java.util.Calendar.YEAR).toLong(), (cal.get(java.util.Calendar.MONTH) + 1).toLong(), cal.get(java.util.Calendar.DAY_OF_MONTH).toLong())
    }
    BernControl(
        target,
        "date_picker",
        label,
        enabled = enabled,
        actions = BernControlActions(
            edit = { text ->
                val parts = text.trim().split("-")
                if (parts.size == 3) {
                    val y = parts[0].toLongOrNull()
                    val m = parts[1].toLongOrNull()
                    val d = parts[2].toLongOrNull()
                    if (y != null && m != null && d != null) onCommit(y, m, d)
                }
            },
            value = { iso },
        ),
    )
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilledTonalButton(onClick = { stepDay(-1) }, enabled = enabled) { Text("-") }
            Text(iso, style = MaterialTheme.typography.bodySmall)
            FilledTonalButton(onClick = { stepDay(1) }, enabled = enabled) { Text("+") }
        }
    }
}

@Composable
fun BernTimePicker(
    target: String,
    label: String,
    minutes: Double,
    onCommit: (Double) -> Unit,
    minMinutes: Double = 0.0,
    maxMinutes: Double = 1439.0,
    step: Double = 1.0,
    enabled: Boolean = true,
    labelHidden: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val lo = minMinutes.coerceIn(0.0, 1439.0)
    val hi = maxMinutes.coerceIn(lo, 1439.0)
    val st = kotlin.math.round(step).coerceIn(1.0, 60.0)
    val total = minutes.coerceIn(lo, hi).toInt()
    val formatted = String.format(java.util.Locale.US, "%02d:%02d", total / 60, total % 60)
    BernControl(
        target,
        "time_picker",
        label,
        enabled = enabled,
        actions = BernControlActions(
            setNumber = { v -> if (v.isFinite()) onCommit(v.coerceIn(lo, hi)) },
            value = { formatted },
        ),
    )
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilledTonalButton(onClick = { onCommit((minutes - st).coerceIn(lo, hi)) }, enabled = enabled) { Text("-") }
            Text(formatted, style = MaterialTheme.typography.bodySmall)
            FilledTonalButton(onClick = { onCommit((minutes + st).coerceIn(lo, hi)) }, enabled = enabled) { Text("+") }
        }
    }
}

@Composable
fun BernEmptyState(
    title: String,
    message: String?,
    icon: ImageVector?,
    actions: @Composable RowScope.() -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (icon != null) Icon(icon, contentDescription = null, modifier = Modifier.size(28.dp))
        Text(title, style = MaterialTheme.typography.titleSmall)
        if (!message.isNullOrEmpty()) Text(message, style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) { actions() }
    }
}

@Composable
fun BernBadge(
    content: String,
    tone: String,
    icon: ImageVector?,
) {
    val tint = when (tone) {
        "accent" -> bernPalette().role("accent")
        "success" -> bernPalette().role("status.success")
        "warning" -> bernPalette().role("status.warning")
        "error" -> bernPalette().role("status.error")
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    Row(
        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(10.dp))
        Text(content, style = MaterialTheme.typography.labelSmall, color = tint, maxLines = 1)
    }
}

@Composable
fun BernAvatar(
    label: String,
    name: String?,
    url: String?,
    size: Dp,
    rounded: Boolean,
) {
    val trimmed = label.trim()
    val initials = if (trimmed.isEmpty()) "?" else trimmed.substring(0, 1).uppercase()
    Box(
        modifier = Modifier.size(size),
        contentAlignment = Alignment.Center,
    ) {
        Text(initials, style = MaterialTheme.typography.labelMedium)
        if (!name.isNullOrEmpty()) {
            BernImage(name, null, size, size, ContentScale.Crop)
        } else if (!url.isNullOrEmpty()) {
            BernImageURL(url, null, size, size, ContentScale.Crop)
        }
    }
}

data class BernTreeEntry<T, K>(val item: T, val key: K, val parent: K?)

@Composable
fun <T, K> BernTreeView(
    target: String,
    label: String,
    entries: List<BernTreeEntry<T, K>>,
    selectedKey: K?,
    onSelect: ((K) -> Unit)?,
    externalExpanded: List<K>?,
    onExpandedChange: ((List<K>) -> Unit)?,
    onActivate: (() -> Unit)?,
    keyToString: (K) -> String,
    row: @Composable RowScope.(T) -> Unit,
) {
    var localExpanded by remember { mutableStateOf<List<K>>(emptyList()) }
    val expandedList = externalExpanded ?: localExpanded
    val setExpanded: (K, Boolean) -> Unit = { key, open ->
        val next = if (open) {
            if (expandedList.contains(key)) expandedList else expandedList + key
        } else {
            expandedList.filterNot { it == key }
        }
        if (onExpandedChange != null) onExpandedChange(next) else localExpanded = next
    }
    BernControl(
        target,
        "tree_view",
        label,
        actions = BernControlActions(
            activate = onActivate,
            edit = { cmd ->
                val idx = cmd.indexOf(':')
                if (idx > 0) {
                    val op = cmd.substring(0, idx).trim().lowercase()
                    val wanted = cmd.substring(idx + 1).trim()
                    val hit = entries.firstOrNull { keyToString(it.key) == wanted }
                    if (hit != null) {
                        when (op) {
                            "expand" -> setExpanded(hit.key, true)
                            "collapse" -> setExpanded(hit.key, false)
                            "toggle" -> setExpanded(hit.key, !expandedList.contains(hit.key))
                        }
                    }
                }
            },
            select = { wanted ->
                val hit = entries.firstOrNull { keyToString(it.key) == wanted }
                if (hit != null) {
                    onSelect?.invoke(hit.key)
                    true
                } else {
                    false
                }
            },
            value = { selectedKey?.let(keyToString) },
        ),
    )
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        for (entry in entries) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable {
                        onSelect?.invoke(entry.key)
                        onActivate?.invoke()
                    }
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                row(entry.item)
            }
        }
    }
}

@Composable
fun BernSplitPane(
    target: String?,
    vertical: Boolean,
    externalRatio: Double?,
    onRatioChange: ((Double) -> Unit)?,
    minFirst: Dp,
    minSecond: Dp,
    first: @Composable () -> Unit,
    second: @Composable () -> Unit,
) {
    var localRatio by remember { mutableStateOf(0.5) }
    val ratio = (externalRatio ?: localRatio).coerceIn(0.05, 0.95)
    if (target != null) {
        BernControl(
            target,
            "split_pane",
            target,
            actions = BernControlActions(
                setNumber = { v ->
                    val next = v.coerceIn(0.05, 0.95)
                    if (onRatioChange != null) onRatioChange(next) else localRatio = next
                },
                value = { String.format(java.util.Locale.US, "%.3f", ratio) },
            ),
        )
    }
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        first()
        second()
    }
}

@Composable
fun <T> BernReorderable(
    target: String,
    label: String,
    items: List<T>,
    onMove: (Long, Long) -> Unit,
    row: @Composable RowScope.(T) -> Unit,
) {
    BernControl(
        target,
        "reorderable",
        label,
        actions = BernControlActions(
            edit = { raw ->
                val parts = raw.trim().replace("->", ":").split(":")
                if (parts.size == 2) {
                    val a = parts[0].trim().toIntOrNull()
                    val b = parts[1].trim().toIntOrNull()
                    if (a != null && b != null && a in items.indices && b in items.indices && a != b) {
                        onMove(a.toLong(), b.toLong())
                    }
                }
            },
            value = { items.size.toString() },
        ),
    )
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        items.forEachIndexed { index, item ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Row(modifier = Modifier.weight(1f)) { row(item) }
                if (index > 0) {
                    FilledTonalButton(onClick = { onMove(index.toLong(), (index - 1).toLong()) }) { Text("^") }
                }
            }
        }
    }
}

@Composable
fun BernBottomSheet(
    detents: String,
    dragIndicator: Boolean,
    onDismiss: (() -> Unit)?,
    content: @Composable ColumnScope.() -> Unit,
) {
    BernWearDialog(onDismiss = { onDismiss?.invoke() }, content = content)
}

@Composable
fun BernRefreshable(
    target: String,
    refreshing: Boolean,
    onRefresh: () -> Unit,
    content: @Composable () -> Unit,
) {
    BernControl(
        target,
        "refreshable",
        target,
        actions = BernControlActions(
            activate = onRefresh,
            value = { if (refreshing) "refreshing" else "idle" },
        ),
    )
    Column(modifier = Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (refreshing) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
        }
        content()
    }
}
