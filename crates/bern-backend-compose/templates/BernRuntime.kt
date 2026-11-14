// Bern Compose runtime. Generated build input; do not edit.
//
// Connects the generated composables to the in-process Rust runtime: it
// decodes typed values, mirrors engine state into snapshot state, mirrors
// window-scoped session drafts, and runs session commands from external
// entries through the same control bindings the user operates. It holds no
// business logic; every surface keeps the Material look of the system.

package dev.bern.runtime

import android.content.Context
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import androidx.activity.compose.BackHandler
import androidx.annotation.Keep
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.height
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
// @bern:wear-skip-start
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
// @bern:wear-skip-end
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.inset
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.findRootCoordinates
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onPlaced
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import java.io.File
import java.util.Locale
import org.json.JSONObject

// MARK: - Binary values

class BernDecodeError(message: String) : Exception(message)

class BernWriter {
    private val out = java.io.ByteArrayOutputStream()

    fun u8(value: Int) = out.write(value and 0xff)

    fun bool(value: Boolean) = u8(if (value) 1 else 0)

    fun u32(value: Int) {
        for (index in 0 until 4) out.write((value ushr (8 * index)) and 0xff)
    }

    fun u64(value: Long) {
        for (index in 0 until 8) out.write(((value ushr (8 * index)) and 0xff).toInt())
    }

    fun i64(value: Long) = u64(value)

    fun f64(value: Double) = u64(value.toRawBits())

    fun text(value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        u32(bytes.size)
        out.write(bytes)
    }

    inline fun <T> optional(value: T?, encode: (T) -> Unit) {
        if (value == null) {
            u8(0)
        } else {
            u8(1)
            encode(value)
        }
    }

    inline fun <T> list(values: List<T>, encode: (T) -> Unit) {
        u32(values.size)
        values.forEach(encode)
    }

    fun bytes(): ByteArray = out.toByteArray()
}

class BernReader(private val bytes: ByteArray) {
    private var position = 0

    val isAtEnd: Boolean get() = position == bytes.size

    private fun take(count: Int): Int {
        if (count < 0 || position + count > bytes.size) throw BernDecodeError("unexpected end of data")
        val start = position
        position += count
        return start
    }

    fun u8(): Int = bytes[take(1)].toInt() and 0xff

    fun bool(): Boolean = when (u8()) {
        0 -> false
        1 -> true
        else -> throw BernDecodeError("invalid bool")
    }

    fun u32(): Int {
        val start = take(4)
        var value = 0
        for (index in 0 until 4) value = value or ((bytes[start + index].toInt() and 0xff) shl (8 * index))
        return value
    }

    fun u64(): Long {
        val start = take(8)
        var value = 0L
        for (index in 0 until 8) value = value or ((bytes[start + index].toLong() and 0xff) shl (8 * index))
        return value
    }

    fun i64(): Long = u64()

    fun f64(): Double = Double.fromBits(u64())

    fun text(): String {
        val count = u32()
        val start = take(count)
        return String(bytes, start, count, Charsets.UTF_8)
    }

    inline fun <T> optional(decode: () -> T): T? = when (u8()) {
        0 -> null
        1 -> decode()
        else -> throw BernDecodeError("invalid optional")
    }

    inline fun <T> list(decode: () -> T): List<T> {
        val count = u32()
        if (count < 0) throw BernDecodeError("invalid list length")
        return List(count) { decode() }
    }
}

// MARK: - Text

/// Enums whose variants declare localized labels.
interface BernLabeled {
    val bernLabel: String
}

/// The interface language. Every localized text reads its strings through
/// it, so switching re-renders them in one update while drafts, navigation
/// and windows stay.
object BernLanguage {
    var supported: List<String> = emptyList()
        private set
    var current by mutableStateOf("en")
        private set

    /// Where the language chosen in the app is kept: it stays through a restart.
    private var kept: android.content.SharedPreferences? = null

    internal fun setup(context: Context, supported: List<String>, default: String) {
        this.supported = supported
        kept = context.getSharedPreferences("bern", Context.MODE_PRIVATE)
        current = kept?.getString("language", null)?.takeIf { it in supported } ?: initial(supported) ?: default
    }

    /// The first system language the app supports: by full tag, by
    /// language and script (Chinese regions imply theirs), then by language.
    private fun initial(supported: List<String>): String? {
        val system = android.os.LocaleList.getDefault()
        for (index in 0 until system.size()) {
            val locale = system.get(index)
            val script = locale.script.ifEmpty {
                when {
                    locale.language != "zh" -> ""
                    locale.country in listOf("TW", "HK", "MO") -> "Hant"
                    else -> "Hans"
                }
            }
            val candidates = listOf(
                locale.toLanguageTag(),
                if (script.isEmpty()) locale.language else "${locale.language}-$script",
                locale.language,
            )
            for (candidate in candidates) {
                supported.firstOrNull { it.equals(candidate, ignoreCase = true) }?.let { return it }
            }
        }
        return null
    }

    /// Switch to a supported language; false for any other tag.
    fun select(language: String): Boolean {
        if (language !in supported) return false
        current = language
        kept?.edit()?.putString("language", language)?.apply()
        return true
    }

    val locale: Locale get() = Locale.forLanguageTag(current)

    /// A language's own name for itself, such as “简体中文” or “English”.
    /// Chinese is named by its script, as the other targets name it.
    fun autonym(language: String): String {
        when (language) {
            "zh-Hans" -> return "简体中文"
            "zh-Hant" -> return "繁體中文"
        }
        val locale = Locale.forLanguageTag(language)
        return locale.getDisplayName(locale).replaceFirstChar { it.titlecase(locale) }
    }
}

/// A value a translated text shows.
sealed class BernArg {
    data class Text(val text: String) : BernArg()

    /// A number, and the way Bern writes it.
    data class Number(val value: Double, val text: String) : BernArg()

    data class Date(val year: Long, val month: Long, val day: Long) : BernArg()
}

/// One part of a parsed text.
sealed class BernPiece {
    data class Text(val text: String) : BernPiece()
    data class Value(val name: String) : BernPiece()

    /// `#` in a plural branch: the number.
    data object Hash : BernPiece()
    data class Plural(val name: String, val branches: List<Pair<String, List<BernPiece>>>) : BernPiece()
    data class Date(val name: String, val pattern: String) : BernPiece()
    data class Duration(val name: String, val pattern: String) : BernPiece()
}

/// Parses a translated text. The compiler checked the format, so the
/// parser trusts its shape.
class BernTextParser(private val chars: String) {
    private var at = 0

    private fun word(stops: String): String {
        val start = at
        while (at < chars.length && chars[at] !in stops) at += 1
        return chars.substring(start, at).trim()
    }

    /// Pieces up to the end, or up to the `}` that closes a plural branch.
    fun message(branch: Boolean): List<BernPiece> {
        val pieces = mutableListOf<BernPiece>()
        fun push(c: Char) {
            val last = pieces.lastOrNull()
            if (last is BernPiece.Text) {
                pieces[pieces.size - 1] = BernPiece.Text(last.text + c)
            } else {
                pieces.add(BernPiece.Text(c.toString()))
            }
        }
        while (at < chars.length) {
            val c = chars[at]
            val next = chars.getOrNull(at + 1)
            when {
                c == '{' && next == '{' -> { push('{'); at += 2 }
                c == '}' && branch -> return pieces
                c == '}' && next == '}' -> { push('}'); at += 2 }
                c == '#' && branch -> { pieces.add(BernPiece.Hash); at += 1 }
                c == '{' -> { at += 1; pieces.add(placeholder()) }
                else -> { push(c); at += 1 }
            }
        }
        return pieces
    }

    private fun placeholder(): BernPiece {
        val name = word(",{}")
        if (at >= chars.length || chars[at] != ',') {
            at += 1
            return BernPiece.Value(name)
        }
        at += 1
        val kind = word(",{}")
        at += 1
        if (kind != "plural") {
            val pattern = word("{}")
            at += 1
            return if (kind == "date") BernPiece.Date(name, pattern) else BernPiece.Duration(name, pattern)
        }
        val branches = mutableListOf<Pair<String, List<BernPiece>>>()
        while (true) {
            while (at < chars.length && chars[at].isWhitespace()) at += 1
            if (at >= chars.length || chars[at] == '}') {
                at += 1
                return BernPiece.Plural(name, branches)
            }
            val selector = word("{}")
            at += 1
            branches.add(selector to message(branch = true))
            at += 1
        }
    }
}

/// Localized strings and Bern's basic, locale-independent formatting.
object BernText {
    private var tables: Map<String, Map<String, String>> = emptyMap()

    /// Read the string tables the build put in the assets, one JSON object
    /// per language.
    internal fun load(context: Context, languages: List<String>) {
        tables = languages.associateWith { language ->
            val json = context.assets.open("bern/texts/$language.json").use {
                JSONObject(it.readBytes().toString(Charsets.UTF_8))
            }
            json.keys().asSequence().associateWith { json.getString(it) }
        }
    }

    fun string(key: String): String = tables[BernLanguage.current]?.get(key) ?: key

    /// A framework text with positional `%s` arguments.
    fun format(key: String, arguments: List<String>): String =
        String.format(BernLanguage.locale, string(key), *arguments.toTypedArray())

    /// A translated text with its arguments, in the current language:
    /// text and numbers as they are, plural branches by the number's
    /// category, dates and durations by their patterns. The format is the
    /// one the compiler checks (`bern_protocol::format`).
    fun format(key: String, arguments: Map<String, BernArg>): String {
        val template = string(key)
        val pieces = messages.getOrPut(template) { BernTextParser(template).message(branch = false) }
        return write(pieces, BernLanguage.current, arguments, hash = null)
    }

    /// Each template read once: a page formats its texts again every time
    /// it is composed.
    private val messages = HashMap<String, List<BernPiece>>()

    fun arg(value: String): BernArg = BernArg.Text(value)

    fun arg(value: Long): BernArg = BernArg.Number(value.toDouble(), value.toString())

    fun arg(value: Int): BernArg = BernArg.Number(value.toDouble(), value.toString())

    fun arg(value: Double): BernArg = BernArg.Number(value, text(value))

    fun arg(value: Boolean): BernArg = BernArg.Text(value.toString())

    fun arg(value: BernLabeled): BernArg = BernArg.Text(value.bernLabel)

    private fun write(
        pieces: List<BernPiece>, language: String, arguments: Map<String, BernArg>, hash: String?,
    ): String {
        val out = StringBuilder()
        for (piece in pieces) {
            when (piece) {
                is BernPiece.Text -> out.append(piece.text)
                BernPiece.Hash -> out.append(hash ?: "")
                is BernPiece.Value -> when (val arg = arguments[piece.name]) {
                    is BernArg.Text -> out.append(arg.text)
                    is BernArg.Number -> out.append(arg.text)
                    is BernArg.Date -> out.append("${arg.year}-${pad2(arg.month)}-${pad2(arg.day)}")
                    null -> {}
                }
                is BernPiece.Plural -> {
                    val (value, shown) = number(arguments[piece.name])
                    val exact = "=" + text(value)
                    val category = pluralCategory(language, value)
                    val chosen = piece.branches.firstOrNull { it.first == exact }
                        ?: piece.branches.firstOrNull { it.first == category }
                        ?: piece.branches.firstOrNull { it.first == "other" }
                    if (chosen != null) out.append(write(chosen.second, language, arguments, shown))
                }
                is BernPiece.Date -> (arguments[piece.name] as? BernArg.Date)?.let {
                    out.append(formatDate(piece.pattern, it.year, it.month, it.day))
                }
                is BernPiece.Duration ->
                    out.append(formatDuration(piece.pattern, number(arguments[piece.name]).first))
            }
        }
        return out.toString()
    }

    private fun number(arg: BernArg?): Pair<Double, String> = when (arg) {
        is BernArg.Number -> arg.value to arg.text
        is BernArg.Text -> (arg.text.toDoubleOrNull() ?: 0.0) to arg.text
        else -> 0.0 to ""
    }

    private fun pluralCategory(language: String, number: Double): String {
        val base = language.split('-', '_').first()
        val oneForm = base in listOf("zh", "ja", "ko", "vi", "th", "id", "ms")
        return if (!oneForm && number == 1.0) "one" else "other"
    }

    private fun pad2(value: Long): String = if (value in 0..9) "0$value" else value.toString()

    /// Runs of one pattern letter, and the characters between them.
    private fun runs(pattern: String): List<Pair<Char, Int>> {
        val out = mutableListOf<Pair<Char, Int>>()
        for (c in pattern) {
            val last = out.lastOrNull()
            if (last != null && last.first == c && (c in 'a'..'z' || c in 'A'..'Z')) {
                out[out.size - 1] = c to last.second + 1
            } else {
                out.add(c to 1)
            }
        }
        return out
    }

    /// The day of the week, 0 for Monday.
    private fun weekday(year: Long, month: Long, day: Long): Long {
        val offsets = longArrayOf(0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4)
        val m = month.coerceIn(1, 12)
        val y = if (m < 3) year - 1 else year
        val sundayFirst = Math.floorMod(
            y + Math.floorDiv(y, 4L) - Math.floorDiv(y, 100L) + Math.floorDiv(y, 400L) + offsets[(m - 1).toInt()] + day,
            7L,
        )
        return (sundayFirst + 6) % 7
    }

    private fun formatDate(pattern: String, year: Long, month: Long, day: Long): String {
        val out = StringBuilder()
        for ((c, count) in runs(pattern)) {
            when {
                c == 'y' && count == 2 -> out.append(pad2(Math.floorMod(year, 100L)))
                c == 'y' -> out.append(year)
                c == 'M' && count == 1 -> out.append(month)
                c == 'M' && count == 2 -> out.append(pad2(month))
                c == 'M' && count == 3 -> out.append(string("bern.date.month.$month"))
                c == 'M' -> out.append(string("bern.date.month_long.$month"))
                c == 'd' && count == 1 -> out.append(day)
                c == 'd' -> out.append(pad2(day))
                c == 'E' && count == 3 -> out.append(string("bern.date.weekday.${weekday(year, month, day) + 1}"))
                c == 'E' -> out.append(string("bern.date.weekday_long.${weekday(year, month, day) + 1}"))
                else -> repeat(count) { out.append(c) }
            }
        }
        return out.toString()
    }

    /// Seconds as `h`, `m` and `s` fields; the largest field written takes
    /// what the larger ones would hold.
    private fun formatDuration(pattern: String, seconds: Double): String {
        val total = if (seconds.isFinite()) Math.floor(maxOf(0.0, seconds)).toLong() else 0L
        val hours = 'h' in pattern
        val minutes = 'm' in pattern
        val out = StringBuilder()
        for ((c, count) in runs(pattern)) {
            val value = when (c) {
                'h' -> total / 3600
                'm' -> if (hours) total % 3600 / 60 else total / 60
                's' -> if (hours || minutes) total % 60 else total
                else -> null
            }
            if (value != null) {
                out.append(if (count == 2) pad2(value) else value.toString())
            } else {
                repeat(count) { out.append(c) }
            }
        }
        return out.toString()
    }

    fun text(value: String): String = value

    fun text(value: Long): String = value.toString()

    fun text(value: Boolean): String = value.toString()

    fun text(value: Double): String =
        if (value == Math.rint(value) && Math.abs(value) < 1e15) value.toLong().toString() else value.toString()

    fun text(value: BernLabeled): String = value.bernLabel
}

// MARK: - Session state

/// Implemented by generated session state classes.
abstract class BernSessionState(val index: Int) {
    var window: BernWindowContext? = null

    abstract fun load(reader: BernReader)
}

/// What a window's top bar shows: titles, a way back and actions that
/// views inside the window declare. The deepest declaration wins.
class BernChromeEntry {
    var title by mutableStateOf<String?>(null)
    var back by mutableStateOf<(() -> Unit)?>(null)
    /// Items at the leading end, after the way back.
    var leading by mutableStateOf<(@Composable RowScope.() -> Unit)?>(null)
    var actions by mutableStateOf<(@Composable RowScope.() -> Unit)?>(null)
}

/// One window instance: its kind, generation and session drafts.
class BernWindowContext(
    val kind: Int,
    val kindID: String,
    val isSettings: Boolean,
    val sessions: List<BernSessionState>,
) {
    init {
        for (session in sessions) session.window = this
    }

    var generation by mutableStateOf(0L)
        private set
    var id by mutableStateOf("")
        private set
    /// How many `under_bars` layers are shown: while any is, the app bar
    /// lets them show through.
    var underBars by mutableStateOf(0)
    /// The window's declared close check, set by the generated window root.
    var closeCheck: BernCloseCheck? = null
    /// The close confirmation shown as this window's dialog. While it is
    /// pending, the window's controls are blocked for every entry.
    var pendingClose by mutableStateOf<BernPendingClose?>(null)
    /// Whether the window had no input for its `idle_after` seconds.
    val fullscreen = true
    var idle by mutableStateOf(false)
    /// The `idle_area`s on screen, and those a mouse rests over, each with
    /// its seconds.
    internal val idleAreas = mutableStateMapOf<Any, Double>()
    internal val idleAreasHovered = mutableStateMapOf<Any, Double>()
    /// Whether the pointer that came last is a mouse: only a mouse rests
    /// over an `idle_area`.
    internal var mouse by mutableStateOf(false)

    /// How long the window waits before it rests: `idleAfter`, or with
    /// `idle_area`s on screen and a mouse, the seconds of the area it
    /// rests over; null when it rests over none, or is outside the window.
    internal fun idleWait(idleAfter: Double): Double? =
        if (mouse && idleAreas.isNotEmpty()) idleAreasHovered.values.minOrNull() else idleAfter
    /// The window's width in dp once laid out; before that, an estimate
    /// from the screen or the declared size.
    var measuredWidth by mutableStateOf<Double?>(null)
    internal var estimatedWidth = 0.0
    /// The window's height in dp once laid out; before that, the screen's.
    var measuredHeight by mutableStateOf<Double?>(null)
    internal var estimatedHeight = 0.0
    /// `window.short`: shorter than the compact breakpoint, as a phone on
    /// its side.
    val short: Boolean get() = (measuredHeight ?: estimatedHeight) < BernHost.layouts.compact
    /// The size of the window's width by the breakpoints of `[layouts]`:
    /// `compact`, `medium`, `expanded` or `large`. A television is `large`
    /// however it counts its pixels: most report 960 dp for a 1080p screen.
    val size: String
        get() = if (BernHost.television) "large" else BernHost.layouts.sizeOf(measuredWidth ?: estimatedWidth)
    val compact: Boolean get() = size == "compact"
    val medium: Boolean get() = size == "medium"
    val expanded: Boolean get() = size == "expanded"
    val large: Boolean get() = size == "large"
    /// `window.tv`: the window is on a television.
    val tv: Boolean get() = BernHost.television
    /// `window.watch`: the window is on a watch.
    val watch: Boolean get() = BernHost.watch
    /// Direct touchscreen hardware, independent of window size or a touchpad.
    val touch: Boolean get() = BernHost.touch
    /// `window.rail`: a split view shows its sidebar as a navigation rail.
    val rail: Boolean get() = !tv && !compact
    /// `window.unfolded`, `window.tabletop` and `window.fold`: a foldable's
    /// postures, which this target does not read yet.
    val unfolded: Boolean get() = false
    val tabletop: Boolean get() = false
    val fold: Double get() = 0.0
    /// A hardware keyboard is attached: `window.keyboard`.
    val keyboard: Boolean get() = BernHost.keyboard
    val chrome = mutableStateListOf<BernChromeEntry>()

    /// The window's title in the interface language.
    val title: String
        get() = BernHost.kinds.firstOrNull { it.id == kindID }?.let { BernText.string(it.title) } ?: ""

    @Suppress("UNCHECKED_CAST")
    fun <S : BernSessionState> session(type: Class<S>): S = sessions.first { type.isInstance(it) } as S

    internal fun attached(generation: Long, id: String) {
        // A new instance starts without a pending close.
        if (generation != this.generation) pendingClose = null
        this.generation = generation
        this.id = id
    }

    /// Write one session field through Rust, which validates it.
    fun set(session: Int, field: Int, encode: (BernWriter) -> Unit): Boolean =
        BernHost.sessionSet(this, session, field, encode)
}

val LocalBernWindow = staticCompositionLocalOf<BernWindowContext> { error("no Bern window") }

/// Where the pointer is over a window, in window coordinates: a mouse while
/// it is over the window, a finger or pen while it presses. Elements that
/// glow compare it with where they are, so what lies on top of them does
/// not keep them dark.
class BernPointer {
    var at by mutableStateOf<Offset?>(null)
    /// The window content the positions are first taken in.
    internal var root: androidx.compose.ui.layout.LayoutCoordinates? = null
}

val LocalBernPointer = staticCompositionLocalOf<BernPointer?> { null }

/// Follows the pointer over the window's content for `pointer`. It only
/// looks, before the content does, and consumes nothing.
fun Modifier.bernPointerWatch(pointer: BernPointer): Modifier =
    onGloballyPositioned { pointer.root = it }.pointerInput(pointer) {
        awaitPointerEventScope {
            while (true) {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                val change = event.changes.firstOrNull() ?: continue
                val over = event.type != PointerEventType.Exit &&
                    (change.type == PointerType.Mouse || change.pressed)
                pointer.at = if (over) pointer.root?.takeIf { it.isAttached }?.localToWindow(change.position) else null
            }
        }
    }

/// The room the app bar and the system bars take around a window's content.
val LocalBernBarsPadding = staticCompositionLocalOf { androidx.compose.foundation.layout.PaddingValues() }

/// `under_bars`: the layer fills the window to its edges while it is shown,
/// reaching out over the room the app bar and the system bars take; the
/// app bar lets it show through. A touch stops at the first layer that reads
/// pointers, and pictures and fills do not, so it reads them for nothing:
/// the pages under it take no touch through it.
@Composable
fun Modifier.bernUnderBars(): Modifier {
    val window = LocalBernWindow.current
    val padding = LocalBernBarsPadding.current
    DisposableEffect(window) {
        window.underBars += 1
        onDispose { window.underBars -= 1 }
    }
    val density = LocalDensity.current
    val direction = LocalLayoutDirection.current
    val top = with(density) { padding.calculateTopPadding().roundToPx() }
    val bottom = with(density) { padding.calculateBottomPadding().roundToPx() }
    // A phone on its side keeps its content off the camera at one end.
    val left = with(density) { padding.calculateLeftPadding(direction).roundToPx() }
    val right = with(density) { padding.calculateRightPadding(direction).roundToPx() }
    return this.fillMaxSize().layout { measurable, constraints ->
        val height = if (constraints.hasBoundedHeight) constraints.maxHeight + top + bottom else constraints.maxHeight
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth + left + right else constraints.maxWidth
        val placeable = measurable.measure(constraints.copy(minWidth = width, maxWidth = width, minHeight = height, maxHeight = height))
        layout(constraints.maxWidth, constraints.maxHeight) { placeable.place(-left, -top) }
    }.pointerInput(Unit) {}
}

/// How far a drawing reaches past its own bounds at each edge, in pixels.
data class BernReach(val left: Float, val top: Float, val right: Float, val bottom: Float) {
    val none: Boolean get() = left <= 0f && top <= 0f && right <= 0f && bottom <= 0f

    companion object {
        val None = BernReach(0f, 0f, 0f, 0f)
    }
}

/// A window's content to its edges and the safe area inside the room the
/// app bar and the system bars take, for what reaches past it.
class BernSafeArea {
    internal var window: LayoutCoordinates? = null
    internal var safe: LayoutCoordinates? = null

    /// How far `coordinates` would reach to the window's edges past each
    /// edge of the safe area it reaches; nothing at the others. A layout
    /// within `slack` of an edge reaches it.
    fun reach(coordinates: LayoutCoordinates, slack: Float): BernReach {
        val window = window?.takeIf { it.isAttached } ?: return BernReach.None
        val safe = safe?.takeIf { it.isAttached } ?: return BernReach.None
        if (!window.bernSharesLayout(safe) || !window.bernSharesLayout(coordinates)) return BernReach.None
        val area = window.localBoundingBoxOf(safe, clipBounds = false)
        val bounds = window.localBoundingBoxOf(coordinates, clipBounds = false)
        return BernReach(
            left = if (bounds.left <= area.left + slack) bounds.left.coerceAtLeast(0f) else 0f,
            top = if (bounds.top <= area.top + slack) bounds.top.coerceAtLeast(0f) else 0f,
            right = if (bounds.right >= area.right - slack) (window.size.width - bounds.right).coerceAtLeast(0f) else 0f,
            bottom = if (bounds.bottom >= area.bottom - slack) (window.size.height - bounds.bottom).coerceAtLeast(0f) else 0f,
        )
    }
}

/// The safe area of the window around; none in a scene, whose layers
/// bleed by their own rule.
val LocalBernSafeArea = staticCompositionLocalOf<BernSafeArea?> { null }

/// Coordinates may be compared only while attached to the same layout root.
/// Composition locals cross Dialog/Popup boundaries; layout coordinates do not.
internal fun LayoutCoordinates.bernSharesLayout(other: LayoutCoordinates): Boolean =
    isAttached && other.isAttached && findRootCoordinates() === other.findRootCoordinates()

/// Content in a separate popup window owns its layout scope. Its safe area is
/// measured inside that surface, never against the activity behind it. A card
/// has no bar insets; an edge-to-edge dialog supplies its own system bar room.
@OptIn(androidx.compose.animation.ExperimentalSharedTransitionApi::class)
@Composable
fun BernPopupContent(padding: PaddingValues = PaddingValues(), content: @Composable () -> Unit) {
    val safeArea = remember { BernSafeArea() }
    val pointer = remember { BernPointer() }
    val editing = remember { BernEditingRegions() }
    CompositionLocalProvider(
        LocalBernSafeArea provides safeArea,
        LocalBernBarsPadding provides padding,
        LocalBernScrollRoom provides null,
        LocalBernViewport provides null,
        LocalBernSceneRoom provides null,
        LocalBernShared provides null,
        LocalBernVisibility provides null,
        LocalBernFlex provides null,
        LocalBernPointer provides pointer,
        LocalBernEditingRegions provides editing,
        LocalBernEditingAncestors provides emptyList(),
    ) {
        Box(
            Modifier.onPlaced { safeArea.window = it }.padding(padding)
                .onPlaced { safeArea.safe = it }.consumeWindowInsets(padding)
                .bernPointerWatch(pointer).bernEditingPresses(editing),
            contentAlignment = Alignment.TopCenter,
            propagateMinConstraints = true,
        ) { content() }
    }
}

/// `Canvas(bleed: true)`: the canvas keeps its place in the layout and its
/// drawing reaches past each edge of the safe area it reaches, to the
/// window's edge, under the system bars and the app bar. In a vertical
/// scroll view only its sides do: at the very top of what the view scrolls
/// it also fills the room the view keeps above that, under the status bar,
/// and scrolls away with it; lower down it keeps its place.
class BernCanvasBleed {
    internal var sides by mutableStateOf(BernReach.None)
    internal var room: BernScrollRoom? = null
    /// The canvas starts where what its scroll view scrolls starts.
    internal var opens by mutableStateOf(false)

    /// `drawing` over the canvas and the room it reaches.
    fun draw(scope: DrawScope, drawing: DrawScope.() -> Unit) {
        val room = room
        val reach = if (room == null) sides else BernReach(sides.left, if (opens) room.top else 0f, sides.right, 0f)
        if (reach.none) {
            scope.drawing()
        } else {
            scope.inset(-reach.left, -reach.top, -reach.right, -reach.bottom) { drawing() }
        }
    }
}

/// Follows where a canvas that bleeds is (`BernCanvasBleed`); in a vertical
/// scroll view it has the view keep its room.
@Composable
fun Modifier.bernBleed(bleed: BernCanvasBleed): Modifier {
    val safe = LocalBernSafeArea.current ?: return this
    val room = LocalBernScrollRoom.current
    bleed.room = room
    if (room != null) {
        DisposableEffect(room) {
            room.bleeding += 1
            onDispose { room.bleeding -= 1 }
        }
    }
    // Half a dp of slack: what is laid out to an edge reaches it.
    val slack = with(LocalDensity.current) { 0.5.dp.toPx() }
    return onGloballyPositioned { coordinates ->
        bleed.sides = safe.reach(coordinates, slack)
        val content = room?.content?.takeIf { it.bernSharesLayout(coordinates) }
        bleed.opens = content != null && content.localPositionOf(coordinates, Offset.Zero).y <= slack
    }
}

// MARK: - Controls

/// What an external entry may do with a mounted control. The closures are
/// the same ones the control runs for user input.
class BernControlActions(
    val activate: (() -> Unit)? = null,
    val edit: ((String) -> Unit)? = null,
    val submit: (() -> Unit)? = null,
    val setBool: ((Boolean) -> Unit)? = null,
    val setNumber: ((Double) -> Unit)? = null,
    val select: ((String) -> Boolean)? = null,
    val back: (() -> Boolean)? = null,
    val value: (() -> String?)? = null,
    val focus: (() -> Unit)? = null,
    /// Takes a file as if the user chose or dropped it.
    val setFile: ((String) -> Unit)? = null,
    /// The kinds of file `setFile` takes, as `files:` names them.
    val fileKinds: String = "any",
)

class BernControlEntry(
    val target: String,
    val window: String,
    val role: String,
    val label: String,
    val enabled: Boolean,
    val actions: BernControlActions,
)

class BernCommandFailure(val code: String, override val message: String) : Exception(message)

class BernControls {
    private val entries = LinkedHashMap<Any, BernControlEntry>()

    fun register(token: Any, entry: BernControlEntry) {
        entries[token] = entry
    }

    fun remove(token: Any) {
        entries.remove(token)
    }

    val all: List<BernControlEntry>
        get() = entries.values.sortedWith(compareBy({ it.window }, { it.target }))

    fun find(target: String): BernControlEntry {
        val matches = entries.values.filter { it.target == target }
        if (matches.isEmpty()) throw BernCommandFailure("target_not_found", "no mounted control `$target`; see ui.inspect")
        if (matches.size > 1) throw BernCommandFailure("target_ambiguous", "`$target` is mounted in several places")
        val entry = matches[0]
        if (BernHost.isBlocked(entry.window)) {
            throw BernCommandFailure(
                "target_blocked",
                "`$target` is blocked by the close confirmation of `${entry.window}`; decide it with window.confirm_close",
            )
        }
        if (!entry.enabled) throw BernCommandFailure("target_unavailable", "`$target` is disabled")
        return entry
    }
}

/// Keys of the rows around a view, outermost first, as `[a][b]`: controls
/// inside a row, also in components it places, add them to their targets.
val LocalBernItemKeys = androidx.compose.runtime.compositionLocalOf { "" }

/// Registers a mounted control for external entries while it is composed.
@Composable
fun BernControl(
    target: String,
    role: String,
    label: String,
    enabled: Boolean = true,
    actions: BernControlActions,
) {
    val window = LocalBernWindow.current
    val token = remember { Any() }
    val entry = BernControlEntry(target + LocalBernItemKeys.current, window.id, role, label, enabled, actions)
    SideEffect { BernHost.controls.register(token, entry) }
    DisposableEffect(token) { onDispose { BernHost.controls.remove(token) } }
}

/// Declares the window's top bar title, a way back or actions while it is
/// composed.
@Composable
fun BernChrome(
    title: String? = null,
    back: (() -> Unit)? = null,
    leading: (@Composable RowScope.() -> Unit)? = null,
    actions: (@Composable RowScope.() -> Unit)? = null,
) {
    val window = LocalBernWindow.current
    val entry = remember { BernChromeEntry() }
    SideEffect {
        entry.title = title
        entry.back = back
        entry.leading = leading
        entry.actions = actions
    }
    DisposableEffect(entry) {
        window.chrome.add(entry)
        onDispose { window.chrome.remove(entry) }
    }
}

// MARK: - Host

/// What to do when the reply of a command arrives.
class BernPendingReply(
    val window: String?,
    val success: ((BernReader) -> Unit)? = null,
    val completion: ((BernCommandFailure?) -> Unit)? = null,
)

class BernFailure(val window: String?, val code: String, val message: String)

/// Window kinds of the application, generated from the window catalog.
/// The breakpoints of `[layouts]`, in dp: below `compact` a width is
/// compact, below `medium` (when given) medium, below `expanded` (when
/// given) expanded, and large from there.
class BernLayouts(val compact: Double, val medium: Double?, val expanded: Double?) {
    fun sizeOf(width: Double): String = when {
        width < compact -> "compact"
        medium != null && width < medium -> "medium"
        expanded == null || width < expanded -> "expanded"
        else -> "large"
    }
}

class BernWindowKind(
    val id: String,
    val title: String,
    val isSettings: Boolean,
    /// Shown over the primary window at its declared size.
    val isDialog: Boolean = false,
    val width: Double? = null,
    val height: Double? = null,
)

/// The functions of the Rust runtime, in `libbern_app.so`.
object BernNative {
    init {
        System.loadLibrary("bern_app")
    }

    external fun start(fingerprint: Long, resources: ByteArray, bridge: BernBridge): ByteArray
    external fun nowPlayingCommand(name: ByteArray)
    external fun overlayPressed(overlay: ByteArray, button: ByteArray)
    external fun allFilesAnswered()
    external fun filesOpened(url: ByteArray)
    external fun filesRead(url: ByteArray): ByteArray?
    external fun stop(app: Long)
    external fun snapshot(app: Long): ByteArray
    external fun engineId(app: Long): ByteArray
    external fun channel(app: Long): ByteArray
    external fun dispatch(app: Long, command: Int, params: ByteArray, token: Long)
    external fun windowOpened(app: Long, kind: Int): ByteArray
    external fun windowClosed(app: Long, kind: Int, generation: Long)
    external fun windowFacts(
        app: Long, kind: Int, generation: Long, source: Int, visibility: Int, size: Int,
        x: Double, y: Double, width: Double, height: Double, focused: Int, actions: Int,
    ): Long
    external fun windowRequestClose(app: Long, kind: Int, generation: Long, needsConfirmation: Int, canSave: Int): ByteArray
    external fun windowConfirmClose(app: Long, kind: Int, generation: Long, bytes: ByteArray): ByteArray
    external fun sessionSet(app: Long, kind: Int, generation: Long, session: Int, field: Int, value: ByteArray): ByteArray
    external fun sessionComplete(app: Long, request: Long, bytes: ByteArray)
    external fun queryOpen(app: Long, kind: Int, generation: Long, index: Int): ByteArray
    external fun queryRequest(app: Long, subscription: Long, params: ByteArray): ByteArray
    external fun queryClose(app: Long, subscription: Long)
    external fun mediaKernelAttach(app: Long, bytes: ByteArray): ByteArray
    external fun surfaceWindow(surface: android.view.Surface): Long
    external fun surfaceWindowRelease(window: Long)
    external fun mediaSurfaceAttach(app: Long, bytes: ByteArray)
    external fun mediaSurfaceResize(app: Long, bytes: ByteArray)
    external fun mediaSurfaceDetach(app: Long, bytes: ByteArray)
    external fun mediaSurfaceFrame(app: Long, view: ByteArray, now: Double, target: Double)
    external fun mediaExecutorAttached(app: Long, bytes: ByteArray): ByteArray
    external fun mediaSnapshot(app: Long): ByteArray
    external fun mediaDispatch(app: Long, command: Int, params: ByteArray, token: Long)
    external fun mediaCreate(app: Long, bytes: ByteArray): ByteArray
    external fun mediaBind(app: Long, bytes: ByteArray): ByteArray
    external fun mediaRelease(app: Long, bytes: ByteArray)
    external fun mediaView(app: Long, bytes: ByteArray)
    external fun mediaComplete(app: Long, bytes: ByteArray)
    external fun mediaEvent(app: Long, bytes: ByteArray)
    external fun svgCached(key: ByteArray): Int
    external fun svgOpen(key: ByteArray, data: ByteArray): Long
    external fun svgFree(player: Long)
    external fun svgConfigure(
        player: Long, playing: Int, speed: Double, restart: Double,
        fit: Int, interactive: Int, dark: Int, reducedMotion: Int, textRgba: Int,
    ): Int
    external fun svgPointer(player: Long, kind: Int, x: Double, y: Double): Int
    external fun svgRender(player: Long, nowSecs: Double, widthPx: Int, heightPx: Int, directBuf: java.nio.ByteBuffer): Int
}

/// Callbacks of the Rust runtime. They arrive on engine and channel
/// threads; the work moves to the main thread, except the file questions,
/// which the calling thread waits for.
@Keep
class BernBridge(context: Context) {
    private val main = Handler(Looper.getMainLooper())
    private val application = context.applicationContext

    /// The application, for engine libraries that reach Android through
    /// `ndk-context`, such as audio output.
    fun applicationContext(): Context = application
    private val documents = context.contentResolver

    @Keep
    fun onState(bytes: ByteArray) {
        main.post { BernHost.receiveState(bytes) }
    }

    @Keep
    fun onReply(token: Long, bytes: ByteArray) {
        main.post { BernHost.receiveReply(token, bytes) }
    }

    @Keep
    fun onSessionRequest(request: Long, bytes: ByteArray) {
        main.post { BernHost.receiveSessionRequest(request, bytes) }
    }

    @Keep
    fun onMediaRequest(bytes: ByteArray) {
        main.post { BernMedia.backend.perform(bytes) }
    }

    @Keep
    fun onMediaState(bytes: ByteArray) {
        main.post { BernMedia.receive(bytes) }
    }

    @Keep
    fun onQuery(subscription: Long, bytes: ByteArray) {
        main.post { BernHost.receiveQuery(subscription, bytes) }
    }

    /// The name the system shows for a document the app was given, such
    /// as a `content:` file or folder from the picker; null when it has none.
    @Keep
    fun fileName(uri: String): String? = try {
        documents.query(documentOf(Uri.parse(uri)), arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
    } catch (error: Exception) {
        null
    }

    /// What a folder from the picker, or a folder inside it, holds: for
    /// each child its document URI, name, `d` or `f`, size and last change
    /// in milliseconds (nothing when unknown), each followed by a NUL,
    /// after `ok:`; `error:` and the reason when it cannot be listed,
    /// `error:not_a_folder` for a file.
    @Keep
    fun listFolder(uri: String): String = try {
        val parsed = Uri.parse(uri)
        val folder = if (parsed.scheme == "content" && android.provider.DocumentsContract.isTreeUri(parsed)) {
            describe(documentOf(parsed))
        } else null
        if (folder == null || !folder.directory) "error:not_a_folder" else {
            val text = StringBuilder("ok:")
            for (child in children(folder.uri)) {
                for (field in listOf(child.uri.toString(), child.name, if (child.directory) "d" else "f", child.size.toString(), child.modified?.toString() ?: "")) {
                    text.append(field).append('\u0000')
                }
            }
            text.toString()
        }
    } catch (error: Exception) {
        "error:${error.message ?: error.javaClass.simpleName}"
    }

    /// A folder from the picker (a tree) stands for its top document.
    private fun documentOf(uri: Uri): Uri =
        if (android.provider.DocumentsContract.isTreeUri(uri) && !android.provider.DocumentsContract.isDocumentUri(application, uri)) {
            android.provider.DocumentsContract.buildDocumentUriUsingTree(uri, android.provider.DocumentsContract.getTreeDocumentId(uri))
        } else uri

    /// A descriptor the engine reads a document through, which it closes;
    /// -1 when the document cannot be opened.
    @Keep
    fun openFile(uri: String): Int = try {
        documents.openFileDescriptor(Uri.parse(uri), "r")?.detachFd() ?: -1
    } catch (error: Exception) {
        -1
    }

    /// Host services called from an engine worker. Only clipboard and
    /// activity launch move to the main thread; document copies stay here.
    @Keep
    fun platformRequest(operation: String, first: String, second: String): String = try {
        val value = when (operation) {
            "secret_read", "secret_write", "secret_delete" -> credential(operation, first, second)
            "copy_text" -> onMain {
                val clipboard = application.getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                clipboard.setPrimaryClip(android.content.ClipData.newPlainText("", first))
                ""
            }
            "open_path" -> onMain { openPath(first); "" }
            "save_image" -> { saveImage(File(first)); "" }
            "now_playing" -> onMain { BernNowPlaying.update(application, first); "" }
            "overlay_allowed" -> if (BernOverlay.allowed(application)) "yes" else "no"
            "overlay_ask" -> onMain { BernOverlay.ask(application); "" }
            "overlay_show" -> onMain { BernOverlay.show(application, first, second); "" }
            "overlay_lit" -> onMain { BernOverlay.lit(first, second); "" }
            "overlay_hide" -> onMain { BernOverlay.hide(application, first); "" }
            "all_files_allowed" -> if (BernFiles.allFilesAllowed(application)) "yes" else "no"
            "all_files_ask" -> onMain { BernFiles.askAllFiles(application); "" }
            "import_folder" -> importFolder(Uri.parse(first), File(second)).absolutePath
            "export_path" -> exportPath(File(first), Uri.parse(second)).toString()
            else -> error("Unsupported platform operation: $operation")
        }
        "ok:$value"
    } catch (error: Exception) {
        "error:${error.cause?.message ?: error.message ?: error.javaClass.simpleName}"
    }

    // Credentials are excluded from backup and encrypted by a non-exportable
    // Android Keystore key. No plaintext token is written to preferences.
    @Synchronized
    private fun credential(operation: String, name: String, value: String): String {
        require(name.isNotEmpty() && name.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it in "._-" })
        val directory = File(application.noBackupFilesDir, "bern-credentials")
        check(directory.isDirectory || directory.mkdirs())
        val file = android.util.AtomicFile(File(directory, name))
        if (operation == "secret_delete") {
            file.delete()
            check(!file.baseFile.exists())
            return ""
        }
        if (operation == "secret_read" && !file.baseFile.exists()) return "missing"
        val alias = "bern.credentials.v1"
        val store = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val key = (store.getKey(alias, null) as? javax.crypto.SecretKey) ?: run {
            check(operation == "secret_write") { "Credential key unavailable" }
            val generator = javax.crypto.KeyGenerator.getInstance("AES", "AndroidKeyStore")
            generator.init(android.security.keystore.KeyGenParameterSpec.Builder(alias,
                android.security.keystore.KeyProperties.PURPOSE_ENCRYPT or android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build())
            generator.generateKey()
        }
        val cipher = javax.crypto.Cipher.getInstance("AES/GCM/NoPadding")
        val aad = "${application.packageName}:$name".toByteArray(Charsets.UTF_8)
        if (operation == "secret_read") {
            val bytes = file.readFully()
            require(bytes.size >= 29 && bytes[0] == 1.toByte()) { "Invalid credential format" }
            cipher.init(javax.crypto.Cipher.DECRYPT_MODE, key, javax.crypto.spec.GCMParameterSpec(128, bytes.copyOfRange(1, 13)))
            cipher.updateAAD(aad)
            return "value:" + String(cipher.doFinal(bytes.copyOfRange(13, bytes.size)), Charsets.UTF_8)
        }
        cipher.init(javax.crypto.Cipher.ENCRYPT_MODE, key)
        cipher.updateAAD(aad)
        check(cipher.iv.size == 12)
        val encrypted = byteArrayOf(1) + cipher.iv + cipher.doFinal(value.toByteArray(Charsets.UTF_8))
        val output = file.startWrite()
        try {
            output.write(encrypted)
            file.finishWrite(output)
        } catch (error: Exception) {
            file.failWrite(output)
            throw error
        }
        return ""
    }

    private fun <T> onMain(action: () -> T): T {
        if (Looper.myLooper() == Looper.getMainLooper()) return action()
        val task = java.util.concurrent.FutureTask<T> { action() }
        check(main.post(task)) { "The Android UI is unavailable" }
        return task.get()
    }

    /// The engine's `save_image`: a picture into the Pictures collection, in
    /// a folder named after the app, where the gallery and other apps'
    /// pickers find it. Android 10 and later let an app add its own
    /// pictures there without asking; earlier versions need a permission
    /// the app does not hold.
    private fun saveImage(file: File) {
        require(file.isFile) { "The picture no longer exists" }
        check(android.os.Build.VERSION.SDK_INT >= 29) { "Saving pictures needs Android 10 or later" }
        val media = android.provider.MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        val type = if (file.extension.lowercase() in setOf("jpg", "jpeg")) "image/jpeg" else "image/png"
        val folder = application.applicationInfo.loadLabel(application.packageManager).toString()
            .filter { it.isLetterOrDigit() || it in " ._-" }.ifBlank { "Pictures" }
        val values = android.content.ContentValues().apply {
            put(android.provider.MediaStore.Images.Media.DISPLAY_NAME, file.name)
            put(android.provider.MediaStore.Images.Media.MIME_TYPE, type)
            put(android.provider.MediaStore.Images.Media.RELATIVE_PATH, "${android.os.Environment.DIRECTORY_PICTURES}/$folder")
            put(android.provider.MediaStore.Images.Media.IS_PENDING, 1)
        }
        val uri = checkNotNull(documents.insert(media, values)) { "The gallery did not take the picture" }
        try {
            checkNotNull(documents.openOutputStream(uri)) { "The gallery did not take the picture" }.use { output ->
                file.inputStream().use { it.copyTo(output) }
            }
            documents.update(uri, android.content.ContentValues().apply {
                put(android.provider.MediaStore.Images.Media.IS_PENDING, 0)
            }, null, null)
        } catch (error: Exception) {
            documents.delete(uri, null, null)
            throw error
        }
    }

    private fun openPath(value: String) {
        val parsed = Uri.parse(value)
        val uri = if (parsed.scheme == "content") parsed else {
            val file = if (parsed.scheme == "file") File(requireNotNull(parsed.path)) else File(value)
            require(file.exists()) { "The file no longer exists" }
            require(file.isFile) { "Export this folder to a location selected in Files before opening it" }
            androidx.core.content.FileProvider.getUriForFile(application, "${application.packageName}.bern.files", file)
        }
        val type = documents.getType(uri) ?: "application/octet-stream"
        val intent = android.content.Intent(android.content.Intent.ACTION_VIEW).apply {
            setDataAndType(uri, type)
            // Grant only this selected document, never a URI prefix.
            clipData = android.content.ClipData.newRawUri("", uri)
            addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        application.startActivity(intent)
    }

    private data class Document(val uri: Uri, val name: String, val directory: Boolean, val size: Long = 0, val modified: Long? = null)

    /// The document at `uri`, a document of a tree.
    private fun describe(uri: Uri): Document {
        val columns = arrayOf(android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME, android.provider.DocumentsContract.Document.COLUMN_MIME_TYPE)
        return documents.query(uri, columns, null, null, null)?.use { cursor ->
            check(cursor.moveToFirst()) { "The selected folder is unavailable" }
            Document(uri, cursor.getString(0) ?: "Folder", cursor.getString(1) == android.provider.DocumentsContract.Document.MIME_TYPE_DIR)
        } ?: error("The selected folder is unavailable")
    }

    private fun rootDocument(tree: Uri): Document {
        require(tree.scheme == "content" && android.provider.DocumentsContract.isTreeUri(tree)) { "Select a folder in the system Files picker" }
        val document = describe(android.provider.DocumentsContract.buildDocumentUriUsingTree(tree, android.provider.DocumentsContract.getTreeDocumentId(tree)))
        require(document.directory) { "The selected document is not a folder" }
        return document
    }

    private fun children(parent: Uri, limit: Int = Int.MAX_VALUE): List<Document> {
        val uri = android.provider.DocumentsContract.buildChildDocumentsUriUsingTree(parent, android.provider.DocumentsContract.getDocumentId(parent))
        val columns = arrayOf(
            android.provider.DocumentsContract.Document.COLUMN_DOCUMENT_ID, android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            android.provider.DocumentsContract.Document.COLUMN_MIME_TYPE, android.provider.DocumentsContract.Document.COLUMN_SIZE,
            android.provider.DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )
        return documents.query(uri, columns, null, null, null)?.use { cursor ->
            buildList {
                while (cursor.moveToNext()) {
                    check(size < limit) { "The folder contains too many items" }
                    add(Document(
                        android.provider.DocumentsContract.buildDocumentUriUsingTree(parent, cursor.getString(0)),
                        cursor.getString(1) ?: "File",
                        cursor.getString(2) == android.provider.DocumentsContract.Document.MIME_TYPE_DIR,
                        if (cursor.isNull(3)) 0 else cursor.getLong(3),
                        if (cursor.isNull(4)) null else cursor.getLong(4),
                    ))
                }
            }
        } ?: error("The selected folder cannot be read")
    }

    private fun safeName(name: String): String = name.map { if (it == '/' || it == '\\' || it.code < 32) '_' else it }
        .joinToString("").trim().takeUnless { it.isEmpty() || it == "." || it == ".." } ?: "File"

    private fun numbered(name: String, number: Int, directory: Boolean): String {
        if (number == 0) return name
        val dot = if (directory) -1 else name.lastIndexOf('.').takeIf { it > 0 } ?: -1
        return if (dot < 0) "$name ($number)" else "${name.substring(0, dot)} ($number)${name.substring(dot)}"
    }

    private fun newLocal(parent: File, name: String, directory: Boolean): File {
        val safe = safeName(name)
        for (number in 0..100_000) {
            val file = File(parent, numbered(safe, number, directory))
            if (if (directory) file.mkdir() else file.createNewFile()) return file
            check(file.exists()) { "Cannot create ${file.name}" }
        }
        error("Too many files have the same name")
    }

    private fun importFolder(tree: Uri, destination: File): File {
        val source = rootDocument(tree)
        check(destination.isDirectory || destination.mkdirs()) { "Cannot create the import directory" }
        val root = newLocal(destination, source.name, true)
        try {
            var fileCount = 0
            var directoryCount = 0
            fun copy(document: Document, target: File, depth: Int) {
                check(depth < 32) { "The folder nesting exceeds 32 levels" }
                if (document.directory) {
                    check(++directoryCount <= 10_000) { "The folder contains more than 10,000 directories" }
                    for (child in children(document.uri, 20_000 - fileCount - directoryCount)) {
                        copy(child, newLocal(target, child.name, child.directory), depth + 1)
                    }
                } else {
                    check(++fileCount <= 10_000) { "The folder contains more than 10,000 files" }
                    val input = documents.openInputStream(document.uri) ?: error("Cannot read ${document.name}")
                    input.use { read -> target.outputStream().use { write -> read.copyTo(write, 128 * 1024) } }
                }
            }
            copy(source, root, 0)
            return root
        } catch (error: Exception) {
            root.deleteRecursively()
            throw error
        }
    }

    private fun createDocument(parent: Uri, name: String, directory: Boolean): Uri {
        val existing = children(parent).map { it.name }.toHashSet()
        val safe = safeName(name)
        val chosen = (0..100_000).asSequence().map { numbered(safe, it, directory) }.firstOrNull { it !in existing }
            ?: error("Too many documents have the same name")
        val type = if (directory) android.provider.DocumentsContract.Document.MIME_TYPE_DIR else {
            android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(chosen.substringAfterLast('.', "").lowercase(Locale.ROOT))
                ?: "application/octet-stream"
        }
        return android.provider.DocumentsContract.createDocument(documents, parent, type, chosen)
            ?: error("Cannot create $chosen in the selected folder")
    }

    private fun exportPath(source: File, tree: Uri): Uri {
        require(!java.nio.file.Files.isSymbolicLink(source.toPath())) { "Symbolic links cannot be exported" }
        require(source.isFile || source.isDirectory) { "The file to export no longer exists" }
        val parent = rootDocument(tree)
        val root = createDocument(parent.uri, source.name, source.isDirectory)
        try {
            fun copy(file: File, target: Uri, depth: Int) {
                require(!java.nio.file.Files.isSymbolicLink(file.toPath())) { "Symbolic links cannot be exported" }
                check(depth < 128) { "The folder nesting is too deep" }
                if (file.isDirectory) {
                    val entries = file.listFiles() ?: error("Cannot read ${file.name}")
                    for (child in entries) copy(child, createDocument(target, child.name, child.isDirectory), depth + 1)
                } else {
                    val output = documents.openOutputStream(target, "w") ?: error("Cannot write ${file.name}")
                    output.use { write -> file.inputStream().use { read -> read.copyTo(write, 128 * 1024) } }
                }
            }
            copy(source, root, 0)
            return root
        } catch (error: Exception) {
            runCatching { android.provider.DocumentsContract.deleteDocument(documents, root) }
            throw error
        }
    }
}

object BernHost {
    var handle = 0L
        private set
    private val replies = HashMap<Long, BernPendingReply>()
    private var nextToken = 1L
    private var revision = 0L
    private var apply: ((Int, BernReader) -> Unit)? = null
    var kinds: List<BernWindowKind> = emptyList()
        private set
    var primary = ""
        private set
    private val windows = HashMap<String, BernWindowContext>()
    private val pendingOpens = HashMap<String, MutableList<Long>>()
    private val queries = HashMap<Long, (ByteArray) -> Unit>()
    private var openedInitial = false
    val controls = BernControls()
    /// The application's breakpoints, set before the first window shows.
    var layouts = BernLayouts(0.0, null, null)
    /// Whether the app runs on a television (Android TV, Google TV), which
    /// shows an `Adaptive`'s `tv` layout whatever its width.
    var television = false
    /// Whether the app runs on a Wear OS watch.
    var watch = false
    var touch = false
        private set
    /// Whether a hardware keyboard is attached, from the configuration the
    /// activity has; it changes as one connects.
    var keyboard by mutableStateOf(true)
        private set

    fun readKeyboard(config: android.content.res.Configuration) {
        keyboard = config.keyboard == android.content.res.Configuration.KEYBOARD_QWERTY &&
            config.hardKeyboardHidden != android.content.res.Configuration.HARDKEYBOARDHIDDEN_YES
    }

    var failure by mutableStateOf<BernFailure?>(null)
    var startupError by mutableStateOf<String?>(null)
        private set
    var engineID = ""
        private set
    var channel = ""
        private set

    /// Whether the primary window fills the screen; closed, the screen
    /// offers to open it again.
    var primaryOpen by mutableStateOf(true)
        private set
    /// Window kinds shown over the primary window, bottom first.
    val sheets = mutableStateListOf<String>()

    /// Start the engine in this process and load the first state snapshot.
    /// The engine lives as long as the process, across activities.
    fun start(
        context: Context,
        fingerprint: Long,
        languages: List<String>,
        defaultLanguage: String,
        kinds: List<BernWindowKind>,
        primary: String,
        apply: (Int, BernReader) -> Unit,
    ) {
        BernMotionState.readSystem(context)
        if (handle != 0L) return
        touch = context.packageManager.hasSystemFeature(android.content.pm.PackageManager.FEATURE_TOUCHSCREEN)
        television = (context.resources.configuration.uiMode and Configuration.UI_MODE_TYPE_MASK) ==
            Configuration.UI_MODE_TYPE_TELEVISION ||
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
        BernText.load(context, languages)
        BernLanguage.setup(context, languages, defaultLanguage)
        this.kinds = kinds
        this.primary = primary
        this.apply = apply
        val resources = try {
            BernResources.extract(context).absolutePath.toByteArray(Charsets.UTF_8)
        } catch (error: Exception) {
            startupError = "cannot unpack the app's resources: ${error.message}"
            return
        }
        val reader = BernReader(BernNative.start(fingerprint, resources, BernBridge(context.applicationContext)))
        if (reader.u8() != 0) {
            startupError = reader.text()
            return
        }
        handle = reader.u64()
        engineID = String(BernNative.engineId(handle), Charsets.UTF_8)
        channel = String(BernNative.channel(handle), Charsets.UTF_8)
        receiveState(BernNative.snapshot(handle))
        BernMedia.attachExecutor(context)
    }

    // MARK: Engine state and commands

    fun receiveState(bytes: ByteArray) {
        val reader = BernReader(bytes)
        try {
            val next = reader.u64()
            if (next < revision) return
            revision = next
            val count = reader.u32()
            repeat(count) {
                val field = reader.u32()
                apply?.invoke(field, reader)
            }
        } catch (error: Exception) {
            failure = BernFailure(null, "invalid_state", error.toString())
        }
    }

    /// Dispatch an engine command; the window receives a failure alert.
    fun dispatch(
        command: Int,
        window: String?,
        encode: (BernWriter) -> Unit,
        success: ((BernReader) -> Unit)? = null,
        completion: ((BernCommandFailure?) -> Unit)? = null,
    ) {
        val reply = BernPendingReply(window, success, completion)
        if (handle == 0L) {
            completion?.invoke(BernCommandFailure("engine_stopped", "the engine is not running"))
            return
        }
        val writer = BernWriter()
        encode(writer)
        val token = nextToken++
        replies[token] = reply
        BernNative.dispatch(handle, command, writer.bytes(), token)
    }

    /// Dispatch a media command; the window receives a failure alert.
    fun dispatchMedia(command: Int, window: String?, encode: (BernWriter) -> Unit) {
        if (handle == 0L) return
        val writer = BernWriter()
        encode(writer)
        val token = nextToken++
        replies[token] = BernPendingReply(window)
        BernNative.mediaDispatch(handle, command, writer.bytes(), token)
    }

    /// Read a `0` or `1, code, message` answer.
    fun status(bytes: ByteArray): BernCommandFailure? {
        val reader = BernReader(bytes)
        return try {
            if (reader.u8() == 0) null else BernCommandFailure(reader.text(), reader.text())
        } catch (_: Exception) {
            BernCommandFailure("engine_stopped", "the engine is not running")
        }
    }

    fun receiveReply(token: Long, bytes: ByteArray) {
        val pending = replies.remove(token) ?: return
        val reader = BernReader(bytes)
        try {
            val status = reader.u8()
            reader.u64()
            if (status == 0) {
                pending.success?.invoke(reader)
                pending.completion?.invoke(null)
            } else {
                val code = reader.text()
                val message = reader.text()
                // A control of a module that was just switched off: its
                // state is on the way, and nothing went wrong for the user.
                if (code != "module_disabled") failure = BernFailure(pending.window, code, message)
                pending.completion?.invoke(BernCommandFailure(code, message))
            }
        } catch (error: Exception) {
            failure = BernFailure(pending.window, "invalid_reply", error.toString())
            pending.completion?.invoke(BernCommandFailure("invalid_reply", error.toString()))
        }
    }

    // MARK: Queries

    fun queryOpen(context: BernWindowContext, index: Int, receive: (ByteArray) -> Unit): Long {
        if (handle == 0L || context.generation == 0L) {
            throw BernCommandFailure("target_not_found", "the window is not attached")
        }
        val reader = BernReader(BernNative.queryOpen(handle, context.kind, context.generation, index))
        if (reader.u8() != 0) throw BernCommandFailure(reader.text(), reader.text())
        val subscription = reader.u64()
        queries[subscription] = receive
        return subscription
    }

    fun queryClose(subscription: Long) {
        queries.remove(subscription)
        if (handle != 0L) BernNative.queryClose(handle, subscription)
    }

    fun receiveQuery(subscription: Long, bytes: ByteArray) {
        queries[subscription]?.invoke(bytes)
    }

    // MARK: Windows

    fun attach(context: BernWindowContext) {
        if (handle == 0L) return
        val reader = BernReader(BernNative.windowOpened(handle, context.kind))
        try {
            if (reader.u8() != 0) return
            val generation = reader.u64()
            val id = reader.text()
            val count = reader.u32()
            repeat(count) {
                val index = reader.u32()
                val session = context.sessions.firstOrNull { it.index == index }
                    ?: throw BernDecodeError("unknown session $index")
                session.load(reader)
            }
            context.attached(generation, id)
            windows[context.kindID] = context
            reportFacts()
            for (request in pendingOpens.remove(context.kindID) ?: emptyList()) {
                complete(request, 8) { it.text(id) }
            }
        } catch (error: Exception) {
            failure = BernFailure(null, "invalid_window", error.toString())
        }
    }

    fun detach(context: BernWindowContext) {
        if (handle == 0L || windows[context.kindID] !== context) return
        windows.remove(context.kindID)
        BernMedia.windowClosed(context.id)
        BernNative.windowClosed(handle, context.kind, context.generation)
        reportFacts()
    }

    fun sessionSet(context: BernWindowContext, session: Int, field: Int, encode: (BernWriter) -> Unit): Boolean {
        if (handle == 0L || context.generation == 0L) return false
        val writer = BernWriter()
        encode(writer)
        val result = BernNative.sessionSet(handle, context.kind, context.generation, session, field, writer.bytes())
        return result.isNotEmpty() && result[0].toInt() == 0
    }

    /// Open a window kind: the primary fills the screen again, any other
    /// becomes the top sheet.
    fun open(kindID: String) {
        if (kinds.none { it.id == kindID }) return
        if (kindID == primary) {
            primaryOpen = true
        } else if (kindID !in sheets) {
            sheets.add(kindID)
        }
    }

    /// Open windows declared `initial: open` once per process.
    fun openInitial(kindIDs: List<String>) {
        if (openedInitial) return
        openedInitial = true
        kindIDs.forEach(::open)
    }

    /// Request closing a window kind's instance through the close protocol.
    fun close(kindID: String) {
        val context = windows[kindID] ?: return
        try {
            requestClose(context)
        } catch (problem: BernCommandFailure) {
            failure = BernFailure(context.id, problem.code, problem.message)
        }
    }

    /// Whether a sheet covers the window.
    private fun isCovered(context: BernWindowContext): Boolean {
        val index = sheets.indexOf(context.kindID)
        if (index >= 0) return index < sheets.size - 1
        return context.kindID == primary && sheets.isNotEmpty()
    }

    private fun covered(context: BernWindowContext) =
        BernCommandFailure("target_unavailable", "`${context.id}` is covered by another window; close the windows over it first")

    private fun checkClosable(context: BernWindowContext) {
        if (context.kindID != primary && isCovered(context)) throw covered(context)
    }

    private fun activate(context: BernWindowContext) {
        if (isCovered(context)) throw covered(context)
    }

    private fun finishClose(context: BernWindowContext) {
        if (context.kindID == primary) {
            primaryOpen = false
        } else {
            val index = sheets.indexOf(context.kindID)
            if (index >= 0) while (sheets.size > index) sheets.removeAt(sheets.size - 1)
        }
    }

    /// Report every window: visible and focused unless covered.
    fun reportFacts() {
        if (handle == 0L) return
        for (context in windows.values) {
            if (context.generation == 0L) continue
            val covered = isCovered(context)
            val closable = context.kindID == primary || !covered
            val actions = (if (covered) 0 else 1) or (if (closable) 2 else 0)
            BernNative.windowFacts(
                handle, context.kind, context.generation, 1, 0, 0,
                0.0, 0.0, 0.0, 0.0, if (covered) 0 else 1, actions,
            )
        }
    }

    // MARK: Close protocol

    fun unavailable(context: BernWindowContext, action: String) =
        BernCommandFailure("target_unavailable", "`${context.id}` cannot $action now")

    fun isBlocked(windowID: String): Boolean = windows.values.any { it.id == windowID && it.pendingClose != null }

    /// Run the window's declared close check and ask the session runtime
    /// whether to close now or to wait for a confirmation decision.
    fun requestClose(context: BernWindowContext): BernCloseResult {
        if (handle == 0L || context.generation == 0L) {
            throw BernCommandFailure("target_not_found", "the window is not attached")
        }
        checkClosable(context)
        val check = context.closeCheck
        val needsConfirmation = check?.needsConfirmation?.invoke() ?: false
        val canSave = check?.save != null
        val reader = BernReader(
            BernNative.windowRequestClose(
                handle, context.kind, context.generation, if (needsConfirmation) 1 else 0, if (canSave) 1 else 0,
            ),
        )
        if (reader.u8() != 0) throw BernCommandFailure(reader.text(), reader.text())
        if (reader.u8() == 0) {
            finishClose(context)
            return BernCloseResult(BernCloseResult.CLOSED, context.id)
        }
        val pending = BernPendingClose(reader.text(), check?.message?.invoke() ?: "", canSave, reader.u64())
        if (context.pendingClose != pending) context.pendingClose = pending
        return BernCloseResult(
            BernCloseResult.CONFIRMATION_REQUIRED, context.id, confirmation = pending.id,
            draftRevision = pending.draftRevision, decisions = pending.decisions, message = pending.message,
        )
    }

    /// Apply a decision from the dialog or from an external entry. A save
    /// that fails keeps the window open.
    fun confirmClose(
        context: BernWindowContext,
        confirmation: String,
        decision: Int,
        completion: (BernCloseResult?, BernCommandFailure?) -> Unit,
    ) {
        if (handle == 0L) {
            completion(null, BernCommandFailure("engine_stopped", "the engine is not running"))
            return
        }
        val writer = BernWriter()
        writer.text(confirmation)
        writer.u32(decision)
        val reader = BernReader(BernNative.windowConfirmClose(handle, context.kind, context.generation, writer.bytes()))
        val revision = context.pendingClose?.draftRevision ?: 0L
        try {
            if (reader.u8() != 0) {
                val problem = BernCommandFailure(reader.text(), reader.text())
                if (problem.code == "stale_confirmation" && context.pendingClose?.id == confirmation) {
                    context.pendingClose = null
                }
                completion(null, problem)
                return
            }
            val step = reader.u8()
            context.pendingClose = null
            when (step) {
                0 -> {
                    finishClose(context)
                    completion(BernCloseResult(BernCloseResult.CLOSED, context.id, draftRevision = revision), null)
                }
                1 -> {
                    val save = context.closeCheck?.save
                    if (save == null) {
                        completion(null, BernCommandFailure("invalid_params", "no save command"))
                        return
                    }
                    save { problem ->
                        if (problem != null) {
                            completion(
                                BernCloseResult(
                                    BernCloseResult.REJECTED, context.id, draftRevision = revision,
                                    reason = "save_failed: ${problem.code}: ${problem.message}",
                                ),
                                null,
                            )
                        } else {
                            finishClose(context)
                            completion(BernCloseResult(BernCloseResult.CLOSED, context.id, draftRevision = revision), null)
                        }
                    }
                }
                else -> completion(
                    BernCloseResult(BernCloseResult.REJECTED, context.id, draftRevision = revision, reason = "cancelled"),
                    null,
                )
            }
        } catch (error: Exception) {
            completion(null, BernCommandFailure("invalid_reply", error.toString()))
        }
    }

    // MARK: Session commands from external entries

    fun receiveSessionRequest(request: Long, bytes: ByteArray) {
        val reader = BernReader(bytes)
        try {
            run(reader.u32(), request, reader)
        } catch (failure: BernCommandFailure) {
            fail(request, failure)
        } catch (error: Exception) {
            fail(request, BernCommandFailure("invalid_request", error.toString()))
        }
    }

    private fun fail(request: Long, failure: BernCommandFailure) {
        val writer = BernWriter()
        writer.u8(1)
        writer.text(failure.code)
        writer.text(failure.message)
        send(request, writer.bytes())
    }

    private fun complete(request: Long, command: Int, result: (BernWriter) -> Unit) {
        val writer = BernWriter()
        writer.u8(0)
        writer.u32(command)
        result(writer)
        send(request, writer.bytes())
    }

    private fun send(request: Long, bytes: ByteArray) {
        if (handle != 0L) BernNative.sessionComplete(handle, request, bytes)
    }

    private fun window(id: String): BernWindowContext =
        windows.values.firstOrNull { it.id == id } ?: throw BernCommandFailure("target_not_found", "no open window `$id`")

    private fun unsupported(entry: BernControlEntry, action: String) =
        BernCommandFailure("target_unavailable", "`${entry.target}` (${entry.role}) does not support $action")

    /// Command indices follow the framework session catalog order.
    private fun run(command: Int, request: Long, reader: BernReader) {
        when (command) {
            0 -> { // ui.inspect
                val entries = controls.all
                complete(request, command) { writer ->
                    writer.u32(entries.size)
                    for (entry in entries) {
                        writer.text(entry.target)
                        writer.text(entry.window)
                        writer.text(entry.role)
                        writer.text(entry.label)
                        writer.bool(entry.enabled)
                        writer.optional(entry.actions.value?.invoke()) { writer.text(it) }
                        val reason = if (isBlocked(entry.window)) "blocked_by_modal" else if (entry.enabled) null else "disabled"
                        writer.optional(reason) { writer.text(it) }
                    }
                }
            }
            1 -> { // ui.activate
                val entry = controls.find(reader.text())
                val activate = entry.actions.activate ?: throw unsupported(entry, "activate")
                activate()
                complete(request, command) {}
            }
            2 -> { // ui.edit
                val entry = controls.find(reader.text())
                val value = reader.text()
                val edit = entry.actions.edit ?: throw unsupported(entry, "edit")
                edit(value)
                complete(request, command) {}
            }
            3 -> { // ui.submit
                val entry = controls.find(reader.text())
                val submit = entry.actions.submit ?: throw unsupported(entry, "submit")
                submit()
                complete(request, command) {}
            }
            4 -> { // ui.set_bool
                val entry = controls.find(reader.text())
                val value = reader.bool()
                val set = entry.actions.setBool ?: throw unsupported(entry, "set_bool")
                set(value)
                complete(request, command) {}
            }
            5 -> { // ui.set_number
                val entry = controls.find(reader.text())
                val value = reader.f64()
                val set = entry.actions.setNumber ?: throw unsupported(entry, "set_number")
                set(value)
                complete(request, command) {}
            }
            6 -> { // ui.select
                val entry = controls.find(reader.text())
                val option = reader.text()
                val select = entry.actions.select ?: throw unsupported(entry, "select")
                if (!select(option)) throw BernCommandFailure("invalid_params", "`${entry.target}` has no option `$option`")
                complete(request, command) {}
            }
            7 -> { // ui.back
                val entry = controls.find(reader.text())
                val back = entry.actions.back ?: throw unsupported(entry, "back")
                if (!back()) throw BernCommandFailure("target_unavailable", "nothing to go back to")
                complete(request, command) {}
            }
            8 -> { // window.open
                val kindID = reader.text()
                if (kinds.none { it.id == kindID }) throw BernCommandFailure("target_not_found", "no window kind `$kindID`")
                val existing = windows[kindID]
                if (existing != null) {
                    open(kindID)
                    complete(request, command) { it.text(existing.id) }
                } else {
                    pendingOpens.getOrPut(kindID) { mutableListOf() }.add(request)
                    open(kindID)
                }
            }
            9 -> { // window.close
                val result = requestClose(window(reader.text()))
                complete(request, command) { result.encode(it) }
            }
            10 -> { // window.activate
                activate(window(reader.text()))
                complete(request, command) {}
            }
            11 -> { // window.confirm_close
                val context = window(reader.text())
                val confirmation = reader.text()
                val decision = reader.u32()
                if (decision !in 0..2) throw BernCommandFailure("invalid_params", "unknown close decision")
                confirmClose(context, confirmation, decision) { result, problem ->
                    if (result != null) complete(request, command) { result.encode(it) } else fail(request, problem!!)
                }
            }
            12 -> throw BernCommandFailure("unknown_command", "window.list is answered by the runtime")
            13 -> throw unavailable(window(reader.text()), "minimize")
            14 -> throw unavailable(window(reader.text()), "restore")
            15 -> throw unavailable(window(reader.text()), "maximize")
            16 -> throw unavailable(window(reader.text()), "move")
            17 -> throw unavailable(window(reader.text()), "resize")
            18 -> complete(request, command) { it.text(BernLanguage.current) } // ui.language
            19 -> { // ui.set_language
                val language = reader.text()
                if (!BernLanguage.select(language)) {
                    throw BernCommandFailure(
                        "unsupported_language",
                        "`$language` is not supported; supported: ${BernLanguage.supported.joinToString(", ")}",
                    )
                }
                complete(request, command) {}
            }
            20 -> { // ui.focus
                val entry = controls.find(reader.text())
                val focus = entry.actions.focus ?: throw unsupported(entry, "focus")
                focus()
                complete(request, command) {}
            }
            21 -> { // ui.set_file
                val entry = controls.find(reader.text())
                val url = reader.text()
                val give = entry.actions.setFile ?: throw unsupported(entry, "set_file")
                val scheme = url.substringBefore(':', "").lowercase()
                if (scheme !in listOf("file", "content", "http", "https")) {
                    throw BernCommandFailure("invalid_url", "`$url` is not a file:// or http(s) URL")
                }
                if (!BernFiles.accepts(url, entry.actions.fileKinds)) {
                    throw BernCommandFailure(
                        "unsupported_file",
                        "`${entry.target}` takes ${entry.actions.fileKinds}, not `${url.substringAfterLast('/')}`",
                    )
                }
                give(url)
                complete(request, command) {}
            }
            else -> throw BernCommandFailure("unknown_command", "unknown session command $command")
        }
    }
}

// MARK: - Close protocol types

/// A window's declared close check, generated from its `Window` declaration.
class BernCloseCheck(
    val needsConfirmation: () -> Boolean,
    val message: () -> String,
    val save: ((done: (BernCommandFailure?) -> Unit) -> Unit)?,
)

/// A close confirmation waiting for a decision.
data class BernPendingClose(val id: String, val message: String, val canSave: Boolean, val draftRevision: Long) {
    /// Decisions in the order of the session catalog's `CloseDecision`:
    /// save, discard, cancel.
    val decisions: List<Int> get() = if (canSave) listOf(0, 1, 2) else listOf(1, 2)
}

/// The session catalog's `CloseResult`, encoded field by field.
class BernCloseResult(
    val outcome: Int,
    val window: String,
    val confirmation: String? = null,
    val draftRevision: Long = 0,
    val decisions: List<Int> = emptyList(),
    val message: String? = null,
    val reason: String? = null,
) {
    fun encode(writer: BernWriter) {
        writer.u32(outcome)
        writer.text(window)
        writer.optional(confirmation) { writer.text(it) }
        writer.i64(draftRevision)
        writer.list(decisions) { writer.u32(it) }
        writer.optional(message) { writer.text(it) }
        writer.optional(reason) { writer.text(it) }
    }

    companion object {
        const val CLOSED = 0
        const val REJECTED = 1
        const val CONFIRMATION_REQUIRED = 2
    }
}

// MARK: - Resources

/// The declared resources ship as assets under `bern/resources`; the engine
/// reads files, so they are unpacked once per installed version.
object BernResources {
    /// Where the unpacked resources are, once the host started.
    var root: File? = null
        private set

    fun extract(context: Context): File {
        val root = File(context.filesDir, "bern-resources")
        this.root = root
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        val stamp = File(context.filesDir, "bern-resources.stamp")
        val version = info.lastUpdateTime.toString()
        if (stamp.isFile && stamp.readText() == version) return root
        root.deleteRecursively()
        copy(context, "bern/resources", root)
        root.mkdirs()
        stamp.writeText(version)
        return root
    }

    private fun copy(context: Context, path: String, target: File) {
        val children = context.assets.list(path) ?: return
        if (children.isEmpty()) {
            // A file, or an empty directory the assets do not keep.
            try {
                context.assets.open(path).use { input ->
                    target.parentFile?.mkdirs()
                    target.outputStream().use { input.copyTo(it) }
                }
            } catch (_: java.io.IOException) {
            }
            return
        }
        for (child in children) copy(context, "$path/$child", File(target, child))
    }
}

// @bern:wear-skip-start
// MARK: - Theme and window roots

/// Material 3 colors that follow the system: dynamic colors from the
/// wallpaper where the system has them, the default scheme otherwise.
@Composable
fun BernTheme(content: @Composable () -> Unit) {
    val system = isSystemInDarkTheme()
    val dark = when (BernThemeState.appearance?.lowercase()) {
        "dark" -> true
        "light" -> false
        else -> system
    }
    // The status and navigation bar icons follow the chosen appearance.
    val view = LocalView.current
    val activity = LocalContext.current as? android.app.Activity
    SideEffect {
        activity?.window?.let { window ->
            val controller = WindowCompat.getInsetsController(window, view)
            controller.isAppearanceLightStatusBars = !dark
            controller.isAppearanceLightNavigationBars = !dark
        }
    }
    // Animations turned off in the system's settings while the app was
    // away apply when it comes back.
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle) {
        val observer = androidx.lifecycle.LifecycleEventObserver { _, event ->
            if (event == androidx.lifecycle.Lifecycle.Event.ON_RESUME) activity?.let(BernMotionState::readSystem)
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
    BernScheme(dark, content)
}

/// Shows the primary window and the windows open over it. Each window
/// fills the screen; the system back closes the top one.
@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun BernVirtualHost(window: @Composable (String) -> Unit) {
    Box(Modifier.fillMaxSize().semantics { testTagsAsResourceId = true }) {
        if (BernHost.primaryOpen) {
            window(BernHost.primary)
        } else {
            BernClosedPrimary()
        }
        for (kind in BernHost.sheets) {
            val dialog = BernHost.kinds.firstOrNull { it.id == kind && it.isDialog }
            if (dialog != null) {
                // A dialog window floats over the primary window at its
                // declared size, with its title bar.
                androidx.compose.ui.window.Dialog(
                    onDismissRequest = { BernHost.close(kind) },
                    properties = androidx.compose.ui.window.DialogProperties(usePlatformDefaultWidth = false),
                ) {
                    androidx.compose.material3.Surface(
                        shape = androidx.compose.foundation.shape.RoundedCornerShape(28.dp),
                        modifier = Modifier
                            .padding(24.dp)
                            .width((dialog.width ?: 360.0).dp)
                            .height(((dialog.height ?: 240.0) + 64.0).dp),
                    ) { window(kind) }
                }
            } else {
                Box(Modifier.fillMaxSize()) { window(kind) }
            }
        }
    }
    val top = BernHost.sheets.lastOrNull()
    BackHandler(enabled = top != null) { top?.let(BernHost::close) }
}

@Composable
private fun BernClosedPrimary() {
    val title = BernHost.kinds.firstOrNull { it.id == BernHost.primary }?.let { BernText.string(it.title) } ?: ""
    Box(Modifier.fillMaxSize().padding(24.dp)) {
        TextButton(onClick = { BernHost.open(BernHost.primary) }) {
            Text(BernText.format("bern.window.open", listOf(title)))
        }
    }
}

/// Root of every window: attaches the instance to the session runtime,
/// shows the top bar, close confirmations and command failures.
@OptIn(ExperimentalMaterial3Api::class)
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
    val top = BernHost.sheets.lastOrNull() ?: BernHost.primary
    val onTop = top == context.kindID
    // `window.idle`: no touch, pointer or key input for `idleAfter` seconds,
    // while `idleEnabled` holds; switched off, the window is not idle. With
    // `idle_area`s on screen a mouse counts only while it rests over one,
    // for that area's seconds: elsewhere, or outside the window, the window
    // stays awake. A change of the areas starts the count again.
    var input by remember { mutableLongStateOf(0L) }
    if (idleAfter != null) {
        LaunchedEffect(input, BernShortcuts.keys, idleEnabled) {
            context.idle = false
            if (!idleEnabled) return@LaunchedEffect
            snapshotFlow { context.idleWait(idleAfter) }.collectLatest { wait ->
                context.idle = false
                if (wait == null) return@collectLatest
                delay((wait * 1000).toLong())
                context.idle = true
            }
        }
    }
    // Immersive: the top bar and the system bars hide; a swipe from the
    // edge shows the system bars for a moment.
    val view = LocalView.current
    val activity = LocalContext.current as? android.app.Activity
    val hidden = immersive && onTop
    DisposableEffect(hidden, activity) {
        val window = activity?.window
        if (window != null) {
            val controller = WindowCompat.getInsetsController(window, view)
            if (hidden) {
                controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                controller.hide(WindowInsetsCompat.Type.systemBars())
            } else {
                controller.show(WindowInsetsCompat.Type.systemBars())
            }
        }
        onDispose {}
    }
    // `landscape`: the phone turns the window to landscape while it holds,
    // and leaves the orientation to the system again after.
    if (landscape != null) {
        val turned = landscape && onTop
        DisposableEffect(turned, activity) {
            activity?.requestedOrientation = if (turned) {
                android.content.pm.ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            } else {
                android.content.pm.ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            }
            onDispose {}
        }
    }
    DisposableEffect(context) {
        BernHost.attach(context)
        onDispose { BernHost.detach(context) }
    }
    val settled = remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        withFrameNanos {}
        settled.value = true
    }
    // `window.compact` and its siblings follow the window's width: the
    // screen's, or a dialog's declared one, until the window is laid out.
    val density = LocalDensity.current
    val dialogWidth = BernHost.kinds.firstOrNull { it.id == context.kindID && it.isDialog }?.width
    context.estimatedWidth = dialogWidth ?: LocalConfiguration.current.screenWidthDp.toDouble()
    context.estimatedHeight = LocalConfiguration.current.screenHeightDp.toDouble()
    val editingRegions = remember(context) { BernEditingRegions() }
    val pointer = remember(context) { BernPointer() }
    val safeArea = remember(context) { BernSafeArea() }
    CompositionLocalProvider(LocalBernWindow provides context, LocalBernSettled provides settled,
        LocalBernEditingRegions provides editingRegions, LocalBernPointer provides pointer) {
        val chrome = context.chrome
        val titled = chrome.lastOrNull { it.title != null }?.title
        val title = titled ?: context.title
        val back = chrome.lastOrNull { it.back != null }?.back
        val sheet = context.kindID != BernHost.primary
        Scaffold(
            // The width for `window.compact` and its siblings, and touches
            // for `window.idle`. Keyboard shortcuts and keys come from the
            // activity, which sees them whether or not a control has focus.
            modifier = Modifier.bernEditingPresses(editingRegions).bernPointerWatch(pointer).onSizeChanged { size ->
                context.measuredWidth = with(density) { size.width.toDp().value.toDouble() }
                context.measuredHeight = with(density) { size.height.toDp().value.toDouble() }
            }.then(if (idleAfter == null) {
                Modifier
            } else {
                Modifier.pointerInput(Unit) {
                    awaitPointerEventScope {
                        // Content moving under a still pointer makes Compose
                        // send it again where it was; only a pointer that
                        // moves is input.
                        var last: Offset? = null
                        while (true) {
                            val event = awaitPointerEvent(PointerEventPass.Initial)
                            val change = event.changes.firstOrNull() ?: continue
                            context.mouse = change.type == PointerType.Mouse
                            if (event.type == PointerEventType.Move && change.position == last) continue
                            last = change.position
                            input += 1
                        }
                    }
                }
            }),
            topBar = {
                // A unified window has no bar of its own: it shows one only
                // for what the content puts there, a title, the way back or
                // toolbar items, and never repeats the window's name.
                val bar = !unified || sheet || titled != null || back != null ||
                    chrome.any { it.leading != null || it.actions != null }
                if (!hidden && bar) TopAppBar(
                    title = { if (!unified || titled != null) Text(title, maxLines = 1) },
                    colors = if (context.underBars > 0) {
                        TopAppBarDefaults.topAppBarColors(containerColor = androidx.compose.ui.graphics.Color.Transparent)
                    } else {
                        TopAppBarDefaults.topAppBarColors()
                    },
                    navigationIcon = {
                        // The way back, then the toolbar items at the
                        // leading end.
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            when {
                                back != null -> IconButton(onClick = back) {
                                    Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = null)
                                }
                                sheet -> IconButton(onClick = { BernHost.close(context.kindID) }) {
                                    Icon(Icons.Filled.Close, contentDescription = BernText.string("bern.window.done"))
                                }
                            }
                            chrome.forEach { entry -> entry.leading?.invoke(this) }
                        }
                    },
                    actions = { chrome.forEach { entry -> entry.actions?.invoke(this) } },
                )
            },
        ) { padding ->
            // What is smaller than the window starts at its top leading
            // corner, as on the other targets. The content to the window's
            // edges and inside the bars is the safe area what bleeds passes.
            Box(
                Modifier.onPlaced { safeArea.window = it }.padding(padding).onPlaced { safeArea.safe = it }
                    .consumeWindowInsets(padding).imePadding().fillMaxSize(),
                contentAlignment = Alignment.TopStart,
            ) {
              CompositionLocalProvider(LocalBernBarsPadding provides padding, LocalBernSafeArea provides safeArea) {
                val error = BernHost.startupError
                if (error != null) {
                    Column(Modifier.padding(24.dp)) {
                        Text(BernText.string("bern.startup.failed"), style = MaterialTheme.typography.titleMedium)
                        Text(error)
                    }
                } else {
                    BernHeroRoot(content)
                }
              }
            }
        }
        // Only the window on top goes back; the ones below it wait.
        BackHandler(enabled = back != null && onTop) { back?.invoke() }
        context.pendingClose?.let { pending ->
            AlertDialog(
                onDismissRequest = { BernHost.confirmClose(context, pending.id, 2) { _, _ -> } },
                title = { Text(pending.message) },
                text = {
                    Text(BernText.string(if (pending.canSave) "bern.close.save_detail" else "bern.close.discard_detail"))
                },
                confirmButton = {
                    if (pending.canSave) {
                        TextButton(onClick = { BernHost.confirmClose(context, pending.id, 0) { _, _ -> } }) {
                            Text(BernText.string("bern.close.save"))
                        }
                    }
                    TextButton(onClick = { BernHost.confirmClose(context, pending.id, 1) { _, _ -> } }) {
                        Text(BernText.string(if (pending.canSave) "bern.close.dont_save" else "bern.close.discard"))
                    }
                },
                dismissButton = {
                    TextButton(onClick = { BernHost.confirmClose(context, pending.id, 2) { _, _ -> } }) {
                        Text(BernText.string("bern.close.cancel"))
                    }
                },
            )
        }
        BernHost.failure?.takeIf { it.window == null || it.window == context.id }?.let { failure ->
            AlertDialog(
                onDismissRequest = { BernHost.failure = null },
                title = { Text(BernText.string("bern.alert.failed")) },
                text = { Text(failure.message) },
                confirmButton = {
                    TextButton(onClick = { BernHost.failure = null }) { Text(BernText.string("bern.alert.ok")) }
                },
            )
        }
    }
}
// @bern:wear-skip-end

/// The audio focus of what `bern::now_playing` shows, so that a call or
/// another app's music stops it instead of sounding over it. Playing takes
/// the focus and pausing gives it back. Lost for good, the music pauses; lost
/// for a while, it pauses and the request stays, for the system to say when
/// the focus is back and the music plays again. Where the system can lower
/// the music under a short sound it does so without a word; where it cannot
/// (AAudio's players) it says so here, and with no way to the engine's volume
/// the music pauses for that while. All of this runs on the main thread.
internal object BernAudioFocus {
    val attributes: android.media.AudioAttributes = android.media.AudioAttributes.Builder()
        .setUsage(android.media.AudioAttributes.USAGE_MEDIA)
        .setContentType(android.media.AudioAttributes.CONTENT_TYPE_MUSIC)
        .build()
    private val request by lazy {
        android.media.AudioFocusRequest.Builder(android.media.AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(attributes)
            .setAcceptsDelayedFocusGain(true)
            .setOnAudioFocusChangeListener(::changed, android.os.Handler(android.os.Looper.getMainLooper()))
            .build()
    }
    private var manager: android.media.AudioManager? = null
    /// What the engine showed last.
    private var playing = false
    /// Whether the system has the request, the focus given or still to come.
    private var held = false
    /// A pause the engine has not shown yet: what it shows until then is
    /// late, not the user pressing play.
    private var pausing = false
    /// Paused until the focus comes back. The playback service stays in the
    /// foreground meanwhile: from the background the system would not let it
    /// back when the music plays again.
    var away = false
        private set

    private fun send(name: String) = BernNative.nowPlayingCommand(name.toByteArray())

    private fun pause() {
        if (!playing) return
        pausing = true
        send("pause")
    }

    private fun release() {
        if (held) manager?.abandonAudioFocusRequest(request)
        held = false
        away = false
    }

    private fun changed(change: Int) {
        when (change) {
            android.media.AudioManager.AUDIOFOCUS_GAIN -> if (away) {
                away = false
                send("play")
            }
            android.media.AudioManager.AUDIOFOCUS_LOSS -> pauseForGood()
            android.media.AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
            android.media.AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> if (playing) {
                away = true
                pause()
            }
        }
    }

    /// Pause and stay paused, whatever the focus does: it was lost for good,
    /// or the headphones came out and the music would go on in the room.
    fun pauseForGood() {
        val waited = away && !playing
        release()
        pause()
        // The engine, paused all along, shows nothing new.
        if (waited) BernNowPlaying.stopWaiting()
    }

    /// What the engine shows now.
    fun update(context: Context, now: Boolean) {
        playing = now
        if (!now) {
            pausing = false
            if (!away) release()
            return
        }
        if (pausing || (held && !away)) return
        val manager = manager ?: (context.getSystemService(Context.AUDIO_SERVICE) as android.media.AudioManager).also { manager = it }
        when (manager.requestAudioFocus(request)) {
            android.media.AudioManager.AUDIOFOCUS_REQUEST_GRANTED -> {
                held = true
                away = false
            }
            // A call: the music waits for it to end.
            android.media.AudioManager.AUDIOFOCUS_REQUEST_DELAYED -> {
                held = true
                away = true
                pause()
            }
            // Android 15 and later refuse it to an app behind others with no
            // foreground service, unless the user just pressed play.
            else -> pause()
        }
    }

    /// Nothing plays any more.
    fun stop() {
        playing = false
        pausing = false
        release()
    }
}

/// The system's now playing (`bern::now_playing`): a media session with
/// what plays, the media notification the system's media controls show,
/// and, where the application declares `background_audio`, a foreground
/// service that keeps the music playing behind other apps. The session's
/// commands go to the engine by name.
internal object BernNowPlaying {
    const val ID = 0x6e70
    private const val CHANNEL = "bern.now_playing"
    private const val ACTION = "dev.bern.runtime.NOW_PLAYING"
    private var session: android.media.session.MediaSession? = null
    private var receiver: android.content.BroadcastReceiver? = null
    /// The notification shown last, which the service stands on.
    var notification: android.app.Notification? = null
        private set
    private var playing = false

    private fun send(name: String) = BernNative.nowPlayingCommand(name.toByteArray())

    /// `json` is what the engine shows: title, artist, album, artwork as
    /// base64, duration, position and playing; empty when nothing plays.
    fun update(context: Context, json: String) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager
        if (json.isEmpty()) {
            session?.isActive = false
            playing = false
            BernAudioFocus.stop()
            BernPlaybackService.running?.stop()
            manager.cancel(ID)
            notification = null
            return
        }
        val now = org.json.JSONObject(json)
        val session = session ?: android.media.session.MediaSession(context, "bern").also { created ->
            created.setCallback(object : android.media.session.MediaSession.Callback() {
                override fun onPlay() = send("play")
                override fun onPause() = send("pause")
                override fun onStop() = send("pause")
                override fun onSkipToNext() = send("next")
                override fun onSkipToPrevious() = send("previous")
                override fun onSeekTo(position: Long) = send("seek:${position / 1000.0}")
            })
            // Pressing the song in the system's media controls, on the lock
            // screen or in a maker's island, opens the application.
            created.setSessionActivity(launchIntent(context))
            created.setPlaybackToLocal(BernAudioFocus.attributes)
            session = created
        }
        val artwork = now.optString("artwork").takeIf { it.isNotEmpty() }?.let {
            val bytes = android.util.Base64.decode(it, android.util.Base64.DEFAULT)
            android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
        }
        val title = now.optString("title")
        val artist = now.optString("artist")
        session.setMetadata(
            android.media.MediaMetadata.Builder()
                .putString(android.media.MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(android.media.MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putString(android.media.MediaMetadata.METADATA_KEY_ALBUM, now.optString("album"))
                .putLong(android.media.MediaMetadata.METADATA_KEY_DURATION, (now.optDouble("duration", 0.0) * 1000).toLong())
                .apply {
                    if (artwork != null) {
                        putBitmap(android.media.MediaMetadata.METADATA_KEY_ALBUM_ART, artwork)
                        putBitmap(android.media.MediaMetadata.METADATA_KEY_ART, artwork)
                    }
                }
                .build()
        )
        playing = now.optBoolean("playing")
        BernAudioFocus.update(context, playing)
        val state = android.media.session.PlaybackState.Builder()
            .setActions(
                android.media.session.PlaybackState.ACTION_PLAY or android.media.session.PlaybackState.ACTION_PAUSE or
                    android.media.session.PlaybackState.ACTION_PLAY_PAUSE or android.media.session.PlaybackState.ACTION_STOP or
                    android.media.session.PlaybackState.ACTION_SKIP_TO_NEXT or android.media.session.PlaybackState.ACTION_SKIP_TO_PREVIOUS or
                    android.media.session.PlaybackState.ACTION_SEEK_TO
            )
            .setState(
                if (playing) android.media.session.PlaybackState.STATE_PLAYING else android.media.session.PlaybackState.STATE_PAUSED,
                (now.optDouble("position", 0.0) * 1000).toLong(),
                if (playing) 1f else 0f
            )
            .build()
        session.setPlaybackState(state)
        session.isActive = true
        listen(context)
        if (android.os.Build.VERSION.SDK_INT >= 26 && manager.getNotificationChannel(CHANNEL) == null) {
            val name = context.applicationInfo.loadLabel(context.packageManager)
            manager.createNotificationChannel(
                android.app.NotificationChannel(CHANNEL, name, android.app.NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) }
            )
        }
        val action = { icon: Int, label: String, command: String, code: Int ->
            android.app.Notification.Action.Builder(
                android.graphics.drawable.Icon.createWithResource(context, icon),
                label,
                android.app.PendingIntent.getBroadcast(
                    context, code,
                    android.content.Intent(ACTION).setPackage(context.packageName).putExtra("command", command),
                    android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT
                )
            ).build()
        }
        val launch = launchIntent(context)
        val builder = if (android.os.Build.VERSION.SDK_INT >= 26) android.app.Notification.Builder(context, CHANNEL)
            else @Suppress("DEPRECATION") android.app.Notification.Builder(context)
        val shown = builder
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(title)
            .setContentText(artist)
            .setLargeIcon(artwork)
            .setContentIntent(launch)
            .setOngoing(playing)
            .setShowWhen(false)
            .setVisibility(android.app.Notification.VISIBILITY_PUBLIC)
            .addAction(action(android.R.drawable.ic_media_previous, "Previous", "previous", 1))
            .addAction(
                if (playing) action(android.R.drawable.ic_media_pause, "Pause", "pause", 2)
                else action(android.R.drawable.ic_media_play, "Play", "play", 3)
            )
            .addAction(action(android.R.drawable.ic_media_next, "Next", "next", 4))
            .setStyle(android.app.Notification.MediaStyle().setMediaSession(session.sessionToken).setShowActionsInCompactView(0, 1, 2))
            .build()
        notification = shown
        val service = BernPlaybackService.running
        if (playing && service == null && BernPlaybackService.declared(context)) {
            try {
                val intent = android.content.Intent(context, BernPlaybackService::class.java)
                if (android.os.Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent) else context.startService(intent)
            } catch (error: Exception) {
                manager.notify(ID, shown)
            }
        } else if (service != null) {
            service.show(shown, playing || BernAudioFocus.away)
        } else {
            manager.notify(ID, shown)
        }
    }

    /// The music stays paused after all: the service no longer keeps the
    /// foreground for it.
    fun stopWaiting() {
        notification?.let { BernPlaybackService.running?.show(it, false) }
    }

    private fun launchIntent(context: Context): android.app.PendingIntent? =
        context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
            android.app.PendingIntent.getActivity(context, 0, it, android.app.PendingIntent.FLAG_IMMUTABLE)
        }

    /// The notification's buttons, heard by the application alone, and the
    /// system saying the sound is about to leave the headphones.
    private fun listen(context: Context) {
        if (receiver != null) return
        val heard = object : android.content.BroadcastReceiver() {
            override fun onReceive(context: Context, intent: android.content.Intent) {
                if (intent.action == android.media.AudioManager.ACTION_AUDIO_BECOMING_NOISY) BernAudioFocus.pauseForGood()
                else intent.getStringExtra("command")?.let(::send)
            }
        }
        val filter = android.content.IntentFilter(ACTION).apply { addAction(android.media.AudioManager.ACTION_AUDIO_BECOMING_NOISY) }
        if (android.os.Build.VERSION.SDK_INT >= 33) context.registerReceiver(heard, filter, Context.RECEIVER_NOT_EXPORTED)
        else @Suppress("UnspecifiedRegisterReceiverFlag") context.registerReceiver(heard, filter)
        receiver = heard
    }
}

/// Keeps the application playing while another app is in front: a
/// foreground service of the media playback kind, standing on the now
/// playing notification. With the music stopped it stays in the foreground
/// for a while, since songs change with the music stopped while the next one
/// loads, and behind other apps the system would not let the service back.
/// After that it leaves the foreground and the notification stays.
class BernPlaybackService : android.app.Service() {
    companion object {
        /// How long the foreground outlasts the music, in milliseconds.
        private const val STAYS = 30_000L

        var running: BernPlaybackService? = null
            private set

        /// Whether the application declares the service
        /// (`[app].background_audio`).
        fun declared(context: Context): Boolean = try {
            context.packageManager.getServiceInfo(android.content.ComponentName(context, BernPlaybackService::class.java), 0)
            true
        } catch (error: Exception) {
            false
        }
    }

    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private var foreground = false
    private val leave = Runnable {
        foreground = false
        if (android.os.Build.VERSION.SDK_INT >= 24) stopForeground(android.app.Service.STOP_FOREGROUND_DETACH)
        else @Suppress("DEPRECATION") stopForeground(false)
        BernNowPlaying.notification?.let(::post)
    }

    override fun onBind(intent: android.content.Intent?): android.os.IBinder? = null

    override fun onStartCommand(intent: android.content.Intent?, flags: Int, startId: Int): Int {
        running = this
        val shown = BernNowPlaying.notification ?: run { stopSelf(); return START_NOT_STICKY }
        show(shown, true)
        return START_NOT_STICKY
    }

    fun show(notification: android.app.Notification, playing: Boolean) {
        handler.removeCallbacks(leave)
        if (!playing) {
            if (foreground) handler.postDelayed(leave, STAYS)
            post(notification)
            return
        }
        try {
            if (android.os.Build.VERSION.SDK_INT >= 29) {
                startForeground(BernNowPlaying.ID, notification, android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
            } else {
                startForeground(BernNowPlaying.ID, notification)
            }
            foreground = true
        } catch (error: Exception) {
            // Refused behind other apps: the notification still follows the
            // music.
            post(notification)
        }
    }

    private fun post(notification: android.app.Notification) {
        (getSystemService(Context.NOTIFICATION_SERVICE) as android.app.NotificationManager).notify(BernNowPlaying.ID, notification)
    }

    fun stop() {
        if (android.os.Build.VERSION.SDK_INT >= 24) stopForeground(android.app.Service.STOP_FOREGROUND_REMOVE)
        else @Suppress("DEPRECATION") stopForeground(true)
        stopSelf()
    }

    override fun onDestroy() {
        handler.removeCallbacks(leave)
        running = null
        super.onDestroy()
    }
}

/// Text over other apps (`bern::overlay`): a row of lines with a shadow on a
/// system overlay window, once the user allows drawing over other apps.
/// Locked, touches go through it; unlocked, it shows a plate and drags, a
/// tap shows its bar of buttons over the lines for a few seconds (the
/// window rises by the bar, so the lines stay), and where it was left is
/// kept.
internal object BernOverlay {
    private val views = mutableMapOf<String, android.widget.LinearLayout>()
    private val layouts = mutableMapOf<String, android.view.WindowManager.LayoutParams>()
    /// What each overlay last showed, and those whose bar shows.
    private val shown = mutableMapOf<String, String>()
    private val barred = mutableSetOf<String>()
    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private const val BAR_SECONDS = 5L
    /// The bar's height: its buttons and the room under them.
    private const val BAR_HEIGHT = 48

    /// Mark the bar of `id` as showing or not, raising the window by it or
    /// lowering it back.
    private fun barred(context: Context, id: String, on: Boolean) {
        if (on == barred.contains(id)) return
        if (on) barred.add(id) else barred.remove(id)
        val rise = (BAR_HEIGHT * context.resources.displayMetrics.density).toInt()
        layouts[id]?.let { it.y += if (on) -rise else rise }
    }

    fun allowed(context: Context): Boolean =
        android.os.Build.VERSION.SDK_INT < 23 || android.provider.Settings.canDrawOverlays(context)

    fun ask(context: Context) {
        if (android.os.Build.VERSION.SDK_INT < 23) return
        val intent = android.content.Intent(android.provider.Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}"))
            .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
    }

    private fun kept(context: Context) = context.getSharedPreferences("bern.overlay", Context.MODE_PRIVATE)

    /// Show the bar of `id`, or put it away, and put it away again a few
    /// seconds after.
    private fun bar(context: Context, id: String, on: Boolean) {
        main.removeCallbacksAndMessages(id)
        val buttons = shown[id]?.let { org.json.JSONObject(it).optJSONArray("buttons")?.length() } ?: 0
        barred(context, id, on && buttons > 0)
        if (on && buttons > 0) {
            main.postAtTime({ bar(context, id, false) }, id, android.os.SystemClock.uptimeMillis() + BAR_SECONDS * 1000)
        }
        shown[id]?.let { show(context, id, it) }
    }

    fun show(context: Context, id: String, json: String) {
        if (!allowed(context)) return
        shown[id] = json
        val manager = context.getSystemService(Context.WINDOW_SERVICE) as android.view.WindowManager
        val overlay = org.json.JSONObject(json)
        val lines = overlay.optJSONArray("lines") ?: org.json.JSONArray()
        val buttons = overlay.optJSONArray("buttons") ?: org.json.JSONArray()
        val locked = overlay.optBoolean("locked")
        val density = context.resources.displayMetrics.density
        val layout = layouts.getOrPut(id) {
            android.view.WindowManager.LayoutParams(
                android.view.WindowManager.LayoutParams.WRAP_CONTENT,
                android.view.WindowManager.LayoutParams.WRAP_CONTENT,
                if (android.os.Build.VERSION.SDK_INT >= 26) android.view.WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
                else @Suppress("DEPRECATION") android.view.WindowManager.LayoutParams.TYPE_PHONE,
                android.view.WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                android.graphics.PixelFormat.TRANSLUCENT
            ).apply {
                gravity = android.view.Gravity.TOP or android.view.Gravity.CENTER_HORIZONTAL
                x = kept(context).getInt("$id.x", 0)
                y = kept(context).getInt("$id.y", (context.resources.displayMetrics.heightPixels * 0.18).toInt())
            }
        }
        if (locked) barred(context, id, false)
        val touchable = android.view.WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        layout.flags = if (locked) layout.flags or touchable else layout.flags and touchable.inv()
        val view = views[id] ?: android.widget.LinearLayout(context).also { created ->
            created.orientation = android.widget.LinearLayout.VERTICAL
            created.gravity = android.view.Gravity.CENTER_HORIZONTAL
            val pad = (12 * density).toInt()
            created.setPadding(pad, pad / 2, pad, pad / 2)
            val slop = android.view.ViewConfiguration.get(context).scaledTouchSlop
            var startX = 0
            var startY = 0
            var downX = 0f
            var downY = 0f
            var moved = false
            created.setOnTouchListener { _, event ->
                when (event.action) {
                    android.view.MotionEvent.ACTION_DOWN -> {
                        startX = layout.x; startY = layout.y; downX = event.rawX; downY = event.rawY; moved = false
                    }
                    android.view.MotionEvent.ACTION_MOVE -> {
                        if (Math.abs(event.rawX - downX) > slop || Math.abs(event.rawY - downY) > slop) moved = true
                        if (moved) {
                            layout.x = startX + (event.rawX - downX).toInt()
                            layout.y = startY + (event.rawY - downY).toInt()
                            manager.updateViewLayout(created, layout)
                        }
                    }
                    android.view.MotionEvent.ACTION_UP ->
                        if (moved) {
                            // Where the lines are, without the bar over them.
                            val rise = if (barred.contains(id)) (BAR_HEIGHT * density).toInt() else 0
                            kept(context).edit().putInt("$id.x", layout.x).putInt("$id.y", layout.y + rise).apply()
                        } else bar(context, id, !barred.contains(id))
                }
                true
            }
            manager.addView(created, layout)
            views[id] = created
        }
        view.removeAllViews()
        val withBar = barred.contains(id) && buttons.length() > 0 && overlay.optString("place") != "taskbar"
        if (withBar) {
            val row = android.widget.LinearLayout(context).apply {
                orientation = android.widget.LinearLayout.HORIZONTAL
                gravity = android.view.Gravity.CENTER_HORIZONTAL
                setPadding(0, 0, 0, ((BAR_HEIGHT - 36) * density).toInt())
            }
            val side = (36 * density).toInt()
            for (i in 0 until buttons.length()) {
                val button = buttons.getJSONObject(i)
                val buttonId = button.optString("id")
                row.addView(android.widget.ImageView(context).apply {
                    setImageDrawable(BernOverlaySymbol(button.optString("symbol")))
                    val inset = (9 * density).toInt()
                    setPadding(inset, inset, inset, inset)
                    background = android.graphics.drawable.GradientDrawable().apply {
                        shape = android.graphics.drawable.GradientDrawable.OVAL
                        setColor(0x4D000000)
                    }
                    contentDescription = button.optString("symbol")
                    setOnClickListener {
                        BernNative.overlayPressed(id.toByteArray(), buttonId.toByteArray())
                        bar(context, id, true)
                    }
                }, android.widget.LinearLayout.LayoutParams(side, side).apply {
                    if (i > 0) marginStart = (10 * density).toInt()
                })
            }
            view.addView(row)
        }
        for (i in 0 until lines.length()) {
            val line = lines.getJSONObject(i)
            val text = line.optString("text")
            if (text.isEmpty()) continue
            val color = android.graphics.Color.parseColor(line.optString("color", "#ffffffff"))
            view.addView(BernLitLine(context, line.optDouble("lit", -1.0), color, bernLineRoute(line), line.optString("ruby")).apply {
                this.text = text
                setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, line.optDouble("size", 18.0).toFloat())
                setTextColor(color)
                if (line.optBoolean("bold")) setTypeface(typeface, android.graphics.Typeface.BOLD)
                setShadowLayer(4 * density, 0f, 1.5f * density, 0xD9000000.toInt())
                gravity = android.view.Gravity.CENTER_HORIZONTAL
            })
        }
        val hasLines = view.childCount > (if (withBar) 1 else 0)
        view.background = if (locked) null else android.graphics.drawable.GradientDrawable().apply {
            setColor(0x4D000000)
            cornerRadius = 12 * density
        }
        view.visibility = if (hasLines) android.view.View.VISIBLE else android.view.View.GONE
        manager.updateViewLayout(view, layout)
    }

    /// Only how far the lines are lit changed: they draw again.
    fun lit(id: String, json: String) {
        shown[id] = json
        val view = views[id] ?: return
        val lines = org.json.JSONObject(json).optJSONArray("lines") ?: return
        var child = 0
        for (i in 0 until lines.length()) {
            val line = lines.getJSONObject(i)
            if (line.optString("text").isEmpty()) continue
            while (child < view.childCount && view.getChildAt(child) !is BernLitLine) child += 1
            if (child == view.childCount) return
            (view.getChildAt(child) as BernLitLine).light(line.optDouble("lit", -1.0), bernLineRoute(line))
            child += 1
        }
    }

    fun hide(context: Context, id: String) {
        main.removeCallbacksAndMessages(id)
        barred(context, id, false)
        shown.remove(id)
        val view = views.remove(id) ?: return
        layouts.remove(id)
        (context.getSystemService(Context.WINDOW_SERVICE) as android.view.WindowManager).removeView(view)
    }
}

/// A symbol of an overlay's bar, drawn in white in a 24-unit square: the
/// ones `bern::overlay::Button` names.
internal class BernOverlaySymbol(private val symbol: String) : android.graphics.drawable.Drawable() {
    private val fill = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFFFFFFF.toInt() }
    private val stroke = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
        color = 0xFFFFFFFF.toInt()
        style = android.graphics.Paint.Style.STROKE
        strokeWidth = 2.4f
        strokeCap = android.graphics.Paint.Cap.ROUND
    }

    override fun draw(canvas: android.graphics.Canvas) {
        val side = minOf(bounds.width(), bounds.height()).toFloat()
        canvas.save()
        canvas.translate(bounds.left + (bounds.width() - side) / 2f, bounds.top + (bounds.height() - side) / 2f)
        canvas.scale(side / 24f, side / 24f)
        fun triangle(path: android.graphics.Path, vararg points: Float) {
            path.moveTo(points[0], points[1]); path.lineTo(points[2], points[3]); path.lineTo(points[4], points[5]); path.close()
        }
        val solid = android.graphics.Path()
        val line = android.graphics.Path()
        val cw = android.graphics.Path.Direction.CW
        when (symbol) {
            "play.fill" -> triangle(solid, 7f, 4f, 20f, 12f, 7f, 20f)
            "pause.fill" -> {
                solid.addRoundRect(android.graphics.RectF(6f, 4f, 10f, 20f), 1f, 1f, cw)
                solid.addRoundRect(android.graphics.RectF(14f, 4f, 18f, 20f), 1f, 1f, cw)
            }
            "forward.fill" -> { triangle(solid, 3f, 5f, 12f, 12f, 3f, 19f); triangle(solid, 12f, 5f, 21f, 12f, 12f, 19f) }
            "backward.fill" -> { triangle(solid, 21f, 5f, 12f, 12f, 21f, 19f); triangle(solid, 12f, 5f, 3f, 12f, 12f, 19f) }
            "lock.fill", "lock.open.fill" -> {
                solid.addRoundRect(android.graphics.RectF(5f, 11f, 19f, 21f), 2f, 2f, cw)
                if (symbol == "lock.fill") {
                    line.moveTo(8f, 11f); line.lineTo(8f, 7f)
                    line.arcTo(android.graphics.RectF(8f, 3f, 16f, 11f), 180f, 180f); line.lineTo(16f, 11f)
                } else {
                    line.moveTo(16f, 11f); line.lineTo(16f, 5f)
                    line.arcTo(android.graphics.RectF(8f, 1f, 16f, 9f), 0f, -180f); line.lineTo(8f, 7f)
                }
            }
            "xmark" -> { line.moveTo(6f, 6f); line.lineTo(18f, 18f); line.moveTo(18f, 6f); line.lineTo(6f, 18f) }
        }
        canvas.drawPath(solid, fill)
        canvas.drawPath(line, stroke)
        canvas.restore()
    }

    override fun setAlpha(alpha: Int) {
        fill.alpha = alpha
        stroke.alpha = alpha
    }

    override fun setColorFilter(filter: android.graphics.ColorFilter?) {
        fill.colorFilter = filter
        stroke.colorFilter = filter
    }

    @Deprecated("Deprecated in Java")
    override fun getOpacity() = android.graphics.PixelFormat.TRANSLUCENT
}

/// `Text(lit_path:)`: seconds and characters, in order, apart by spaces. The
/// place moves evenly from each to the next, and stays at the first before
/// it and the last after it. `clock` is the second it showed at `since`, in
/// seconds since the Unix epoch; a `since` of zero stands still.
internal class BernLitRoute(path: String, private val clock: Double, private val since: Double) {
    private val numbers = path.split(' ').mapNotNull { it.toDoubleOrNull() }
    val empty get() = numbers.size < 2
    val moving get() = since > 0 && !empty

    fun place(): Double = at(clock + if (since > 0) System.currentTimeMillis() / 1000.0 - since else 0.0)

    private fun at(second: Double): Double {
        if (second <= numbers[0]) return numbers[1]
        var index = 2
        while (index + 1 < numbers.size) {
            if (second < numbers[index]) {
                val part = (second - numbers[index - 2]) / (numbers[index] - numbers[index - 2])
                return numbers[index - 1] + (numbers[index + 1] - numbers[index - 1]) * part
            }
            index += 2
        }
        return numbers[numbers.size - 1]
    }
}

/// How far along a character a lit edge moves between frames drawn: about a
/// pixel, and nothing is drawn while it waits between words.
internal const val BERN_LIT_STEP = 1.0 / 48

/// The route of a line of an overlay as JSON has it, if it has one.
private fun bernLineRoute(line: org.json.JSONObject): BernLitRoute? {
    val route = BernLitRoute(line.optString("path"), line.optDouble("clock", 0.0), line.optDouble("since", 0.0))
    return if (route.empty) null else route
}

/// A line of an overlay: as far as `lit` in `color`, or along `route` by
/// itself, the rest faded, the edge inside the character it stops in. All of
/// it below zero.
private class BernLitLine(
    context: android.content.Context, private var number: Double, private val color: Int,
    private var route: BernLitRoute?, private val ruby: String = "",
) : android.widget.TextView(context) {
    private var rubySpans = emptyList<BernRuby>()
    private var rubyHeight = 0

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        rubySpans = bernRuby(text.toString(), ruby)
        if (rubySpans.isNotEmpty()) {
            val annotation = android.text.TextPaint(paint).apply { textSize = paint.textSize * 0.5f }
            val metrics = annotation.fontMetrics
            val height = kotlin.math.ceil((metrics.bottom - metrics.top + resources.displayMetrics.density).toDouble()).toInt()
            if (height != rubyHeight) {
                rubyHeight = height
                setPadding(paddingLeft, height, paddingRight, paddingBottom)
                setLineSpacing(height.toFloat(), 1f)
            }
        }
        super.onMeasure(widthMeasureSpec, heightMeasureSpec)
    }

    private var drawn = -1.0
    private val frame = object : Runnable {
        override fun run() {
            val moving = route?.takeIf { it.moving } ?: return
            if (kotlin.math.abs(moving.place() - drawn) >= BERN_LIT_STEP) invalidate()
            postOnAnimation(this)
        }
    }

    fun light(value: Double, moving: BernLitRoute?) {
        number = value
        route = moving
        invalidate()
        removeCallbacks(frame)
        if (isAttachedToWindow) postOnAnimation(frame)
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        postOnAnimation(frame)
    }

    override fun onDetachedFromWindow() {
        removeCallbacks(frame)
        super.onDetachedFromWindow()
    }

    private fun drawRuby(canvas: android.graphics.Canvas, shown: String, lines: android.text.Layout, lit: Double) {
        val faded = (color and 0x00FFFFFF) or ((android.graphics.Color.alpha(color) * 0.4).toInt() shl 24)
        val count = shown.codePointCount(0, shown.length)
        val whole = if (lit.isFinite()) lit.toInt().coerceIn(0, count) else count
        val edge = shown.offsetByCodePoints(0, whole)
        val annotation = android.text.TextPaint(paint).apply {
            shader = null
            textSize = paint.textSize * 0.5f
        }
        val saved = canvas.save()
        canvas.translate(compoundPaddingLeft.toFloat(), extendedPaddingTop.toFloat())
        for (line in 0 until lines.lineCount) {
            val top = lines.getLineTop(line).toFloat()
            val bottom = lines.getLineBottom(line).toFloat()
            val start = lines.getLineStart(line)
            val end = lines.getLineVisibleEnd(line)
            fun horizontal(offset: Int): Float = if (offset >= lines.getLineEnd(line)) {
                if (lines.getParagraphDirection(line) > 0) lines.getLineRight(line) else lines.getLineLeft(line)
            } else lines.getPrimaryHorizontal(offset)
            val clipped = canvas.save()
            canvas.clipRect(0f, top, lines.width.toFloat(), bottom)
            paint.shader = null
            paint.color = if (lit < 0) color else faded
            lines.draw(canvas)
            if (lit >= 0) {
                var offset = start
                while (offset < end && offset <= edge) {
                    val next = shown.offsetByCodePoints(offset, 1)
                    val a = horizontal(offset)
                    val b = horizontal(next)
                    val fraction = if (offset < edge) 1f else (lit - whole).toFloat().coerceIn(0f, 1f)
                    val glyph = canvas.save()
                    val left = minOf(a, b)
                    canvas.clipRect(left, top, left + kotlin.math.abs(b - a) * fraction, bottom)
                    paint.color = color
                    lines.draw(canvas)
                    canvas.restoreToCount(glyph)
                    offset = next
                }
            }
            canvas.restoreToCount(clipped)
            val fragments = rubySpans.mapNotNull { span ->
                val a = maxOf(start, span.start)
                val b = minOf(end, span.end)
                if (a >= b) null else {
                    val reading = bernRubyReading(shown, span, a, b)
                    if (reading.isEmpty()) null else Triple(a to b, reading,
                        (horizontal(a) + horizontal(b)) / 2)
                }
            }.sortedBy { it.third }
            val centers = fragments.map { it.third }
            fragments.forEachIndexed { index, fragment ->
                val slot = bernRubySlot(centers, index, lines.width.toFloat())
                annotation.textScaleX = 1f
                val natural = annotation.measureText(fragment.second)
                annotation.textScaleX = minOf(1f, (slot.second - slot.first - resources.displayMetrics.density).coerceAtLeast(0f) / natural.coerceAtLeast(1f))
                val width = annotation.measureText(fragment.second)
                val x = (fragment.third - width / 2).coerceIn(slot.first, maxOf(slot.first, slot.second - width))
                val y = lines.getLineBaseline(line) + paint.fontMetrics.ascent - annotation.fontMetrics.descent
                annotation.color = if (lit < 0) color else faded
                canvas.drawText(fragment.second, x, y, annotation)
                if (lit >= 0) {
                    val source = shown.codePointCount(0, fragment.first.first)
                    val length = shown.codePointCount(fragment.first.first, fragment.first.second)
                    val progress = ((lit - source) / length).toFloat().coerceIn(0f, 1f)
                    val clip = canvas.save()
                    canvas.clipRect(x, y + annotation.fontMetrics.top, x + width * progress, y + annotation.fontMetrics.bottom)
                    annotation.color = color
                    canvas.drawText(fragment.second, x, y, annotation)
                    canvas.restoreToCount(clip)
                }
            }
        }
        paint.shader = null
        paint.color = color
        canvas.restoreToCount(saved)
    }

    override fun onDraw(canvas: android.graphics.Canvas) {
        val shown = text.toString()
        val lines = layout
        val lit = route?.place() ?: number
        drawn = lit
        if (rubySpans.isNotEmpty() && lines != null) {
            drawRuby(canvas, shown, lines, lit)
            return
        }
        if (lit >= 0 && lines != null) {
            val whole = lit.toInt().coerceAtMost(shown.codePointCount(0, shown.length))
            val at = shown.offsetByCodePoints(0, whole)
            val left = lines.getPrimaryHorizontal(at)
            val right = if (at < shown.length) lines.getPrimaryHorizontal(shown.offsetByCodePoints(at, 1)) else left
            val end = left + (right - left) * (lit - lit.toInt()).toFloat()
            val faded = (color and 0x00FFFFFF) or ((android.graphics.Color.alpha(color) * 0.4).toInt() shl 24)
            paint.shader = android.graphics.LinearGradient(
                end - 0.5f, 0f, end + 0.5f, 0f, color, faded, android.graphics.Shader.TileMode.CLAMP,
            )
        }
        super.onDraw(canvas)
    }
}
