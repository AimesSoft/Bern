// Wear player controls; media state, decoding and execution are shared.
package dev.bern.runtime

import androidx.compose.foundation.layout.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.wear.compose.material3.*

@Composable
internal fun BernVideoStatus(state: BernPlayerState?) {
    when (state?.phase) {
        BernMediaPhase.Loading -> CircularProgressIndicator(modifier = Modifier.size(24.dp))
        BernMediaPhase.Failed -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(BernIcons.get("exclamationmark.triangle"), null, tint = Color.White)
            Text(BernText.string("bern.media.failed"), color = Color.White)
            Text(state.error ?: "", color = Color.White, style = MaterialTheme.typography.bodySmall)
        }
        BernMediaPhase.Unavailable, null -> Text(BernText.string("bern.media.unavailable"), color = Color.White)
        BernMediaPhase.Idle -> Text(BernText.string("bern.media.empty"), color = Color.White)
        else -> {}
    }
}

@Composable
internal fun BernVideoControls(target: String, playerID: String, state: BernPlayerState?) {
    val window = LocalBernWindow.current
    val current = { BernMedia.players[playerID] }
    val playing = state?.phase in listOf(BernMediaPhase.Playing, BernMediaPhase.Buffering)
    val playable = state?.sourceURL != null && state.phase !in listOf(BernMediaPhase.Idle, BernMediaPhase.Failed, BernMediaPhase.Unavailable)
    val seekable = playable && state?.seekable == true && (state.durationMs ?: 0) > 0
    val controllable = state != null && state.phase != BernMediaPhase.Unavailable
    val toggle = {
        BernMedia.command(if (current()?.phase in listOf(BernMediaPhase.Playing, BernMediaPhase.Buffering)) BernMediaCommand.Pause else BernMediaCommand.Play,
            playerID, window.id)
    }
    val mute = { BernMedia.setMuted(playerID, !(current()?.muted ?: false), window.id) }
    val seek: (Double) -> Unit = { BernMedia.seek(playerID, it.toLong(), window.id) }
    val volume: (Double) -> Unit = { BernMedia.setVolume(playerID, it.coerceIn(0.0, 1.0), window.id) }
    val playLabel = BernText.string(if (playing) "bern.media.pause" else "bern.media.play")
    BernControl("$target.play_pause", "button", playLabel, playable, BernControlActions(activate = toggle))
    BernControl("$target.position", "slider", BernText.string("bern.media.position"), seekable,
        BernControlActions(setNumber = seek, value = { BernText.text((current()?.positionMs ?: 0).toDouble()) }))
    BernControl("$target.mute", "switch", BernText.string("bern.media.mute"), controllable,
        BernControlActions(activate = mute, setBool = { BernMedia.setMuted(playerID, it, window.id) }, value = { (current()?.muted ?: false).toString() }))
    BernControl("$target.volume", "slider", BernText.string("bern.media.volume"), controllable,
        BernControlActions(setNumber = volume, value = { BernText.text(current()?.volume ?: 1.0) }))
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Row {
            BernIconButton(playLabel, BernIcons.get(if (playing) "pause.fill" else "play.fill"), playable, onClick = toggle)
            BernIconButton(BernText.string(if (state?.muted == true) "bern.media.unmute" else "bern.media.mute"),
                BernIcons.get(if (state?.muted == true) "speaker.slash.fill" else "speaker.wave.2.fill"), controllable, onClick = mute)
        }
        Text("${bernTime(state?.positionMs ?: 0)} / ${state?.durationMs?.let(::bernTime) ?: BernText.string("bern.media.live")}",
            style = MaterialTheme.typography.labelSmall)
        if (seekable) BernSlider(BernText.string("bern.media.position"), (state?.positionMs ?: 0).toDouble(), seek,
            0.0, (state?.durationMs ?: 1).toDouble(), null, null, null, null, labelHidden = true)
        Slider(value = (state?.volume ?: 1.0).toFloat(), onValueChange = { volume(it.toDouble()) }, steps = 9,
            valueRange = 0f..1f, enabled = controllable)
    }
}
