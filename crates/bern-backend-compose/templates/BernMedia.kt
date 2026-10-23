// Bern media runtime for Compose. Generated build input; do not edit.
//
// Media3's ExoPlayer is the media executor: it performs the requests the
// Rust media hub routes to it and reports what it observes, tagged with the
// source version it played. Reports are facts; nothing here turns them into
// commands. Video views show a player without its own controls, so every
// playback change starts as a Bern media command. The control bar uses
// Material components in the system's colors; video pixels keep their own.

package dev.bern.runtime

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.AspectRatioFrameLayout
import androidx.media3.ui.PlayerView
import androidx.compose.ui.draw.clip
import java.io.File

// MARK: - Merged player state

/// Playback phases, in the order of the media contract's `MediaPhase`.
enum class BernMediaPhase { Idle, Loading, Ready, Playing, Paused, Buffering, Ended, Failed, Unavailable }

/// One `MediaPlayer` record of the media contract, decoded field by field
/// in the contract's order.
data class BernPlayerState(
    val playerID: String,
    val owner: String,
    val sourceBound: Boolean,
    val sourceURL: String?,
    val sourceVersion: Long,
    val phase: BernMediaPhase,
    val positionMs: Long,
    val durationMs: Long?,
    val seekable: Boolean,
    val volume: Double,
    val muted: Boolean,
    val error: String?,
    val views: Long,
) {
    companion object {
        fun decode(reader: BernReader): BernPlayerState {
            val state = BernPlayerState(
                reader.text(), reader.text(), reader.bool(), reader.optional { reader.text() }, reader.i64(),
                BernMediaPhase.entries.getOrNull(reader.u32()) ?: throw BernDecodeError("unknown media phase"),
                reader.i64(), reader.optional { reader.i64() }, reader.bool(), reader.f64(), reader.bool(),
                reader.optional { reader.text() }, reader.i64(),
            )
            // The latest request (command, status, version, error) is only
            // shown to external entries.
            if (reader.u8() == 1) {
                reader.text()
                reader.u32()
                reader.i64()
                reader.optional { reader.text() }
            }
            return state
        }
    }
}

/// Media commands, in the order of the media catalog.
enum class BernMediaCommand { Players, State, Open, Play, Pause, Seek, SetVolume, SetMuted }

/// Player identities, merged states and the commands of the media hub.
object BernMedia {
    val players = mutableStateMapOf<String, BernPlayerState>()
    private var revision = 0L
    lateinit var backend: BernMediaBackend
        private set
    private val surfaces = HashMap<String, Pair<PlayerView, String>>()

    fun receive(bytes: ByteArray) {
        val reader = BernReader(bytes)
        try {
            val next = reader.u64()
            // Updates carry every player; only the newest revision counts.
            if (next < revision) return
            val received = List(reader.u32()) { BernPlayerState.decode(reader) }.associateBy { it.playerID }
            revision = next
            players.keys.retainAll(received.keys)
            for ((id, state) in received) if (players[id] != state) players[id] = state
        } catch (error: Exception) {
            BernHost.failure = BernFailure(null, "invalid_media_state", error.toString())
        }
    }

    /// Attach Media3 as the executor of every player. One player shows in
    /// one view at a time, and stops being reliable in the background, where
    /// Android needs a media service.
    fun attachExecutor(context: Context) {
        backend = BernMediaBackend(context.applicationContext)
        val writer = BernWriter()
        writer.text("media3")
        writer.text("1")
        writer.u8(0)
        writer.u8(0)
        BernNative.mediaExecutorAttached(BernHost.handle, writer.bytes())
        receive(BernNative.mediaSnapshot(BernHost.handle))
    }

    fun command(command: BernMediaCommand, player: String, window: String?, encode: (BernWriter) -> Unit = {}) {
        BernHost.dispatchMedia(command.ordinal, window) { writer ->
            writer.text(player)
            encode(writer)
        }
    }

    fun open(player: String, url: String, window: String?) = command(BernMediaCommand.Open, player, window) { it.text(url) }

    fun seek(player: String, positionMs: Long, window: String?) =
        command(BernMediaCommand.Seek, player, window) { it.i64(positionMs) }

    fun setVolume(player: String, volume: Double, window: String?) =
        command(BernMediaCommand.SetVolume, player, window) { it.f64(volume) }

    fun setMuted(player: String, muted: Boolean, window: String?) =
        command(BernMediaCommand.SetMuted, player, window) { it.bool(muted) }

    /// Create the player of a `Video(source: …)` declaration.
    fun create(player: String, owner: String): BernCommandFailure? {
        val writer = BernWriter()
        writer.text(player)
        writer.text(owner)
        return BernHost.status(BernNative.mediaCreate(BernHost.handle, writer.bytes()))
    }

    /// Report the declaration's current source; the hub loads only changes.
    fun bind(player: String, url: String?): BernCommandFailure? {
        val writer = BernWriter()
        writer.text(player)
        writer.optional(url) { writer.text(it) }
        return BernHost.status(BernNative.mediaBind(BernHost.handle, writer.bytes()))
    }

    fun release(player: String) {
        val writer = BernWriter()
        writer.text(player)
        BernNative.mediaRelease(BernHost.handle, writer.bytes())
    }

    fun attach(view: PlayerView, token: String, player: String, window: String) {
        view.player = backend.player(player).player
        surfaces[token] = view to window
        val writer = BernWriter()
        writer.text(token)
        writer.u8(1)
        writer.text(player)
        writer.text(window)
        BernNative.mediaView(BernHost.handle, writer.bytes())
    }

    fun detach(token: String) {
        val (view, _) = surfaces.remove(token) ?: return
        view.player = null
        val writer = BernWriter()
        writer.text(token)
        writer.u8(0)
        BernNative.mediaView(BernHost.handle, writer.bytes())
    }

    /// A window instance closed. Its views stop presenting at once.
    fun windowClosed(window: String) {
        val closed = surfaces.filterValues { it.second == window }.keys
        for (token in closed) detach(token)
    }
}

// MARK: - Media3 executor

/// Runs the requests of the media hub with one ExoPlayer per player id.
class BernMediaBackend(private val context: Context) {
    private val players = HashMap<String, BernExoPlayer>()

    fun player(id: String): BernExoPlayer = players.getOrPut(id) { BernExoPlayer(id, context) }

    /// Perform one request: request u64, player text, source version u64,
    /// operation u32 and its payload, as encoded by the media contract.
    fun perform(bytes: ByteArray) {
        val reader = BernReader(bytes)
        val request: Long
        val id: String
        val version: Long
        val operation: Int
        try {
            request = reader.u64()
            id = reader.text()
            version = reader.u64()
            operation = reader.u32()
        } catch (_: Exception) {
            return
        }
        try {
            when (operation) {
                0 -> player(id).load(uri(reader.text()), version)
                1 -> player(id).unload(version)
                7 -> players.remove(id)?.release()
                else -> {
                    val target = player(id)
                    if (target.version != version) throw BernCommandFailure("stale_source", "a newer source is loaded")
                    when (operation) {
                        2 -> target.play()
                        3 -> target.pause()
                        4 -> target.seek(reader.i64())
                        5 -> target.setVolume(reader.f64())
                        6 -> target.setMuted(reader.u8() != 0)
                        else -> throw BernCommandFailure("unknown_command", "unknown media operation $operation")
                    }
                }
            }
            complete(request, null)
        } catch (failure: BernCommandFailure) {
            complete(request, failure)
        } catch (error: Exception) {
            complete(request, BernCommandFailure("invalid_request", error.toString()))
        }
    }

    private fun complete(request: Long, failure: BernCommandFailure?) {
        val writer = BernWriter()
        writer.u64(request)
        if (failure != null) {
            writer.u8(1)
            writer.text(failure.code)
            writer.text(failure.message)
        } else {
            writer.u8(0)
        }
        BernNative.mediaComplete(BernHost.handle, writer.bytes())
    }

    private fun uri(text: String): Uri {
        if (text.startsWith("/")) return Uri.fromFile(File(text))
        val uri = Uri.parse(text)
        if (uri.scheme.isNullOrEmpty()) throw BernCommandFailure("invalid_source", "`$text` is not a media URL")
        return uri
    }
}

/// One ExoPlayer and the facts it reports for its current source version.
class BernExoPlayer(private val id: String, context: Context) {
    val player: ExoPlayer = ExoPlayer.Builder(context).build()
    var version = 0L
        private set
    private var started = false
    private var volume = 1.0
    private var muted = false
    private val main = Handler(Looper.getMainLooper())
    private val ticker = object : Runnable {
        override fun run() {
            report()
            if (player.isPlaying) main.postDelayed(this, 500)
        }
    }

    init {
        player.playWhenReady = false
        player.addListener(object : Player.Listener {
            override fun onEvents(player: Player, events: Player.Events) {
                report()
                if (player.isPlaying) {
                    main.removeCallbacks(ticker)
                    main.postDelayed(ticker, 500)
                }
            }
        })
    }

    fun load(uri: Uri, version: Long) {
        this.version = version
        started = false
        // A new source waits for a play command instead of starting.
        player.playWhenReady = false
        player.setMediaItem(MediaItem.fromUri(uri))
        player.prepare()
        report()
    }

    fun unload(version: Long) {
        this.version = version
        started = false
        player.stop()
        player.clearMediaItems()
        report()
    }

    fun play() {
        if (player.playbackState == Player.STATE_ENDED) player.seekTo(0)
        started = true
        player.play()
    }

    fun pause() = player.pause()

    fun seek(positionMs: Long) {
        player.seekTo(positionMs)
        report()
    }

    fun setVolume(volume: Double) {
        this.volume = volume.coerceIn(0.0, 1.0)
        apply()
    }

    fun setMuted(muted: Boolean) {
        this.muted = muted
        apply()
    }

    private fun apply() {
        player.volume = if (muted) 0f else volume.toFloat()
        report()
    }

    fun release() {
        main.removeCallbacks(ticker)
        player.release()
    }

    /// Report the current facts.
    private fun report() {
        val error = player.playerError
        val phase = when {
            error != null -> BernMediaPhase.Failed
            player.mediaItemCount == 0 -> BernMediaPhase.Idle
            player.playbackState == Player.STATE_ENDED -> BernMediaPhase.Ended
            player.playbackState == Player.STATE_BUFFERING && !started -> BernMediaPhase.Loading
            player.playbackState == Player.STATE_BUFFERING -> BernMediaPhase.Buffering
            player.playbackState == Player.STATE_IDLE -> BernMediaPhase.Loading
            player.isPlaying -> BernMediaPhase.Playing.also { started = true }
            started -> BernMediaPhase.Paused
            else -> BernMediaPhase.Ready
        }
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it >= 0 }
        val writer = BernWriter()
        writer.text(id)
        writer.u64(version)
        writer.u32(phase.ordinal)
        writer.i64(maxOf(player.currentPosition, 0))
        writer.optional(duration) { writer.i64(it) }
        writer.bool(duration != null && player.isCurrentMediaItemSeekable)
        writer.f64(Math.round(volume * 1000) / 1000.0)
        writer.bool(muted)
        writer.optional(error?.let(::describe)) { writer.text(it) }
        BernNative.mediaEvent(BernHost.handle, writer.bytes())
    }

    private fun describe(error: PlaybackException): String = error.message ?: error.errorCodeName
}

// MARK: - Video declaration

/// How a `Video` gets its player: one created for the declaration that
/// follows its source binding, or an application player.
sealed class BernVideoBinding {
    data class Source(val url: String?) : BernVideoBinding()
    data class Player(val id: String) : BernVideoBinding()
}

/// How video fills its frame: `contain` letterboxes, `cover` crops and
/// `fill` stretches.
enum class BernVideoFit { Contain, Cover, Fill }

@Composable
fun BernVideo(
    target: String,
    label: String?,
    binding: BernVideoBinding,
    controls: Boolean,
    fit: BernVideoFit,
    corner: androidx.compose.ui.unit.Dp? = null,
    fillsContainer: Boolean = false,
) {
    val window = LocalBernWindow.current
    val playerID = when (binding) {
        is BernVideoBinding.Player -> binding.id
        is BernVideoBinding.Source -> "${window.id}/$target"
    }
    val state = BernMedia.players[playerID]
    val name = label ?: BernText.string("bern.media.video")
    BernControl(target, "video", name, actions = BernControlActions(value = { BernMedia.players[playerID]?.let(::describe) }))
    // The declaration's player: created once, released when the view's
    // identity ends.
    if (binding is BernVideoBinding.Source && window.id.isNotEmpty()) {
        DisposableEffect(playerID) {
            val failure = BernMedia.create(playerID, window.id)
            if (failure != null) BernHost.failure = BernFailure(window.id, failure.code, failure.message)
            onDispose { if (failure == null) BernMedia.release(playerID) }
        }
        LaunchedEffect(playerID, binding.url) {
            BernMedia.bind(playerID, binding.url)?.let {
                BernHost.failure = BernFailure(window.id, it.code, it.message)
            }
        }
    }
    Column(if (fillsContainer) Modifier.fillMaxSize() else Modifier.fillMaxWidth()) {
        Box(
            Modifier
                .fillMaxWidth()
                .then(if (fillsContainer) Modifier.weight(1f) else Modifier.aspectRatio(16f / 9f))
                .then(if (corner != null) Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(corner)) else Modifier)
                .background(Color.Black)
                .semantics {
                    contentDescription = name
                    stateDescription = state?.let(::describe) ?: ""
                },
            contentAlignment = Alignment.Center,
        ) {
            if (state != null && window.id.isNotEmpty()) {
                BernVideoSurface(playerID, window.id, fit, showsVideo(state), clipped = corner != null)
            }
            BernVideoStatus(state)
        }
        if (controls) BernVideoControls(target, playerID, state)
    }
}

private fun showsVideo(state: BernPlayerState?): Boolean = when (state?.phase) {
    BernMediaPhase.Ready, BernMediaPhase.Playing, BernMediaPhase.Paused, BernMediaPhase.Buffering, BernMediaPhase.Ended -> true
    else -> false
}

private fun describe(state: BernPlayerState): String {
    val duration = state.durationMs?.let(::bernTime) ?: "--:--"
    return "${state.phase.name} ${bernTime(state.positionMs)} / $duration"
}

fun bernTime(milliseconds: Long): String {
    val seconds = maxOf(milliseconds, 0) / 1000
    val hours = seconds / 3600
    val minutes = (seconds % 3600) / 60
    val rest = seconds % 60
    return if (hours > 0) "%d:%02d:%02d".format(hours, minutes, rest) else "%d:%02d".format(minutes, rest)
}

/// The player's picture in a Media3 view without its own controls.
@OptIn(UnstableApi::class)
@Composable
private fun BernVideoSurface(playerID: String, window: String, fit: BernVideoFit, visible: Boolean, clipped: Boolean = false) {
    val token = remember { java.util.UUID.randomUUID().toString() }
    val context = LocalContext.current
    val view = remember(clipped) {
        // A rounded frame clips a TextureView only.
        val layout = if (clipped) context.resources.getIdentifier("bern_player_texture", "layout", context.packageName) else 0
        val inflated = if (layout != 0) android.view.LayoutInflater.from(context).inflate(layout, null) as? PlayerView else null
        (inflated ?: PlayerView(context)).apply {
            useController = false
            setShutterBackgroundColor(android.graphics.Color.BLACK)
        }
    }
    DisposableEffect(playerID, window) {
        BernMedia.attach(view, token, playerID, window)
        onDispose { BernMedia.detach(token) }
    }
    AndroidView(
        factory = { view },
        update = {
            it.resizeMode = when (fit) {
                BernVideoFit.Contain -> AspectRatioFrameLayout.RESIZE_MODE_FIT
                BernVideoFit.Cover -> AspectRatioFrameLayout.RESIZE_MODE_ZOOM
                BernVideoFit.Fill -> AspectRatioFrameLayout.RESIZE_MODE_FILL
            }
            it.alpha = if (visible) 1f else 0f
        },
        modifier = Modifier.fillMaxSize(),
    )
}

/// What the video area shows while there is no picture.
@Composable
private fun BernVideoStatus(state: BernPlayerState?) {
    val onVideo = Color.White
    when (state?.phase) {
        BernMediaPhase.Loading -> CircularProgressIndicator(color = onVideo)
        BernMediaPhase.Failed -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(Icons.Filled.Warning, contentDescription = null, tint = onVideo)
            Text(BernText.string("bern.media.failed"), color = onVideo)
            Text(state.error ?: "", color = onVideo.copy(alpha = 0.7f), style = MaterialTheme.typography.bodySmall)
        }
        BernMediaPhase.Unavailable, null -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(Icons.Filled.Info, contentDescription = null, tint = onVideo)
            Text(BernText.string("bern.media.unavailable"), color = onVideo)
        }
        BernMediaPhase.Idle -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(Icons.Filled.PlayArrow, contentDescription = null, tint = onVideo)
            Text(BernText.string("bern.media.empty"), color = onVideo)
        }
        else -> {}
    }
}

/// The standard control bar. It shows the merged state only and turns user
/// input into media commands: dragging previews locally and commits once,
/// and state updates never call back into a command.
@Composable
private fun BernVideoControls(target: String, playerID: String, state: BernPlayerState?) {
    val window = LocalBernWindow.current
    var scrubbing by remember { mutableStateOf<Float?>(null) }
    var volumePreview by remember { mutableStateOf<Float?>(null) }
    val current = { BernMedia.players[playerID] }
    val playing = state?.phase == BernMediaPhase.Playing || state?.phase == BernMediaPhase.Buffering
    val playable = state != null && state.sourceURL != null &&
        state.phase !in listOf(BernMediaPhase.Idle, BernMediaPhase.Failed, BernMediaPhase.Unavailable)
    val seekable = playable && state?.seekable == true && state.durationMs != null
    val controllable = state != null && state.phase != BernMediaPhase.Unavailable
    val togglePlayback = {
        val live = current()
        val command = if (live?.phase == BernMediaPhase.Playing || live?.phase == BernMediaPhase.Buffering) {
            BernMediaCommand.Pause
        } else {
            BernMediaCommand.Play
        }
        BernMedia.command(command, playerID, window.id)
    }
    val toggleMuted = { BernMedia.setMuted(playerID, !(current()?.muted ?: false), window.id) }
    val playLabel = BernText.string(if (playing) "bern.media.pause" else "bern.media.play")
    val muted = state?.muted ?: false
    BernControl("$target.play_pause", "button", playLabel, playable, BernControlActions(activate = togglePlayback))
    BernControl(
        "$target.position", "slider", BernText.string("bern.media.position"), seekable,
        BernControlActions(
            setNumber = { BernMedia.seek(playerID, Math.round(it), window.id) },
            value = { BernText.text((current()?.positionMs ?: 0).toDouble()) },
        ),
    )
    BernControl(
        "$target.mute", "switch", BernText.string("bern.media.mute"), controllable,
        BernControlActions(
            activate = toggleMuted,
            setBool = { BernMedia.setMuted(playerID, it, window.id) },
            value = { if (current()?.muted == true) "true" else "false" },
        ),
    )
    BernControl(
        "$target.volume", "slider", BernText.string("bern.media.volume"), controllable,
        BernControlActions(
            setNumber = { BernMedia.setVolume(playerID, it.coerceIn(0.0, 1.0), window.id) },
            value = { BernText.text(current()?.volume ?: 1.0) },
        ),
    )
    val duration = (state?.durationMs ?: 0).toFloat()
    val position = (state?.positionMs ?: 0).toFloat()
    val neutral = SliderDefaults.colors(
        thumbColor = MaterialTheme.colorScheme.onSurface,
        activeTrackColor = MaterialTheme.colorScheme.onSurface,
    )
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        IconButton(onClick = togglePlayback, enabled = playable) {
            Icon(BernIcons.get(if (playing) "pause.fill" else "play.fill"), contentDescription = playLabel)
        }
        Text(
            bernTime((scrubbing ?: position).toLong()),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Slider(
            value = scrubbing ?: minOf(position, duration),
            onValueChange = { scrubbing = it },
            onValueChangeFinished = {
                scrubbing?.let { BernMedia.seek(playerID, it.toLong(), window.id) }
                scrubbing = null
            },
            valueRange = 0f..maxOf(duration, 1f),
            enabled = seekable,
            colors = neutral,
            modifier = Modifier.weight(1f),
        )
        Text(
            state?.durationMs?.let(::bernTime)
                ?: if (playable) BernText.string("bern.media.live") else "--:--",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        IconButton(onClick = toggleMuted, enabled = controllable) {
            Icon(
                BernIcons.get(if (muted) "speaker.slash.fill" else "speaker.wave.2.fill"),
                contentDescription = BernText.string(if (muted) "bern.media.unmute" else "bern.media.mute"),
            )
        }
        Slider(
            value = volumePreview ?: (state?.volume ?: 1.0).toFloat(),
            onValueChange = { volumePreview = it },
            onValueChangeFinished = {
                volumePreview?.let { BernMedia.setVolume(playerID, it.toDouble(), window.id) }
                volumePreview = null
            },
            enabled = controllable,
            colors = neutral,
            modifier = Modifier.width(72.dp),
        )
    }
}
