// Bern standard controls, backed by Wear Material 3 rather than phone Material.
package dev.bern.runtime

import android.text.InputType
import android.text.TextWatcher
import android.text.Editable
import android.widget.EditText
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.*
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.wear.compose.material3.*

@Composable
fun BernProgress(value: Double?, total: Double, label: String?) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (label != null) Text(label, style = MaterialTheme.typography.labelMedium)
        if (value == null) CircularProgressIndicator(modifier = Modifier.size(24.dp))
        else LinearProgressIndicator(progress = { if (total > 0) (value / total).coerceIn(0.0, 1.0).toFloat() else 0f },
            modifier = Modifier.fillMaxWidth())
    }
}

@Composable
fun BernProgressRing(value: Double, total: Double, width: Dp, strokeWidth: Double, label: String?) {
    CircularProgressIndicator(progress = { if (total > 0) (value / total).coerceIn(0.0, 1.0).toFloat() else 0f },
        modifier = Modifier.size(width).semantics { if (label != null) contentDescription = label }, strokeWidth = strokeWidth.dp)
}

@Composable
fun BernButton(label: String, icon: ImageVector?, prominence: BernProminence, destructive: Boolean = false,
    enabled: Boolean = true, style: BernStyle? = null, shape: BernShape? = null, fill: Boolean = false,
    modifier: Modifier = Modifier, onDoubleTap: (() -> Unit)? = null, onClick: () -> Unit) {
    var sized = modifier.bernDoubleTap(enabled, onClick, onDoubleTap)
    if (fill) sized = sized.fillMaxWidth()
    if (shape?.height != null) sized = sized.height(shape.height)
    val textStyle = when (style?.size) {
        BernControlSize.Small -> MaterialTheme.typography.labelSmall
        BernControlSize.Large -> MaterialTheme.typography.titleMedium
        BernControlSize.ExtraLarge -> MaterialTheme.typography.titleLarge
        else -> MaterialTheme.typography.labelMedium
    }
    val content: @Composable RowScope.() -> Unit = {
        if (icon != null) { Icon(icon, null, Modifier.size(20.dp)); Spacer(Modifier.width(6.dp)) }
        Text(label, style = textStyle)
    }
    val error = MaterialTheme.colorScheme.error
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { m, interaction ->
        when (prominence) {
            BernProminence.Prominent -> Button(onClick = onClick, modifier = m, enabled = enabled,
                colors = if (destructive) ButtonDefaults.buttonColors(containerColor = error) else ButtonDefaults.buttonColors(),
                interactionSource = interaction, content = content)
            BernProminence.Standard -> FilledTonalButton(onClick = onClick, modifier = m, enabled = enabled,
                colors = if (destructive) ButtonDefaults.filledTonalButtonColors(contentColor = error) else ButtonDefaults.filledTonalButtonColors(),
                interactionSource = interaction, content = content)
            BernProminence.Plain -> ChildButton(onClick = onClick, modifier = m, enabled = enabled,
                colors = if (destructive) ButtonDefaults.childButtonColors(contentColor = error) else ButtonDefaults.childButtonColors(),
                interactionSource = interaction, content = content)
        }
    }
    if (style == null) material(sized, null) else BernStyledButton(style, enabled, sized, onClick, material, content)
}

@Composable
fun BernIconButton(label: String, icon: ImageVector?, enabled: Boolean, modifier: Modifier = Modifier,
    style: BernStyle? = null, iconSize: Float? = null, diameter: Dp? = null,
    prominence: BernProminence = BernProminence.Standard, iconProgress: Double? = null,
    badgeIcon: ImageVector? = null, badgeVisible: Boolean = true, onClick: () -> Unit) {
    val glyph: @Composable () -> Unit = {
        BernIconGlyph(icon ?: BernIcons.get(""), label, iconSize?.let(::bernIconSize) ?: 24.dp,
            iconProgress, badgeIcon, badgeVisible)
    }
    val sized = if (diameter != null) modifier.size(diameter) else modifier
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { m, interaction ->
        when (prominence) {
            BernProminence.Prominent -> FilledIconButton(onClick = onClick, enabled = enabled, modifier = m, interactionSource = interaction) { glyph() }
            BernProminence.Standard -> FilledTonalIconButton(onClick = onClick, enabled = enabled, modifier = m, interactionSource = interaction) { glyph() }
            BernProminence.Plain -> IconButton(onClick = onClick, enabled = enabled, modifier = m, interactionSource = interaction) { glyph() }
        }
    }
    if (style == null) material(sized, null) else BernStyledButton(style, enabled, sized, onClick, material) { glyph() }
}

// ChildButton is Wear's text action; no phone TextButton overload is imported.
@Composable
fun TextButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true,
    content: @Composable RowScope.() -> Unit) =
    ChildButton(onClick = onClick, modifier = modifier, enabled = enabled, content = content)

// Wear has no divider component. A one-pixel rule is a layout primitive, not
// a reimplementation of a control; its contrast comes from Wear's palette.
@Composable
fun HorizontalDivider(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(1.dp).background(MaterialTheme.colorScheme.outlineVariant))
}
@Composable
fun VerticalDivider(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxHeight().width(1.dp).background(MaterialTheme.colorScheme.outlineVariant))
}

@Composable
fun BernContentButton(label: String, enabled: Boolean, style: BernStyle?, modifier: Modifier = Modifier,
    shape: BernShape? = null, prominent: Boolean = false, onDoubleTap: (() -> Unit)? = null,
    onClick: () -> Unit, content: @Composable () -> Unit) {
    val tapping = modifier.bernDoubleTap(enabled, onClick, onDoubleTap)
    if (shape?.capsule == true || prominent) {
        val height = shape?.height
        val sized = (if (height != null) tapping.height(height) else tapping)
            .semantics(mergeDescendants = true) { contentDescription = label }
        val padding = if (height != null) PaddingValues(horizontal = 14.dp, vertical = 0.dp) else ButtonDefaults.ContentPadding
        if (prominent) Button(onClick = onClick, enabled = enabled, modifier = sized, contentPadding = padding) { content() }
        else FilledTonalButton(onClick = onClick, enabled = enabled, modifier = sized, contentPadding = padding) { content() }
        return
    }
    val interaction = remember { BernInteractions() }
    val (look, motion) = (style ?: BernStyle()).look(false, interaction.hovered, interaction.pressed, enabled)
    val palette = bernPalette()
    val outline = RoundedCornerShape((style?.corner ?: 0.0).dp)
    val sized = shape?.height?.let { tapping.height(it) } ?: tapping
    Box(
        sized
            .bernTransform(look, motion)
            .bernShadow(look, motion, outline)
            .clip(outline)
            .background(look.background?.let(palette::color) ?: Color.Transparent)
            .clickable(interaction, ripple(), enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics(mergeDescendants = true) { contentDescription = label },
    ) {
        CompositionLocalProvider(
            LocalContentColor provides (look.foreground?.let(palette::color) ?: LocalContentColor.current),
            content = content,
        )
    }
}

@Composable
fun BernSwitch(label: String, value: Boolean, onChange: (Boolean) -> Unit, checkbox: Boolean,
    labelHidden: Boolean = false, modifier: Modifier = Modifier) {
    val named = modifier.semantics { contentDescription = label }
    val text: @Composable RowScope.() -> Unit = { if (!labelHidden) Text(label) }
    if (checkbox) CheckboxButton(checked = value, onCheckedChange = onChange, modifier = named, label = text)
    else SwitchButton(checked = value, onCheckedChange = onChange, modifier = named, label = text)
}

@Composable
fun BernToggleButton(label: String, icon: ImageVector?, checked: Boolean, onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier, shape: BernShape? = null) {
    if (icon != null) IconToggleButton(checked = checked, onCheckedChange = onChange,
        modifier = modifier.then(shape?.height?.let { Modifier.size(it) } ?: Modifier)) { Icon(icon, label) }
    else SwitchButton(checked = checked, onCheckedChange = onChange,
        modifier = modifier.then(shape?.height?.let { Modifier.height(it) } ?: Modifier)) { Text(label) }
}

@Composable
fun BernSlider(label: String, value: Double, onChange: (Double) -> Unit, min: Double, max: Double,
    step: Double?, onCommit: (() -> Unit)?, minIcon: ImageVector?, maxIcon: ImageVector?,
    labelHidden: Boolean = false, modifier: Modifier = Modifier) {
    val count = if (step != null && step > 0) kotlin.math.round((max - min) / step).toInt().coerceAtLeast(1) else 100
    Column(modifier.fillMaxWidth().semantics { contentDescription = label }) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
        Slider(value = value.coerceIn(min, max).toFloat(),
            onValueChange = { onChange(it.toDouble()); onCommit?.invoke() }, steps = (count - 1).coerceAtLeast(0),
            valueRange = min.toFloat()..max.toFloat(), enabled = max > min,
            decreaseIcon = { if (minIcon != null) Icon(minIcon, null) else SliderDefaults.DecreaseIcon() },
            increaseIcon = { if (maxIcon != null) Icon(maxIcon, null) else SliderDefaults.IncreaseIcon() })
    }
}

// Wear Slider's +/- actions are atomic rather than a phone thumb drag.
// Commit each user step once, including when continuous is requested.
@Composable
fun BernControlledSlider(label: String, value: Double, commit: (Double) -> Unit, min: Double, max: Double,
    step: Double?, minIcon: ImageVector?, maxIcon: ImageVector?, labelHidden: Boolean = false,
    continuous: Boolean = false, modifier: Modifier = Modifier) =
    BernSlider(label, value, commit, min, max, step, null, minIcon, maxIcon, labelHidden, modifier)

// The native state indexes one immutable option snapshot. When options are
// reordered or resized, rebuild that indexed state and keep its draft by value.
// A new bound selection wins; a removed draft falls back to the bound value,
// then the first remaining option. None of these changes submits a selection.
private class BernWearPickerHistory<T> {
    private var options: List<T> = emptyList()
    private var bound: T? = null
    private var state: PickerState? = null

    fun initialIndex(next: List<T>, selected: T): Int {
        val index = state?.selectedOptionIndex
        val draft = if (state != null && bound == selected && index != null && index in options.indices) options[index] else selected
        return next.indexOf(draft).takeIf { it >= 0 } ?: next.indexOf(selected).coerceAtLeast(0)
    }

    fun keep(options: List<T>, selected: T, state: PickerState) {
        this.options = options
        this.bound = selected
        this.state = state
    }
}

@Composable
fun <T> BernPicker(label: String, options: List<T>, selected: T, optionLabel: (T) -> String,
    onSelect: (T) -> Unit, style: String, modifier: Modifier = Modifier, labelHidden: Boolean = false,
    shape: BernShape? = null, inFlow: Boolean = false, inRow: Boolean = false) {
    var open by remember { mutableStateOf(false) }
    val inline = style in listOf("radio", "inline", "segmented", "chips")
    // An empty/replaced inline picker must not reopen an old dialog when its
    // options later return. Closing here never calls onSelect.
    SideEffect { if (options.isEmpty() || inline) open = false }
    if (inline) {
        Column(modifier.fillMaxWidth().semantics { contentDescription = label }, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
            options.forEach { option ->
                RadioButton(selected = option == selected, onSelect = { onSelect(option) }) { Text(optionLabel(option)) }
            }
        }
    } else {
        FilledTonalButton(onClick = { open = true }, enabled = options.isNotEmpty(),
            modifier = modifier.then(if (!inFlow && !inRow) Modifier.fillMaxWidth() else Modifier)
                .then(shape?.height?.let { Modifier.height(it) } ?: Modifier)
                .semantics { contentDescription = label },
            secondaryLabel = if (labelHidden) null else ({ Text(label) }),
            label = { Text(optionLabel(selected)) })
        if (open && options.isNotEmpty()) {
            val history = remember { BernWearPickerHistory<T>() }
            val snapshot = options.toList()
            Dialog(visible = true, onDismissRequest = { open = false }) {
                key(snapshot, selected) {
                    val initial = remember { history.initialIndex(snapshot, selected) }
                    val state = rememberPickerState(snapshot.size, initial, shouldRepeatOptions = false)
                    SideEffect { history.keep(snapshot, selected, state) }
                    Column(Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
                        Picker(state = state, contentDescription = { label }, modifier = Modifier.weight(1f).fillMaxWidth()) { index ->
                            Text(optionLabel(snapshot[index]), style = MaterialTheme.typography.titleMedium)
                        }
                        Button(onClick = { onSelect(snapshot[state.selectedOptionIndex]); open = false }) { Text(BernText.string("bern.alert.ok")) }
                    }
                }
            }
        }
    }
}

@Composable
fun <T> BernToolbarPicker(label: String, options: List<T>, selected: T, optionLabel: (T) -> String,
    onSelect: (T) -> Unit, modifier: Modifier = Modifier) =
    BernPicker(label, options, selected, optionLabel, onSelect, "menu", modifier, labelHidden = true)

// Explicit focus is separate from Android's default-focus traversal.
// The controller does not retain an Android View after it leaves composition.
class BernWearInputFocus {
    private var view = java.lang.ref.WeakReference<BernWearEditText>(null)
    internal var focused by mutableStateOf(false)
        private set

    fun request() {
        val editor = view.get() ?: throw BernCommandFailure("target_unavailable", "The text input is not mounted")
        editor.beginEditing()
    }

    internal fun attach(field: EditText) {
        val editor = field as BernWearEditText
        if (view.get() === editor) return
        view.get()?.focusChanged = null
        view = java.lang.ref.WeakReference(editor)
        editor.focusChanged = { focused = it }
        focused = editor.hasFocus()
    }

    internal fun detach(field: EditText) {
        if (view.get() !== field) return
        (field as BernWearEditText).focusChanged = null
        view.clear()
        focused = false
    }
}

// Keep the native default in pixels so removing a declared size restores it,
// instead of leaving the last explicit size on a reused AndroidView.
private class BernWearEditText(context: android.content.Context) : EditText(context) {
    private val systemTextSize = textSize
    var focusChanged: ((Boolean) -> Unit)? = null
    var applyingValue = false
        private set

    fun value(text: String) {
        if (this.text.toString() == text) return
        applyingValue = true
        try { setText(text); setSelection(this.text.length) } finally { applyingValue = false }
    }

    init {
        // Android must not pick the first EditText as a new window's default
        // focus: that also asks the enclosing watch form to scroll to it.
        isFocusableInTouchMode = false
        isFocusable = false
    }

    private fun allowEditing() {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    fun beginEditing() {
        allowEditing()
        if (requestFocus()) {
            (context.getSystemService(android.content.Context.INPUT_METHOD_SERVICE) as? android.view.inputmethod.InputMethodManager)
                ?.showSoftInput(this, android.view.inputmethod.InputMethodManager.SHOW_IMPLICIT)
        }
    }

    override fun onTouchEvent(event: android.view.MotionEvent): Boolean {
        if (event.actionMasked == android.view.MotionEvent.ACTION_DOWN) allowEditing()
        return super.onTouchEvent(event)
    }

    override fun performClick(): Boolean {
        beginEditing()
        return super.performClick()
    }

    override fun performAccessibilityAction(action: Int, arguments: android.os.Bundle?): Boolean {
        if (action == android.view.accessibility.AccessibilityNodeInfo.ACTION_FOCUS ||
            action == android.view.accessibility.AccessibilityNodeInfo.ACTION_SET_TEXT) allowEditing()
        return super.performAccessibilityAction(action, arguments)
    }

    override fun onFocusChanged(focused: Boolean, direction: Int, previouslyFocusedRect: android.graphics.Rect?) {
        super.onFocusChanged(focused, direction, previouslyFocusedRect)
        focusChanged?.invoke(focused)
        if (!focused) {
            // After leaving an editor, remounts/window navigation cannot
            // reactivate it without another deliberate edit request.
            isFocusableInTouchMode = false
            isFocusable = false
        }
    }

    fun fontSize(points: Float?) {
        if (points == null) setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX, systemTextSize)
        else setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, points)
    }
}

// Wear has no Material text field. Android's actual EditText invokes the
// installed watch IME (voice/handwriting/keyboard) and preserves input semantics.
@Composable
fun BernTextInput(value: String, onChange: (String) -> Unit, label: String, prompt: String?, kind: String,
    submit: (() -> Unit)?, modifier: Modifier = Modifier, labelHidden: Boolean = false, fontSize: Float? = null,
    inputFocus: BernWearInputFocus? = null) {
    val change by rememberUpdatedState(onChange)
    val done by rememberUpdatedState(submit)
    val foreground = MaterialTheme.colorScheme.onSurface.toArgb()
    val hint = MaterialTheme.colorScheme.onSurfaceVariant.toArgb()
    Column(modifier.fillMaxWidth()) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.labelMedium)
        AndroidView(modifier = Modifier.fillMaxWidth(), factory = { context ->
            BernWearEditText(context).apply {
                this.fontSize(fontSize)
                contentDescription = label
                this.hint = prompt
                setTextColor(foreground); setHintTextColor(hint)
                inputType = when (kind) {
                    "secure" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                    "multiline" -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
                    else -> InputType.TYPE_CLASS_TEXT
                }
                setSingleLine(kind != "multiline")
                imeOptions = if (submit != null) android.view.inputmethod.EditorInfo.IME_ACTION_DONE else android.view.inputmethod.EditorInfo.IME_ACTION_UNSPECIFIED
                setText(value)
                addTextChangedListener(object : TextWatcher {
                    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
                    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                        if (!applyingValue) change(s?.toString() ?: "")
                    }
                    override fun afterTextChanged(s: Editable?) {}
                })
                setOnEditorActionListener { _, action, _ ->
                    if (action == android.view.inputmethod.EditorInfo.IME_ACTION_DONE && done != null) { done?.invoke(); true } else false
                }
            }
        }, onRelease = { field -> inputFocus?.detach(field) }, update = { field ->
            inputFocus?.attach(field)
            field.fontSize(fontSize)
            field.value(value)
            field.contentDescription = label; field.hint = prompt
            field.setTextColor(foreground); field.setHintTextColor(hint)
        })
    }
}

@Composable
fun BernHeldTextInput(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    prompt: String?,
    kind: String,
    modifier: Modifier = Modifier,
    labelHidden: Boolean = false,
    inputFocus: BernWearInputFocus? = null,
) {
    val focus = inputFocus ?: remember { BernWearInputFocus() }
    var held by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(focus.focused) { if (!focus.focused) held = null }
    BernTextInput(held ?: value, { held = it; onChange(it) }, label, prompt, kind, null, modifier, labelHidden, inputFocus = focus)
}

@Composable
fun BernSearchField(value: String, onChange: (String) -> Unit, prompt: String, submit: (() -> Unit)?,
    modifier: Modifier = Modifier, width: Dp? = null, height: Dp? = null, fontSize: Float? = null) =
    BernTextInput(value, onChange, prompt, prompt, "text", submit,
        modifier.then(width?.let { Modifier.width(it) } ?: Modifier).then(height?.let { Modifier.height(it) } ?: Modifier),
        labelHidden = true, fontSize = fontSize)

@Composable
fun BernSizedSearchField(value: String, onChange: (String) -> Unit, prompt: String, submit: (() -> Unit)?,
    modifier: Modifier, width: Dp?, height: Dp?, fontSize: Float?) =
    BernSearchField(value, onChange, prompt, submit, modifier, width, height, fontSize)

@Composable
fun BernTableHeadings(chosen: ToggleableState?, chooseAll: (Boolean) -> Unit, headings: @Composable RowScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().padding(top = 28.dp, start = 16.dp, end = 16.dp, bottom = 4.dp)) {
        if (chosen != null) CheckboxButton(checked = chosen == ToggleableState.On,
            onCheckedChange = chooseAll, modifier = Modifier.fillMaxWidth().semantics { toggleableState = chosen }) { Row(content = headings) }
        else Row(Modifier.fillMaxWidth(), content = headings)
    }
}
@Composable
fun BernTableHeading(title: String, sorted: Boolean, descending: Boolean, sort: (() -> Unit)?, modifier: Modifier = Modifier) {
    if (sort == null) Text(title, modifier.semantics { heading() }, style = MaterialTheme.typography.labelMedium)
    else ChildButton(onClick = sort, modifier = modifier.semantics { heading() }) {
        Text(title)
        if (sorted) Icon(BernIcons.get(if (descending) "chevron.down" else "chevron.up"), null)
    }
}
@Composable
fun BernTableRow(chosen: Boolean?, toggle: (Boolean) -> Unit, only: () -> Unit, primary: (() -> Unit)?,
    menu: (@Composable ColumnScope.(dismiss: () -> Unit) -> Unit)?, modifier: Modifier = Modifier,
    cells: @Composable RowScope.() -> Unit) {
    var open by remember { mutableStateOf(false) }
    val click = { if (primary != null) { only(); primary() } else if (chosen != null) toggle(!chosen) }
    val openMenu = { if (chosen == false) only(); open = true }
    val latestOpen by rememberUpdatedState(openMenu)
    val menuModifier = if (menu == null) Modifier else Modifier.bernMenuPress { latestOpen() }
        .semantics { onLongClick { latestOpen(); true } }
    Column(modifier.padding(horizontal = 12.dp, vertical = 2.dp)) {
        when {
            // A primary row action and selection are distinct native touch
            // targets: tapping the checkbox never invokes the primary action.
            chosen != null && primary != null -> SplitCheckboxButton(
                checked = chosen, onCheckedChange = toggle, toggleContentDescription = null,
                onContainerClick = click, modifier = Modifier.fillMaxWidth().then(menuModifier), label = cells)
            // With no primary action the entire label toggles selection; its
            // long press still opens the menu without also toggling the row.
            chosen != null -> CheckboxButton(checked = chosen, onCheckedChange = toggle,
                modifier = Modifier.fillMaxWidth().then(menuModifier), label = cells)
            primary != null -> FilledTonalButton(onClick = click,
                onLongClick = if (menu == null) null else openMenu,
                modifier = Modifier.fillMaxWidth(), content = cells)
            // A display-only row is not a button with a no-op click action.
            else -> Row(Modifier.fillMaxWidth().then(menuModifier), content = cells)
        }
        if (open && menu != null) BernWearDialog(onDismiss = { open = false }) { menu { open = false } }
    }
}
