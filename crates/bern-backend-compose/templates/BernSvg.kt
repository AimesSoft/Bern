// Native pure-Rust animated SVG presenter (`bern-svg` over JNI + DirectByteBuffer).
// Zero `android.webkit.WebView` dependency; runs on both Android and Wear OS.
package dev.bern.runtime

import android.graphics.Bitmap
import android.provider.Settings
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntSize
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.nio.ByteBuffer

private class BernSvgHolder {
    var handle: Long = 0L
    var loadedSource: String = ""
    var buffer: ByteBuffer? = null
    var bitmap: Bitmap? = null
    var bufW: Int = 0
    var bufH: Int = 0

    fun close() {
        if (handle != 0L) {
            BernNative.svgFree(handle)
            handle = 0L
        }
        loadedSource = ""
    }
}

@Composable
fun BernSvg(
    source: String,
    label: String?,
    width: Dp,
    height: Dp,
    fit: String = "contain",
    playing: Boolean = true,
    speed: Double = 1.0,
    restart: Double = 0.0,
    interactive: Boolean = true,
    data: String = "",
    externalLinks: Boolean = false,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    var resumed by remember { mutableStateOf(lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) }
    val dark = bernDark()
    val reducedMotion = remember(context) {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }

    val holder = remember { BernSvgHolder() }
    var active by remember { mutableStateOf(true) }
    var frameTick by remember { mutableLongStateOf(0L) }

    val wPx = with(density) { width.roundToPx().coerceAtLeast(1) }
    val hPx = with(density) { height.roundToPx().coerceAtLeast(1) }

    DisposableEffect(source) {
        if (holder.loadedSource != source) {
            holder.close()
            val keyBytes = source.encodeToByteArray()
            val bytes = if (BernNative.svgCached(keyBytes) != 0) {
                ByteArray(0)
            } else {
                runCatching {
                    context.assets.open("bern/resources/$source").use { it.readBytes() }
                }.getOrElse { ByteArray(0) }
            }
            holder.handle = BernNative.svgOpen(keyBytes, bytes)
            holder.loadedSource = source
            active = true
            frameTick++
        }
        onDispose { holder.close() }
    }

    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, _ ->
            resumed = lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }

    val fitCode = when (fit) {
        "cover" -> 1
        "fill" -> 2
        else -> 0
    }
    val textRgba = if (dark) 0xFFFFFFFF.toInt() else 0x000000FF

    fun renderAt(nowSecs: Double) {
        val h = holder.handle
        if (h == 0L) return
        BernNative.svgConfigure(
            h,
            if (playing && resumed) 1 else 0,
            if (speed.isFinite() && speed > 0.0) speed else 1.0,
            if (restart.isFinite()) restart else 0.0,
            fitCode,
            if (interactive) 1 else 0,
            if (dark) 1 else 0,
            if (reducedMotion) 1 else 0,
            textRgba,
        )
        val needed = wPx * hPx * 4
        var buf = holder.buffer
        var bmp = holder.bitmap
        if (buf == null || bmp == null || holder.bufW != wPx || holder.bufH != hPx) {
            buf = ByteBuffer.allocateDirect(needed)
            bmp = Bitmap.createBitmap(wPx, hPx, Bitmap.Config.ARGB_8888)
            holder.buffer = buf
            holder.bitmap = bmp
            holder.bufW = wPx
            holder.bufH = hPx
        }
        val status = BernNative.svgRender(h, nowSecs, wPx, hPx, buf)
        if ((status and 1) != 0) {
            buf.rewind()
            bmp.copyPixelsFromBuffer(buf)
            frameTick++
        }
        active = (status and 2) != 0
    }

    LaunchedEffect(source, wPx, hPx, fit, playing, speed, restart, interactive, dark, reducedMotion, resumed) {
        renderAt(System.nanoTime() * 1e-9)
    }

    LaunchedEffect(active, playing, resumed, holder.handle) {
        if (!active || !playing || !resumed || holder.handle == 0L) return@LaunchedEffect
        while (active && playing && resumed && holder.handle != 0L) {
            withFrameNanos { nanos ->
                renderAt(nanos * 1e-9)
            }
        }
    }

    var mod: Modifier = Modifier.size(width, height)
    if (interactive) {
        mod = mod.pointerInput(wPx, hPx) {
            detectTapGestures(
                onDoubleTap = { pt ->
                    if (holder.handle != 0L) {
                        val st = BernNative.svgPointer(
                            holder.handle, 1,
                            (pt.x / size.width.coerceAtLeast(1)).toDouble(),
                            (pt.y / size.height.coerceAtLeast(1)).toDouble(),
                        )
                        if ((st and 2) != 0) active = true
                        if ((st and 1) != 0) renderAt(System.nanoTime() * 1e-9)
                    }
                },
                onTap = { pt ->
                    if (holder.handle != 0L) {
                        val st = BernNative.svgPointer(
                            holder.handle, 0,
                            (pt.x / size.width.coerceAtLeast(1)).toDouble(),
                            (pt.y / size.height.coerceAtLeast(1)).toDouble(),
                        )
                        if ((st and 2) != 0) active = true
                        if ((st and 1) != 0) renderAt(System.nanoTime() * 1e-9)
                    }
                },
            )
        }
    }
    mod = if (label != null) {
        mod.clearAndSetSemantics {
            contentDescription = label
            role = Role.Image
        }
    } else {
        mod.clearAndSetSemantics {}
    }

    Canvas(modifier = mod) {
        val tick = frameTick
        val bmp = holder.bitmap
        if (tick >= 0L && bmp != null) {
            drawImage(
                image = bmp.asImageBitmap(),
                dstSize = IntSize(size.width.toInt().coerceAtLeast(1), size.height.toInt().coerceAtLeast(1)),
            )
        }
    }
}
