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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
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
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Constraints
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
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.PathParser
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
        "chevron.left" -> Icons.AutoMirrored.Filled.KeyboardArrowLeft
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
        "pause", "pause.fill" -> pause
        "speaker.wave.2.fill", "speaker.wave.2", "speaker.wave.3.fill", "speaker.wave.3" -> volumeUp
        "speaker.wave.1.fill", "speaker.wave.1" -> volumeDown
        "speaker.fill", "speaker" -> volumeMute
        "sun.min", "sun.min.fill" -> brightnessLow
        "sun.max", "sun.max.fill" -> brightnessHigh
        "speaker.slash.fill", "speaker.slash" -> volumeOff
        "square.grid.2x2", "square.grid.2x2.fill", "rectangle.grid.2x2" -> grid
        "archivebox", "archivebox.fill" -> archive
        "tray", "tray.fill", "tray.full" -> inbox
        "photo", "photo.fill", "photo.on.rectangle" -> photo
        "film", "film.fill", "video", "video.fill", "play.rectangle.fill" -> movie
        "music.note", "music.note.list" -> music
        "folder", "folder.fill" -> folder
        "tag", "tag.fill" -> tag
        "globe" -> language
        "moon", "circle.lefthalf.filled", "paintbrush", "paintpalette" -> palette
        "arrow.down.circle", "arrow.down.to.line" -> download
        "slider.horizontal.3", "dial.low" -> tune
        "textformat", "textformat.size", "textformat.size.smaller", "textformat.size.larger" -> text
        "sidebar.right", "sidebar.trailing" -> sidebarRight
        "star.slash" -> starBorder
        "captions.bubble", "text.bubble", "captions.bubble.fill", "text.bubble.fill" -> subtitles
        "square.grid.3x3", "square.grid.3x3.fill" -> gridOn
        // Names without a counterpart show a neutral dot.
        else -> dot
    }

    // Media icons the core set lacks, from the Material icon paths.
    private val pause by lazy { icon("Pause", "M6,19h4L10,5L6,5v14zM14,5v14h4L18,5h-4z") }
    private val volumeUp by lazy {
        icon(
            "VolumeUp",
            "M3,9v6h4l5,5L12,4L7,9L3,9zM16.5,12c0,-1.77 -1.02,-3.29 -2.5,-4.03v8.05c1.48,-0.73 2.5,-2.25 2.5,-4.02zM14,3.23v2.06c2.89,0.86 5,3.54 5,6.71s-2.11,5.85 -5,6.71v2.06c4.01,-0.91 7,-4.49 7,-8.77s-2.99,-7.86 -7,-8.77z",
        )
    }
    private val volumeOff by lazy {
        icon(
            "VolumeOff",
            "M16.5,12c0,-1.77 -1.02,-3.29 -2.5,-4.03v2.21l2.45,2.45c0.03,-0.2 0.05,-0.41 0.05,-0.63zM19,12c0,0.94 -0.2,1.82 -0.54,2.64l1.51,1.51C20.63,14.91 21,13.5 21,12c0,-4.28 -2.99,-7.86 -7,-8.77v2.06c2.89,0.86 5,3.54 5,6.71zM4.27,3L3,4.27 7.73,9L3,9v6h4l5,5v-6.73l4.25,4.25c-0.67,0.52 -1.42,0.93 -2.25,1.18v2.06c1.38,-0.31 2.63,-0.95 3.69,-1.81L19.73,21 21,19.73l-9,-9L4.27,3zM12,4L9.91,6.09 12,8.18L12,4z",
        )
    }

    private val volumeDown by lazy {
        icon("VolumeDown", "M18.5,12c0,-1.77 -1.02,-3.29 -2.5,-4.03v8.05c1.48,-0.73 2.5,-2.25 2.5,-4.02zM5,9v6h4l5,5V4L9,9H5z")
    }
    private val volumeMute by lazy { icon("VolumeMute", "M7,9v6h4l5,5V4l-5,5H7z") }
    private val brightnessLow by lazy {
        icon("BrightnessLow", "M20,15.31L23.31,12 20,8.69V4h-4.69L12,0.69 8.69,4H4v4.69L0.69,12 4,15.31V20h4.69L12,23.31 15.31,20H20v-4.69zM12,18c-3.31,0 -6,-2.69 -6,-6s2.69,-6 6,-6 6,2.69 6,6 -2.69,6 -6,6z")
    }
    private val brightnessHigh by lazy {
        icon("BrightnessHigh", "M20,8.69V4h-4.69L12,0.69 8.69,4H4v4.69L0.69,12 4,15.31V20h4.69L12,23.31 15.31,20H20v-4.69L23.31,12 20,8.69zM12,18c-3.31,0 -6,-2.69 -6,-6s2.69,-6 6,-6 6,2.69 6,6 -2.69,6 -6,6zM12,8c-2.21,0 -4,1.79 -4,4s1.79,4 4,4 4,-1.79 4,-4 -1.79,-4 -4,-4z")
    }
    private val grid by lazy {
        icon("Grid", "M3,3v8h8V3H3zM9,9H5V5h4V9zM3,13v8h8v-8H3zM9,19H5v-4h4V19zM13,3v8h8V3H13zM19,9h-4V5h4V9zM13,13v8h8v-8H13zM19,19h-4v-4h4V19z")
    }
    private val archive by lazy {
        icon("Archive", "M20.54,5.23l-1.39,-1.68C18.88,3.21 18.47,3 18,3H6c-0.47,0 -0.88,0.21 -1.16,0.55L3.46,5.23C3.17,5.57 3,6.02 3,6.5V19c0,1.1 0.9,2 2,2h14c1.1,0 2,-0.9 2,-2V6.5c0,-0.48 -0.17,-0.93 -0.46,-1.27zM12,17.5L6.5,12H10v-2h4v2h3.5L12,17.5zM5.12,5l0.81,-1h12l0.94,1H5.12z")
    }
    private val inbox by lazy {
        icon("Inbox", "M19,3H4.99c-1.11,0 -1.98,0.89 -1.98,2L3,19c0,1.1 0.88,2 1.99,2H19c1.1,0 2,-0.9 2,-2V5c0,-1.11 -0.9,-2 -2,-2zM19,15h-4c0,1.66 -1.35,3 -3,3s-3,-1.34 -3,-3H4.99V5H19v10z")
    }
    private val photo by lazy {
        icon("Photo", "M21,19V5c0,-1.1 -0.9,-2 -2,-2H5c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h14c1.1,0 2,-0.9 2,-2zM8.5,13.5l2.5,3.01L14.5,12l4.5,6H5l3.5,-4.5z")
    }
    private val movie by lazy {
        icon("Movie", "M18,4l2,4h-3l-2,-4h-2l2,4h-3l-2,-4H8l2,4H7L5,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4h-4z")
    }
    private val music by lazy {
        icon("Music", "M12,3v10.55c-0.59,-0.34 -1.27,-0.55 -2,-0.55 -2.21,0 -4,1.79 -4,4s1.79,4 4,4 4,-1.79 4,-4V7h4V3h-6z")
    }
    private val folder by lazy {
        icon("Folder", "M10,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V8c0,-1.1 -0.9,-2 -2,-2h-8l-2,-2z")
    }
    private val tag by lazy {
        icon("Tag", "M21.41,11.58l-9,-9C12.05,2.22 11.55,2 11,2H4c-1.1,0 -2,0.9 -2,2v7c0,0.55 0.22,1.05 0.59,1.42l9,9c0.36,0.36 0.86,0.58 1.41,0.58 0.55,0 1.05,-0.22 1.41,-0.59l7,-7c0.37,-0.36 0.59,-0.86 0.59,-1.41 0,-0.55 -0.23,-1.06 -0.59,-1.42zM5.5,7C4.67,7 4,6.33 4,5.5S4.67,4 5.5,4 7,4.67 7,5.5 6.33,7 5.5,7z")
    }
    private val language by lazy {
        icon("Language", "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM18.92,8h-2.95c-0.32,-1.25 -0.78,-2.45 -1.38,-3.56 1.84,0.63 3.37,1.91 4.33,3.56zM12,4.04c0.83,1.2 1.48,2.53 1.91,3.96h-3.82c0.43,-1.43 1.08,-2.76 1.91,-3.96zM4.26,14C4.1,13.36 4,12.69 4,12s0.1,-1.36 0.26,-2h3.38c-0.08,0.66 -0.14,1.32 -0.14,2 0,0.68 0.06,1.34 0.14,2H4.26zM5.08,16h2.95c0.32,1.25 0.78,2.45 1.38,3.56 -1.84,-0.63 -3.37,-1.9 -4.33,-3.56zM8.03,8H5.08c0.96,-1.66 2.49,-2.93 4.33,-3.56C8.81,5.55 8.35,6.75 8.03,8zM12,19.96c-0.83,-1.2 -1.48,-2.53 -1.91,-3.96h3.82c-0.43,1.43 -1.08,2.76 -1.91,3.96zM14.34,14H9.66c-0.09,-0.66 -0.16,-1.32 -0.16,-2 0,-0.68 0.07,-1.35 0.16,-2h4.68c0.09,0.65 0.16,1.32 0.16,2 0,0.68 -0.07,1.34 -0.16,2zM14.59,19.56c0.6,-1.11 1.06,-2.31 1.38,-3.56h2.95c-0.96,1.65 -2.49,2.93 -4.33,3.56zM16.36,14c0.08,-0.66 0.14,-1.32 0.14,-2 0,-0.68 -0.06,-1.34 -0.14,-2h3.38c0.16,0.64 0.26,1.31 0.26,2s-0.1,1.36 -0.26,2h-3.38z")
    }
    private val palette by lazy {
        icon("Palette", "M12,3c-4.97,0 -9,4.03 -9,9s4.03,9 9,9c0.83,0 1.5,-0.67 1.5,-1.5 0,-0.39 -0.15,-0.74 -0.39,-1.01 -0.23,-0.26 -0.38,-0.61 -0.38,-0.99 0,-0.83 0.67,-1.5 1.5,-1.5H16c2.76,0 5,-2.24 5,-5 0,-4.42 -4.03,-8 -9,-8zM6.5,12c-0.83,0 -1.5,-0.67 -1.5,-1.5S5.67,9 6.5,9 8,9.67 8,10.5 7.33,12 6.5,12zM9.5,8C8.67,8 8,7.33 8,6.5S8.67,5 9.5,5s1.5,0.67 1.5,1.5S10.33,8 9.5,8zM14.5,8c-0.83,0 -1.5,-0.67 -1.5,-1.5S13.67,5 14.5,5s1.5,0.67 1.5,1.5S15.33,8 14.5,8zM17.5,12c-0.83,0 -1.5,-0.67 -1.5,-1.5S16.67,9 17.5,9s1.5,0.67 1.5,1.5 -0.67,1.5 -1.5,1.5z")
    }
    private val download by lazy {
        icon("Download", "M19,9h-4V3H9v6H5l7,7 7,-7zM5,18v2h14v-2H5z")
    }
    private val tune by lazy {
        icon("Tune", "M3,17v2h6v-2H3zM3,5v2h10V5H3zM13,21v-2h8v-2h-8v-2h-2v6h2zM7,9v2H3v2h4v2h2V9H7zM21,13v-2H11v2h10zM15,9h2V7h4V5h-4V3h-2v6z")
    }
    private val text by lazy {
        icon("Text", "M2.5,4v3h5v12h3V7h5V4H2.5zM21.5,9h-9v3h3v7h3v-7h3V9z")
    }
    private val sidebarRight by lazy {
        icon("ViewSidebar", "M16,20H2V4h14V20zM18,8h4V4h-4V8zM18,20h4v-4h-4V20zM18,14h4v-4h-4V14z")
    }
    private val starBorder by lazy {
        icon("StarBorder", "M22,9.24l-7.19,-0.62L12,2 9.19,8.63 2,9.24l5.46,4.73L5.82,21 12,17.27 18.18,21l-1.63,-7.03L22,9.24zM12,15.4l-3.76,2.27 1,-4.28 -3.32,-2.88 4.38,-0.38L12,6.1l1.71,4.04 4.38,0.38 -3.32,2.88 1,4.28L12,15.4z")
    }
    private val subtitles by lazy {
        icon("Subtitles", "M20,4H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V6c0,-1.1 -0.9,-2 -2,-2zM4,12h4v2H4v-2zM14,18H4v-2h10v2zM20,18h-4v-2h4v2zM20,14H10v-2h10v2z")
    }
    private val gridOn by lazy {
        icon("GridOn", "M20,2H4c-1.1,0 -2,0.9 -2,2v16c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4c0,-1.1 -0.9,-2 -2,-2zM8,20H4v-4h4v4zM8,14H4v-4h4v4zM8,8H4V4h4v4zM14,20h-4v-4h4v4zM14,14h-4v-4h4v4zM14,8h-4V4h4v4zM20,20h-4v-4h4v4zM20,14h-4v-4h4v4zM20,8h-4V4h4v4z")
    }
    private val dot by lazy { icon("Dot", "M12,12m-4,0a4,4 0,1 0,8 0a4,4 0,1 0,-8 0") }

    private fun icon(name: String, path: String): ImageVector =
        ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
            .addPath(PathParser().parsePathString(path).toNodes(), fill = SolidColor(Color.Black))
            .build()
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

/// An icon and a title; with `fontSize` the icon is set at that size too.
/// `iconOnly` shows the icon alone, the title its accessible name.
@Composable
fun BernLabel(title: String, icon: ImageVector?, style: TextStyle, fontSize: Float? = null, iconOnly: Boolean = false) {
    // The icon follows the text's size, as a symbol does, and so does the
    // room between them (0.6 of it, 4 to 12).
    val points = fontSize ?: style.fontSize.takeIf { it.isSp }?.value
    val iconModifier = if (points != null) Modifier.size(bernIconSize(points)) else Modifier
    if (iconOnly && icon != null) {
        Icon(icon, contentDescription = title, modifier = iconModifier)
        return
    }
    val gap = points?.let { (it * 0.6f).coerceIn(4f, 12f).dp } ?: 8.dp
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(gap)) {
        if (icon != null) Icon(icon, contentDescription = null, modifier = iconModifier)
        Text(title, style = if (fontSize != null) style.copy(fontSize = fontSize.sp) else style)
    }
}

// MARK: - Buttons

enum class BernProminence { Standard, Prominent, Plain }

/// A Material button; `shape` a capsule of its own height, `fill` across
/// its container.
@Composable
fun BernButton(
    label: String,
    icon: ImageVector?,
    prominence: BernProminence,
    destructive: Boolean = false,
    enabled: Boolean = true,
    style: BernStyle? = null,
    shape: BernShape? = null,
    fill: Boolean = false,
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
    var padding = if (style?.size != null) style.insets else ButtonDefaults.ContentPadding
    val height = shape?.height
    if (height != null) padding = PaddingValues(horizontal = 16.dp, vertical = 0.dp)
    val form = if (shape?.capsule == true) CircleShape else ButtonDefaults.shape
    var sized = modifier
    if (fill) sized = sized.fillMaxWidth()
    if (height != null) sized = sized.height(height)
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { modifier, interaction ->
        when (prominence) {
            BernProminence.Prominent -> Button(
                onClick, modifier, enabled,
                shape = form,
                colors = if (destructive) ButtonDefaults.buttonColors(containerColor = error) else ButtonDefaults.buttonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
            BernProminence.Plain -> TextButton(
                onClick, modifier, enabled,
                shape = form,
                colors = if (destructive) ButtonDefaults.textButtonColors(contentColor = error) else ButtonDefaults.textButtonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
            BernProminence.Standard -> FilledTonalButton(
                onClick, modifier, enabled,
                shape = form,
                colors = if (destructive) ButtonDefaults.filledTonalButtonColors(contentColor = error) else ButtonDefaults.filledTonalButtonColors(),
                contentPadding = padding,
                interactionSource = interaction,
                content = content,
            )
        }
    }
    if (style == null) {
        material(sized, null)
    } else {
        BernStyledButton(style, enabled, sized, onClick, material, content)
    }
}

/// An icon button named by `label`. With a `diameter` it is a round tonal
/// button of that size, `prominent` a filled one, `plain` has no
/// container; `iconSize` sets its icon at that many points.
@Composable
fun BernIconButton(
    label: String,
    icon: ImageVector?,
    enabled: Boolean,
    modifier: Modifier = Modifier,
    style: BernStyle? = null,
    iconSize: Float? = null,
    diameter: Dp? = null,
    prominence: BernProminence = BernProminence.Standard,
    onClick: () -> Unit,
) {
    val iconModifier = if (iconSize != null) Modifier.size(bernIconSize(iconSize)) else Modifier
    val content: @Composable RowScope.() -> Unit = {
        Icon(icon ?: BernIcons.get(""), contentDescription = label, modifier = iconModifier)
    }
    val material: @Composable (Modifier, MutableInteractionSource?) -> Unit = { modifier, interaction ->
        val sized = if (diameter != null) modifier.size(diameter) else modifier
        val glyph: @Composable () -> Unit = { Icon(icon ?: BernIcons.get(""), contentDescription = label, modifier = iconModifier) }
        when {
            prominence == BernProminence.Prominent ->
                FilledIconButton(onClick = onClick, enabled = enabled, modifier = sized, interactionSource = interaction, content = glyph)
            diameter != null && prominence == BernProminence.Standard ->
                FilledTonalIconButton(onClick = onClick, enabled = enabled, modifier = sized, interactionSource = interaction, content = glyph)
            else -> IconButton(onClick = onClick, enabled = enabled, modifier = sized, interactionSource = interaction, content = glyph)
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
    width: Dp? = null,
    height: Dp? = null,
    fontSize: Float? = null,
) {
    // A field of its own size stands in the page as it is.
    if (width != null || height != null || fontSize != null) {
        BernSizedSearchField(value, onChange, prompt, submit, modifier, width, height, fontSize)
        return
    }
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
    labelHidden: Boolean = false,
    modifier: Modifier = Modifier,
) {
    // Without its label shown, the control alone, named by it.
    if (labelHidden) {
        val named = modifier.semantics { contentDescription = label }
        if (checkbox) Checkbox(checked = value, onCheckedChange = onChange, modifier = named)
        else Switch(checked = value, onCheckedChange = onChange, modifier = named)
        return
    }
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
    labelHidden: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val steps = if (step != null && step > 0) maxOf(0, Math.round((max - min) / step).toInt() - 1) else 0
    Column(modifier.fillMaxWidth().semantics { if (labelHidden) contentDescription = label }) {
        if (!labelHidden) Text(label, style = MaterialTheme.typography.bodyMedium)
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

/// A slider showing engine state: dragging moves only the slider, letting
/// go runs `commit` with the new value; keys and accessibility actions
/// commit at once.
@Composable
fun BernControlledSlider(
    label: String,
    value: Double,
    commit: (Double) -> Unit,
    min: Double,
    max: Double,
    step: Double?,
    minIcon: ImageVector?,
    maxIcon: ImageVector?,
    labelHidden: Boolean = false,
    modifier: Modifier = Modifier,
) {
    var preview by remember { mutableStateOf<Double?>(null) }
    BernSlider(
        label, preview ?: value, { preview = it }, min, max, step,
        { preview?.let(commit); preview = null },
        minIcon, maxIcon, labelHidden, modifier,
    )
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
    labelHidden: Boolean = false,
    shape: BernShape? = null,
) {
    // A capsule of its own height: the chosen option on a tonal button,
    // opening a menu of the others.
    if (shape != null && style != "segmented" && style != "radio" && style != "inline") {
        var expanded by remember { mutableStateOf(false) }
        Box(modifier) {
            FilledTonalButton(
                onClick = { expanded = true },
                shape = CircleShape,
                contentPadding = PaddingValues(horizontal = 14.dp, vertical = 0.dp),
                modifier = (if (shape.height != null) Modifier.height(shape.height) else Modifier)
                    .semantics { contentDescription = label },
            ) {
                Text(optionLabel(selected), maxLines = 1)
                Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
            }
            DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                for (option in options) {
                    DropdownMenuItem(
                        text = { Text(optionLabel(option)) },
                        leadingIcon = if (option == selected) {
                            { Icon(Icons.Filled.Check, contentDescription = null) }
                        } else {
                            null
                        },
                        onClick = {
                            onSelect(option)
                            expanded = false
                        },
                    )
                }
            }
        }
        return
    }
    when (style) {
        "segmented" -> Column(modifier.semantics { if (labelHidden) contentDescription = label }) {
            if (!labelHidden) Text(label, style = MaterialTheme.typography.bodyMedium)
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
        "radio", "inline" -> Column(modifier.semantics { if (labelHidden) contentDescription = label }) {
            if (!labelHidden) Text(label, style = MaterialTheme.typography.bodyMedium)
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
                    label = if (labelHidden) null else { { Text(label) } },
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
/// follows the picture's aspect ratio, or `aspect` when given; with only
/// `aspect` it takes its container's width. `corner` rounds it; a
/// `template` takes the color of the text around it through its alpha. A
/// label makes it an image for assistive technologies; without one it is
/// decorative.
@Composable
fun BernImage(
    name: String,
    label: String?,
    width: Dp?,
    height: Dp?,
    fit: ContentScale,
    corner: Dp? = null,
    template: Boolean = false,
    aspect: Float? = null,
) {
    val bitmap = remember(name) {
        BitmapFactory.decodeFile(File(BernResources.root, name).absolutePath)?.asImageBitmap()
    } ?: return
    val ratio = aspect ?: (bitmap.width.toFloat() / maxOf(1, bitmap.height))
    Image(
        bitmap,
        contentDescription = label,
        modifier = bernPictureFrame(width, height, ratio, aspect != null, corner),
        contentScale = fit,
        colorFilter = if (template) ColorFilter.tint(LocalContentColor.current, BlendMode.SrcIn) else null,
    )
}

/// The frame of a picture: its declared sizes, the other side by `ratio`,
/// the container's width with only a proportion, and its rounded corners.
fun bernPictureFrame(width: Dp?, height: Dp?, ratio: Float, proportioned: Boolean, corner: Dp?): Modifier {
    val frame = when {
        width != null && height != null -> Modifier.size(width, height)
        width != null -> Modifier.width(width).height(width / ratio)
        height != null -> Modifier.height(height).width(height * ratio)
        proportioned -> Modifier.fillMaxWidth().aspectRatio(ratio)
        else -> Modifier.fillMaxWidth()
    }
    return if (corner != null) frame.clip(RoundedCornerShape(corner)) else frame
}

// MARK: - Groups and layout

/// Equal columns no narrower than `minWidth`, as many as fit, `gap`
/// apart and rows `rowGap` apart (`gap` unless given); children fill them
/// row by row. With `cellRatio` every row is the column width ÷ `cellRatio`
/// \+ `cellExtra` tall, its children at the top; taller content runs into
/// the gap below.
@Composable
fun BernGrid(
    minWidth: Dp,
    gap: Dp,
    rowGap: Dp? = null,
    cellRatio: Float? = null,
    cellExtra: Dp = 0.dp,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Layout(content, modifier.fillMaxWidth()) { measurables, constraints ->
        val spacing = gap.roundToPx()
        val rowSpacing = rowGap?.roundToPx() ?: spacing
        val width = constraints.maxWidth
        val columns = maxOf(1, (width + spacing) / (minWidth.roundToPx() + spacing))
        val cell = maxOf(0, (width - spacing * (columns - 1)) / columns)
        val placeables = measurables.map {
            it.measure(Constraints(minWidth = cell, maxWidth = cell, minHeight = 0, maxHeight = Constraints.Infinity))
        }
        val rows = placeables.chunked(columns)
        val fixed = cellRatio?.takeIf { it > 0f }?.let { (cell / it + cellExtra.toPx()).toInt() }
        val heights = rows.map { row -> fixed ?: row.maxOf { it.height } }
        val total = heights.sum() + rowSpacing * maxOf(0, rows.size - 1)
        layout(width, total) {
            var y = 0
            rows.forEachIndexed { index, row ->
                row.forEachIndexed { column, placeable -> placeable.place(column * (cell + spacing), y) }
                y += heights[index] + rowSpacing
            }
        }
    }
}

/// One layout by the available width and the breakpoints of `[layouts]`.
/// A size without a layout of its own shows the next larger declared one,
/// else the next smaller.
@Composable
fun BernAdaptive(
    target: String?,
    compact: (@Composable () -> Unit)?,
    medium: (@Composable () -> Unit)?,
    expanded: (@Composable () -> Unit)?,
    large: (@Composable () -> Unit)?,
) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val size = BernHost.layouts.sizeOf(maxWidth.value.toDouble())
        if (target != null) BernControl(target, "adaptive", "", actions = BernControlActions(value = { size }))
        val layouts = listOf("compact" to compact, "medium" to medium, "expanded" to expanded, "large" to large)
        val at = layouts.indexOfFirst { it.first == size }
        val shown = (layouts.drop(at) + layouts.take(at).reversed()).firstOrNull { it.second != null }
        if (shown != null) key(shown.first) { shown.second?.invoke() }
    }
}

// MARK: - Menus

/// A button opening a menu of commands. With an icon only the icon shows
/// (`iconOnly`, or an icon in a top bar); with a `shape` height it is a
/// round tonal button of that diameter, or a tonal capsule with its label.
@Composable
fun BernMenu(
    label: String,
    icon: ImageVector?,
    modifier: Modifier = Modifier,
    iconOnly: Boolean = false,
    shape: BernShape? = null,
    items: @Composable (dismiss: () -> Unit) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier) {
        val height = shape?.height
        if (icon != null && (iconOnly || shape == null)) {
            if (height != null) {
                FilledTonalIconButton(onClick = { expanded = true }, modifier = Modifier.size(height)) {
                    Icon(icon, contentDescription = label)
                }
            } else {
                IconButton(onClick = { expanded = true }) { Icon(icon, contentDescription = label) }
            }
        } else if (shape != null) {
            FilledTonalButton(
                onClick = { expanded = true },
                shape = CircleShape,
                contentPadding = PaddingValues(horizontal = 14.dp, vertical = 0.dp),
                modifier = if (height != null) Modifier.height(height) else Modifier,
            ) {
                if (icon != null) {
                    Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                }
                Text(label, maxLines = 1)
            }
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
    showingDetail: MutableState<Boolean>,
    sidebarWidth: Dp = 280.dp,
    header: (@Composable () -> Unit)? = null,
    sidebar: @Composable (showDetail: () -> Unit) -> Unit,
    footer: (@Composable () -> Unit)? = null,
    detail: @Composable () -> Unit,
) {
    var showingDetail by showingDetail
    // The sidebar's own top and bottom areas, such as the application's
    // name and the account, stay put while the destinations scroll.
    val list: @Composable (() -> Unit) -> Unit = { showDetail ->
        Column(Modifier.fillMaxHeight()) {
            if (header != null) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) { header() }
            }
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                sidebar(showDetail)
            }
            if (footer != null) {
                HorizontalDivider()
                Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) { footer() }
            }
        }
    }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxWidth >= 600.dp) {
            BernChrome(title = title)
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.width(sidebarWidth).fillMaxHeight()) { list {} }
                VerticalDivider()
                Box(Modifier.weight(1f).fillMaxHeight()) { detail() }
            }
        } else if (showingDetail) {
            BernChrome(title = title, back = { showingDetail = false })
            Box(Modifier.fillMaxSize()) { detail() }
        } else {
            Box(Modifier.fillMaxSize()) { list { showingDetail = true } }
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
fun BernSidebarItem(label: String, icon: ImageVector?, selected: Boolean, badge: Long? = null, onClick: () -> Unit) {
    NavigationDrawerItem(
        label = { Text(label) },
        icon = icon?.let { { Icon(it, contentDescription = null) } },
        // A live count beside the page, hidden at zero.
        badge = if (badge != null && badge > 0) {
            { Text("$badge") }
        } else {
            null
        },
        selected = selected,
        onClick = onClick,
    )
}

/// Tab pages above a navigation bar.
@Composable
fun BernTabs(
    content: @Composable () -> Unit,
    accessory: (@Composable () -> Unit)? = null,
    bar: @Composable RowScope.() -> Unit,
) {
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.weight(1f).fillMaxWidth()) { content() }
        accessory?.invoke()
        NavigationBar(content = bar)
    }
}

/// `TabAccessory`: a bar on a raised surface above the navigation bar,
/// as a player shows what is playing over every page.
@Composable
fun BernTabAccessory(content: @Composable () -> Unit) {
    Surface(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
        shape = RoundedCornerShape(24.dp),
        tonalElevation = 3.dp,
        shadowElevation = 2.dp,
    ) {
        Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) { content() }
    }
}

@Composable
fun RowScope.BernTabItem(label: String, icon: ImageVector?, selected: Boolean, onClick: () -> Unit) {
    NavigationBarItem(
        selected = selected,
        onClick = onClick,
        icon = { Icon(icon ?: BernIcons.get(""), contentDescription = null) },
        label = { Text(label) },
    )
}

// MARK: - Queries

/// What a query subscription shows, as the session runtime reports it.
sealed class BernQueryPhase<out T> {
    data object Idle : BernQueryPhase<Nothing>()
    data object Loading : BernQueryPhase<Nothing>()
    data class Ready<T>(val data: T) : BernQueryPhase<T>()
    /// New parameters are loading; the previous data stays visible.
    data class Refreshing<T>(val data: T) : BernQueryPhase<T>()
    /// The last request failed; data from the last success stays visible.
    data class Failed<T>(val previous: T?, val code: String, val message: String) : BernQueryPhase<T>()
}

/// One subscription to a catalog query. Every parameter change is a new
/// request with a new version; the session runtime cancels the previous
/// request and forwards only the answer for the current version, and
/// states older than the last one shown are ignored here as well.
class BernQuery<T>(private val index: Int, private val decode: (BernReader) -> T) {
    var phase by mutableStateOf<BernQueryPhase<T>>(BernQueryPhase.Idle)
        private set
    var count: ((T) -> Int)? = null
    private var version = 0L
    private var subscription: Long? = null
    private var generation = 0L
    private var requested: ByteArray? = null
    private val control = Any()

    /// After each composition: open the subscription for the window and
    /// request the parameters it shows, if they changed.
    fun want(window: BernWindowContext, params: ByteArray, target: String) {
        if (window.generation == 0L) return
        try {
            if (subscription != null && generation != window.generation) close()
            if (subscription == null) {
                subscription = BernHost.queryOpen(window, index) { receive(it) }
                generation = window.generation
                BernHost.controls.register(
                    control,
                    BernControlEntry(
                        target, window.id, "query", "", true,
                        BernControlActions(activate = { retry() }, value = { summary }),
                    ),
                )
            }
            if (!params.contentEquals(requested)) request(params)
        } catch (problem: BernCommandFailure) {
            phase = BernQueryPhase.Failed(null, problem.code, problem.message)
        }
    }

    private fun request(params: ByteArray) {
        val subscription = subscription ?: return
        requested = params
        val reader = BernReader(BernNative.queryRequest(BernHost.handle, subscription, params))
        if (reader.u8() != 0) {
            phase = BernQueryPhase.Failed(null, reader.text(), reader.text())
            return
        }
        receive(reader)
    }

    /// Ask again with the current parameters.
    fun retry() {
        requested?.let(::request)
    }

    fun close() {
        subscription?.let(BernHost::queryClose)
        subscription = null
        requested = null
        version = 0
        BernHost.controls.remove(control)
    }

    private fun receive(bytes: ByteArray) = receive(BernReader(bytes))

    private fun receive(reader: BernReader) {
        try {
            val next = reader.u64()
            if (next < version) return
            val phase: BernQueryPhase<T> = when (reader.u8()) {
                0 -> BernQueryPhase.Idle
                1 -> BernQueryPhase.Loading
                2 -> BernQueryPhase.Ready(decode(reader))
                3 -> BernQueryPhase.Refreshing(decode(reader))
                4 -> {
                    val previous = if (reader.u8() == 1) decode(reader) else null
                    BernQueryPhase.Failed(previous, reader.text(), reader.text())
                }
                else -> throw BernDecodeError("unknown query phase")
            }
            version = next
            this.phase = phase
        } catch (error: Exception) {
            phase = BernQueryPhase.Failed(null, "invalid_state", error.toString())
        }
    }

    /// Machine-readable state for `ui.inspect`.
    val summary: String
        get() {
            fun size(data: T): String = count?.let { " (${it(data)})" } ?: ""
            return when (val phase = phase) {
                is BernQueryPhase.Idle -> "idle"
                is BernQueryPhase.Loading -> "loading v$version"
                is BernQueryPhase.Ready -> "ready v$version${size(phase.data)}"
                is BernQueryPhase.Refreshing -> "refreshing v$version${size(phase.data)}"
                is BernQueryPhase.Failed ->
                    "failed v$version ${phase.code}: ${phase.message}${phase.previous?.let(::size) ?: ""}"
            }
        }
}

/// A `Query` declaration: owns its subscription and shows each state with
/// standard components. `shown` maps the data to what the template shows,
/// or null when it is empty.
@Composable
fun <T, S> BernQueryView(
    query: Int,
    target: String,
    params: ByteArray,
    loading: String,
    empty: String?,
    emptyIcon: String,
    failed: String,
    decode: (BernReader) -> T,
    count: ((T) -> Int)?,
    shown: (T) -> S?,
    content: @Composable (S) -> Unit,
) {
    val window = LocalBernWindow.current
    val state = remember { BernQuery(query, decode).also { it.count = count } }
    SideEffect { state.want(window, params, target) }
    DisposableEffect(state) { onDispose { state.close() } }
    val presented: @Composable (T) -> Unit = { data ->
        val value = shown(data)
        if (value != null) {
            content(value)
        } else if (empty != null) {
            BernUnavailable(empty, BernIcons.get(emptyIcon), null, null)
        }
    }
    when (val phase = state.phase) {
        is BernQueryPhase.Idle, is BernQueryPhase.Loading -> BernQueryProgress(loading)
        is BernQueryPhase.Ready -> presented(phase.data)
        is BernQueryPhase.Refreshing -> {
            BernQueryProgress(loading)
            presented(phase.data)
        }
        is BernQueryPhase.Failed -> {
            val previous = phase.previous
            if (previous != null) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Filled.Warning, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Column(Modifier.weight(1f)) {
                        Text(failed)
                        Text(phase.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    TextButton(onClick = { state.retry() }) { Text(BernText.string("bern.query.retry")) }
                }
                presented(previous)
            } else {
                BernUnavailable(failed, Icons.Filled.Warning, phase.message) { state.retry() }
            }
        }
    }
}

/// A small progress row shown while a query loads or refreshes.
@Composable
fun BernQueryProgress(text: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/// Nothing to show yet: an icon, a title, why, and a way to try again.
@Composable
fun BernUnavailable(title: String, icon: ImageVector, message: String?, retry: (() -> Unit)?) {
    Column(
        Modifier.fillMaxWidth().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(40.dp))
        Text(title, style = MaterialTheme.typography.titleMedium)
        if (message != null) {
            Text(message, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (retry != null) {
            TextButton(onClick = retry) { Text(BernText.string("bern.query.retry")) }
        }
    }
}

/// A picker in the top bar: the chosen option, opening a menu of the
/// others, as the bar has room for one line.
@Composable
fun <T> BernToolbarPicker(
    label: String,
    options: List<T>,
    selected: T,
    optionLabel: (T) -> String,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier) {
        TextButton(onClick = { expanded = true }) {
            Text(optionLabel(selected))
            Icon(Icons.Filled.ArrowDropDown, contentDescription = label)
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            for (option in options) {
                DropdownMenuItem(
                    text = { Text(optionLabel(option)) },
                    leadingIcon = if (option == selected) {
                        { Icon(Icons.Filled.Check, contentDescription = null) }
                    } else {
                        null
                    },
                    onClick = {
                        onSelect(option)
                        expanded = false
                    },
                )
            }
        }
    }
}
