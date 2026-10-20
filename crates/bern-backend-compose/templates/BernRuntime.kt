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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlinx.coroutines.delay
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

    internal fun setup(supported: List<String>, default: String) {
        this.supported = supported
        current = initial(supported) ?: default
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
        val pieces = BernTextParser(string(key)).message(branch = false)
        return write(pieces, BernLanguage.current, arguments, hash = null)
    }

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
    var idle by mutableStateOf(false)
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
    /// `compact`, `medium`, `expanded` or `large`.
    val size: String
        get() = BernHost.layouts.sizeOf(measuredWidth ?: estimatedWidth)
    val compact: Boolean get() = size == "compact"
    val medium: Boolean get() = size == "medium"
    val expanded: Boolean get() = size == "expanded"
    val large: Boolean get() = size == "large"
    /// `window.tv`: the window is on a television.
    val tv: Boolean get() = BernHost.television
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

/// The room the app bar and the system bars take around a window's content.
val LocalBernBarsPadding = staticCompositionLocalOf { androidx.compose.foundation.layout.PaddingValues() }

/// `under_bars`: the layer fills the window to its edges while it is shown,
/// reaching out over the room the app bar and the system bars take; the
/// app bar lets it show through.
@Composable
fun Modifier.bernUnderBars(): Modifier {
    val window = LocalBernWindow.current
    val padding = LocalBernBarsPadding.current
    DisposableEffect(window) {
        window.underBars += 1
        onDispose { window.underBars -= 1 }
    }
    val density = LocalDensity.current
    val top = with(density) { padding.calculateTopPadding().roundToPx() }
    val bottom = with(density) { padding.calculateBottomPadding().roundToPx() }
    return this.fillMaxSize().layout { measurable, constraints ->
        val height = if (constraints.hasBoundedHeight) constraints.maxHeight + top + bottom else constraints.maxHeight
        val placeable = measurable.measure(constraints.copy(minHeight = height, maxHeight = height))
        layout(constraints.maxWidth, constraints.maxHeight) { placeable.place(0, -top) }
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
    external fun mediaExecutorAttached(app: Long, bytes: ByteArray): ByteArray
    external fun mediaSnapshot(app: Long): ByteArray
    external fun mediaDispatch(app: Long, command: Int, params: ByteArray, token: Long)
    external fun mediaCreate(app: Long, bytes: ByteArray): ByteArray
    external fun mediaBind(app: Long, bytes: ByteArray): ByteArray
    external fun mediaRelease(app: Long, bytes: ByteArray)
    external fun mediaView(app: Long, bytes: ByteArray)
    external fun mediaComplete(app: Long, bytes: ByteArray)
    external fun mediaEvent(app: Long, bytes: ByteArray)
}

/// Callbacks of the Rust runtime. They arrive on engine and channel
/// threads; the work moves to the main thread, except the file questions,
/// which the calling thread waits for.
@Keep
class BernBridge(context: Context) {
    private val main = Handler(Looper.getMainLooper())
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
    /// as a `content:` file from the picker; null when it has none.
    @Keep
    fun fileName(uri: String): String? = try {
        documents.query(Uri.parse(uri), arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
    } catch (error: Exception) {
        null
    }

    /// A descriptor the engine reads a document through, which it closes;
    /// -1 when the document cannot be opened.
    @Keep
    fun openFile(uri: String): Int = try {
        documents.openFileDescriptor(Uri.parse(uri), "r")?.detachFd() ?: -1
    } catch (error: Exception) {
        -1
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
        private set

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
        if (handle != 0L) return
        television = (context.resources.configuration.uiMode and Configuration.UI_MODE_TYPE_MASK) ==
            Configuration.UI_MODE_TYPE_TELEVISION ||
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
        BernText.load(context, languages)
        BernLanguage.setup(languages, defaultLanguage)
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
                failure = BernFailure(pending.window, code, message)
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
    // while `idleEnabled` holds; switched off, the window is not idle.
    var input by remember { mutableLongStateOf(0L) }
    if (idleAfter != null) {
        LaunchedEffect(input, BernShortcuts.keys, idleEnabled) {
            context.idle = false
            if (!idleEnabled) return@LaunchedEffect
            delay((idleAfter * 1000).toLong())
            context.idle = true
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
    CompositionLocalProvider(LocalBernWindow provides context, LocalBernSettled provides settled) {
        val chrome = context.chrome
        val titled = chrome.lastOrNull { it.title != null }?.title
        val title = titled ?: context.title
        val back = chrome.lastOrNull { it.back != null }?.back
        val sheet = context.kindID != BernHost.primary
        Scaffold(
            // The width for `window.compact` and its siblings, and touches
            // for `window.idle`. Keyboard shortcuts and keys come from the
            // activity, which sees them whether or not a control has focus.
            modifier = Modifier.onSizeChanged { size ->
                context.measuredWidth = with(density) { size.width.toDp().value.toDouble() }
                context.measuredHeight = with(density) { size.height.toDp().value.toDouble() }
            }.then(if (idleAfter == null) {
                Modifier
            } else {
                Modifier.pointerInput(Unit) {
                    awaitPointerEventScope {
                        while (true) {
                            awaitPointerEvent(PointerEventPass.Initial)
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
            Box(Modifier.padding(padding).consumeWindowInsets(padding).imePadding().fillMaxSize(), contentAlignment = Alignment.TopCenter) {
              CompositionLocalProvider(LocalBernBarsPadding provides padding) {
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
