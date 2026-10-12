// Bern Compose views. Generated build input; do not edit.
//
// The Material 3 realization of Bern's standard components. Each keeps the
// platform look; the generated code passes values and actions, and
// registers the control for external entries separately.

package dev.bern.runtime

import android.graphics.BitmapFactory
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Create
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Face
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Phone
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.ShoppingCart
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.ThumbUp
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.io.File

// MARK: - Icons

/// Bern icon names (SF Symbols naming) as Material icons. Names without a
/// counterpart show a neutral dot.
object BernIcons {
    fun get(name: String): ImageVector = when (name) {
        "plus", "plus.circle" -> Icons.Filled.Add
        "minus" -> Icons.Filled.Clear
        "xmark", "xmark.circle", "xmark.circle.fill" -> Icons.Filled.Close
        "checkmark" -> Icons.Filled.Check
        "checkmark.circle", "checkmark.circle.fill" -> Icons.Filled.CheckCircle
        "chevron.right" -> Icons.AutoMirrored.Filled.KeyboardArrowRight
        "chevron.down", "chevron.up.chevron.down" -> Icons.Filled.KeyboardArrowDown
        "chevron.up" -> Icons.Filled.KeyboardArrowUp
        "arrow.right" -> Icons.AutoMirrored.Filled.ArrowForward
        "ellipsis", "ellipsis.circle" -> Icons.Filled.MoreVert
        "magnifyingglass" -> Icons.Filled.Search
        "gearshape", "gearshape.2", "gear" -> Icons.Filled.Settings
        "house", "house.fill" -> Icons.Filled.Home
        "person", "person.fill", "person.crop.circle" -> Icons.Filled.Person
        "trash", "trash.fill" -> Icons.Filled.Delete
        "pencil", "square.and.pencil" -> Icons.Filled.Edit
        "square.and.arrow.up" -> Icons.Filled.Share
        "star", "star.fill" -> Icons.Filled.Star
        "heart" -> Icons.Filled.FavoriteBorder
        "heart.fill" -> Icons.Filled.Favorite
        "info.circle", "info" -> Icons.Filled.Info
        "exclamationmark.triangle" -> Icons.Filled.Warning
        "bell", "bell.fill" -> Icons.Filled.Notifications
        "lock", "lock.fill" -> Icons.Filled.Lock
        "envelope" -> Icons.Filled.Email
        "phone" -> Icons.Filled.Phone
        "calendar" -> Icons.Filled.DateRange
        "mappin", "location" -> Icons.Filled.LocationOn
        "map" -> Icons.Filled.Place
        "cart" -> Icons.Filled.ShoppingCart
        "hand.thumbsup" -> Icons.Filled.ThumbUp
        "play.fill", "play", "play.rectangle", "play.circle" -> Icons.Filled.PlayArrow
        "arrow.clockwise" -> Icons.Filled.Refresh
        "paperplane" -> Icons.AutoMirrored.Filled.Send
        "wrench", "hammer" -> Icons.Filled.Build
        "face.smiling" -> Icons.Filled.Face
        "line.3.horizontal", "sidebar.left" -> Icons.Filled.Menu
        "list.bullet", "list.bullet.rectangle", "doc", "doc.text", "note.text" -> Icons.AutoMirrored.Filled.List
        "person.circle" -> Icons.Filled.AccountCircle
        "phone.fill" -> Icons.Filled.Call
        "pencil.line" -> Icons.Filled.Create
        "checkmark.seal" -> Icons.Filled.Done
        else -> Icons.Filled.Info
    }
}

// MARK: - Keys

/// A key Compose can save: the value itself where it is a string or a
/// number, else its text.
object BernKey {
    fun of(value: Any?): Any = when (value) {
        is String, is Long, is Int, is Double, is Boolean -> value
        null -> "null"
        else -> value.toString()
    }
}

// MARK: - Text and labels

@Composable
fun BernLabel(title: String, icon: ImageVector?, style: TextStyle) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (icon != null) Icon(icon, contentDescription = null)
        Text(title, style = style)
    }
}

// MARK: - Buttons

enum class BernProminence { Standard, Prominent, Plain }

@Composable
fun BernButton(
    label: String,
    icon: ImageVector?,
    prominence: BernProminence,
    destructive: Boolean = false,
    enabled: Boolean = true,
    style: BernStyle? = null,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val textStyle = when (style?.size) {
        BernControlSize.Small -> MaterialTheme.typography.labelMedium
        BernControlSize.Large -> MaterialTheme.typography.titleMedium
        BernControlSize.ExtraLarge -> MaterialTheme.typography.titleLarge
        else -> MaterialTheme.typography.labelLarge
    }
    val content: @Composable RowScope.() -> Unit = {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(label, style = textStyle)
    }
    val error = MaterialTheme.colorScheme.error
    val padding = if (style?.size != null) style.insets else ButtonDefaults.ContentPadding
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { modifier, interaction ->
        when (prominence) {
            BernProminence.Prominent -> Button(
                onClick, modifier, enabled,
                colors = if (destructive) ButtonDefaults.buttonColors(containerColor = error) else ButtonDefaults.buttonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
            BernProminence.Plain -> TextButton(
                onClick, modifier, enabled,
                colors = if (destructive) ButtonDefaults.textButtonColors(contentColor = error) else ButtonDefaults.textButtonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
            BernProminence.Standard -> FilledTonalButton(
                onClick, modifier, enabled,
                colors = if (destructive) ButtonDefaults.filledTonalButtonColors(contentColor = error) else ButtonDefaults.filledTonalButtonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
        }
    }
    if (style == null) {
        material(modifier, null)
    } else {
        BernStyledButton(style, enabled, modifier, onClick, material, content)
    }
}

@Composable
fun BernIconButton(
    label: String,
    icon: ImageVector?,
    enabled: Boolean,
    modifier: Modifier = Modifier,
    style: BernStyle? = null,
    onClick: () -> Unit,
) {
    val content: @Composable RowScope.() -> Unit = {
        Icon(icon ?: Icons.Filled.Info, contentDescription = label)
    }
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { modifier, interaction ->
        IconButton(onClick = onClick, enabled = enabled, modifier = modifier, interactionSource = interaction) {
            Icon(icon ?: Icons.Filled.Info, contentDescription = label)
        }
    }
    if (style == null) {
        material(modifier, null)
    } else {
        BernStyledButton(style, enabled, modifier, onClick, material, content)
    }
}

// MARK: - Inputs

@Composable
fun BernTextInput(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    prompt: String?,
    kind: String,
    submit: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        placeholder = prompt?.let { { Text(it) } },
        singleLine = kind != "multiline",
        minLines = if (kind == "multiline") 3 else 1,
        maxLines = if (kind == "multiline") 8 else 1,
        visualTransformation = if (kind == "secure") PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (kind == "secure") KeyboardType.Password else KeyboardType.Text,
            imeAction = if (submit != null && kind != "multiline") ImeAction.Done else ImeAction.Default,
        ),
        keyboardActions = KeyboardActions(onDone = { submit?.invoke() }),
        modifier = modifier.fillMaxWidth(),
    )
}

@Composable
fun BernSearchField(
    value: String,
    onChange: (String) -> Unit,
    prompt: String,
    submit: (() -> Unit)?,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        placeholder = { Text(prompt) },
        leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { submit?.invoke() }),
        modifier = modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
    )
}

@Composable
fun BernSwitch(
    label: String,
    value: Boolean,
    onChange: (Boolean) -> Unit,
    checkbox: Boolean,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier
            .fillMaxWidth()
            .toggleable(value = value, role = if (checkbox) Role.Checkbox else Role.Switch, onValueChange = onChange)
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (checkbox) {
            Checkbox(checked = value, onCheckedChange = null)
            Spacer(Modifier.width(12.dp))
            Text(label, Modifier.weight(1f))
        } else {
            Text(label, Modifier.weight(1f))
            Switch(checked = value, onCheckedChange = null)
        }
    }
}

@Composable
fun BernSlider(
    label: String,
    value: Double,
    onChange: (Double) -> Unit,
    min: Double,
    max: Double,
    step: Double?,
    onCommit: (() -> Unit)?,
    minIcon: ImageVector?,
    maxIcon: ImageVector?,
    modifier: Modifier = Modifier,
) {
    val steps = if (step != null && step > 0) maxOf(0, Math.round((max - min) / step).toInt() - 1) else 0
    Column(modifier.fillMaxWidth()) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (minIcon != null) Icon(minIcon, contentDescription = null)
            Slider(
                value = value.toFloat(),
                onValueChange = { onChange(it.toDouble()) },
                valueRange = min.toFloat()..max.toFloat(),
                steps = steps,
                onValueChangeFinished = onCommit,
                modifier = Modifier.weight(1f),
            )
            if (maxIcon != null) Icon(maxIcon, contentDescription = null)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun <T> BernPicker(
    label: String,
    options: List<T>,
    selected: T,
    optionLabel: (T) -> String,
    onSelect: (T) -> Unit,
    style: String,
    modifier: Modifier = Modifier,
) {
    when (style) {
        "segmented" -> Column(modifier) {
            Text(label, style = MaterialTheme.typography.bodyMedium)
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                options.forEachIndexed { index, option ->
                    SegmentedButton(
                        selected = option == selected,
                        onClick = { onSelect(option) },
                        shape = SegmentedButtonDefaults.itemShape(index, options.size),
                    ) { Text(optionLabel(option), maxLines = 1) }
                }
            }
        }
        "radio", "inline" -> Column(modifier) {
            Text(label, style = MaterialTheme.typography.bodyMedium)
            for (option in options) {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .selectable(selected = option == selected, role = Role.RadioButton, onClick = { onSelect(option) })
                        .padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RadioButton(selected = option == selected, onClick = null)
                    Spacer(Modifier.width(12.dp))
                    Text(optionLabel(option))
                }
            }
        }
        else -> {
            var expanded by remember { mutableStateOf(false) }
            ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }, modifier = modifier) {
                OutlinedTextField(
                    value = optionLabel(selected),
                    onValueChange = {},
                    readOnly = true,
                    label = { Text(label) },
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded) },
                    modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
                )
                ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                    for (option in options) {
                        DropdownMenuItem(
                            text = { Text(optionLabel(option)) },
                            onClick = {
                                onSelect(option)
                                expanded = false
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun BernLanguagePicker(label: String, target: String) {
    BernControl(
        target, "picker", label,
        actions = BernControlActions(select = { BernLanguage.select(it) }, value = { BernLanguage.current }),
    )
    BernPicker(
        label, BernLanguage.supported, BernLanguage.current, { BernLanguage.autonym(it) },
        { BernLanguage.select(it) }, "menu",
    )
}

// MARK: - Progress and pictures

@Composable
fun BernProgress(value: Double?, total: Double, label: String?) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (label != null) Text(label, style = MaterialTheme.typography.bodyMedium)
        if (value == null) {
            CircularProgressIndicator()
        } else {
            val fraction = if (total > 0) (value / total).coerceIn(0.0, 1.0) else 0.0
            LinearProgressIndicator(progress = { fraction.toFloat() }, modifier = Modifier.fillMaxWidth())
        }
    }
}

/// A picture from a declared resource. With one side given, the other
/// follows the picture's aspect ratio. A label makes it an image for
/// assistive technologies; without one it is decorative.
@Composable
fun BernImage(name: String, label: String?, width: Dp?, height: Dp?, fit: ContentScale) {
    val bitmap = remember(name) {
        BitmapFactory.decodeFile(File(BernResources.root, name).absolutePath)?.asImageBitmap()
    } ?: return
    val ratio = bitmap.width.toFloat() / maxOf(1, bitmap.height)
    val modifier = when {
        width != null && height != null -> Modifier.size(width, height)
        width != null -> Modifier.width(width).height(width / ratio)
        height != null -> Modifier.height(height).width(height * ratio)
        else -> Modifier.fillMaxWidth()
    }
    Image(bitmap, contentDescription = label, modifier = modifier, contentScale = fit)
}

// MARK: - Groups and layout

@Composable
fun BernSection(title: String?, footer: String?, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (title != null) {
            Text(title, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        }
        content()
        if (footer != null) {
            Text(footer, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/// Equal columns no narrower than `minWidth`, as many as fit; children
/// fill them row by row.
@Composable
fun BernGrid(minWidth: Dp, gap: Dp, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier.fillMaxWidth()) { measurables, constraints ->
        val spacing = gap.roundToPx()
        val width = constraints.maxWidth
        val columns = maxOf(1, (width + spacing) / (minWidth.roundToPx() + spacing))
        val cell = maxOf(0, (width - spacing * (columns - 1)) / columns)
        val placeables = measurables.map { it.measure(constraints.copy(minWidth = cell, maxWidth = cell, minHeight = 0)) }
        val rows = placeables.chunked(columns)
        val heights = rows.map { row -> row.maxOf { it.height } }
        val total = heights.sum() + spacing * maxOf(0, rows.size - 1)
        layout(width, total) {
            var y = 0
            rows.forEachIndexed { index, row ->
                row.forEachIndexed { column, placeable -> placeable.place(column * (cell + spacing), y) }
                y += heights[index] + spacing
            }
        }
    }
}

/// One layout by the available width: `compact` below the breakpoint.
@Composable
fun BernAdaptive(breakpoint: Dp, target: String?, compact: @Composable () -> Unit, expanded: @Composable () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val size = if (maxWidth < breakpoint) "compact" else "expanded"
        if (target != null) BernControl(target, "adaptive", "", actions = BernControlActions(value = { size }))
        if (size == "compact") compact() else expanded()
    }
}

// MARK: - Menus

@Composable
fun BernMenu(label: String, icon: ImageVector?, modifier: Modifier = Modifier, items: @Composable (dismiss: () -> Unit) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier) {
        if (icon != null) {
            IconButton(onClick = { expanded = true }) { Icon(icon, contentDescription = label) }
        } else {
            TextButton(onClick = { expanded = true }) {
                Text(label)
                Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
            }
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            items { expanded = false }
        }
    }
}

@Composable
fun BernMenuItem(label: String, icon: ImageVector?, enabled: Boolean, destructive: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = {
            Text(label, color = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
        },
        leadingIcon = icon?.let { { Icon(it, contentDescription = null) } },
        enabled = enabled,
        onClick = onClick,
    )
}

// MARK: - Navigation

/// A row that pushes a route.
@Composable
fun BernNavLink(onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Row(
        modifier.fillMaxWidth().clickable(role = Role.Button, onClick = onClick).padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), content = content)
        Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null)
    }
}

/// A sidebar beside the selected destination where the window is wide; a
/// list of destinations that opens one where it is narrow, with the
/// system back returning to the list.
@Composable
fun BernSplitView(
    title: String,
    sidebar: @Composable (showDetail: () -> Unit) -> Unit,
    detail: @Composable () -> Unit,
) {
    var showingDetail by rememberSaveable { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxWidth >= 600.dp) {
            BernChrome(title = title)
            Row(Modifier.fillMaxSize()) {
                Column(Modifier.width(280.dp).fillMaxHeight().verticalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                    sidebar {}
                }
                VerticalDivider()
                Box(Modifier.weight(1f).fillMaxHeight()) { detail() }
            }
        } else if (showingDetail) {
            BernChrome(title = title, back = { showingDetail = false })
            Box(Modifier.fillMaxSize()) { detail() }
        } else {
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                sidebar { showingDetail = true }
            }
        }
    }
}

@Composable
fun BernSidebarSection(title: String?, content: @Composable () -> Unit) {
    if (title != null) {
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 16.dp, top = 16.dp, bottom = 8.dp),
        )
    }
    content()
}

@Composable
fun BernSidebarItem(label: String, icon: ImageVector?, selected: Boolean, onClick: () -> Unit) {
    NavigationDrawerItem(
        label = { Text(label) },
        icon = icon?.let { { Icon(it, contentDescription = null) } },
        selected = selected,
        onClick = onClick,
    )
}

/// Tab pages above a navigation bar.
@Composable
fun BernTabs(content: @Composable () -> Unit, bar: @Composable RowScope.() -> Unit) {
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.weight(1f).fillMaxWidth()) { content() }
        NavigationBar(content = bar)
    }
}

@Composable
fun RowScope.BernTabItem(label: String, icon: ImageVector?, selected: Boolean, onClick: () -> Unit) {
    NavigationBarItem(
        selected = selected,
        onClick = onClick,
        icon = { Icon(icon ?: Icons.Filled.Info, contentDescription = null) },
        label = { Text(label) },
    )
}
