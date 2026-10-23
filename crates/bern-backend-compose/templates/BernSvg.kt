// Bundled animated SVG documents; no JavaScript/native command bridge.
package dev.bern.runtime

import android.content.Context
import android.graphics.Color
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import org.json.JSONObject

private const val bernSvgPrefix = /*bern:svg-prefix*/
private const val bernSvgSuffix = /*bern:svg-suffix*/

@Composable
fun BernSvg(source: String, label: String?, width: Dp, height: Dp, fit: String = "contain",
            playing: Boolean = true, speed: Double = 1.0, restart: Double = 0.0, interactive: Boolean = true, data: String = "", externalLinks: Boolean = false) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    var resumed by remember { mutableStateOf(lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) }
    val dark = isSystemInDarkTheme()
    val state = JSONObject().apply {
        put("playing", playing); put("visible", resumed)
        put("speed", if (speed.isFinite() && speed > 0) speed else 1.0)
        put("restart", if (restart.isFinite()) restart else 0.0)
        put("externalLinks", externalLinks); put("data", data); put("interactive", interactive); put("fit", fit); put("scheme", if (dark) "dark" else "light")
    }.toString()
    val currentState by rememberUpdatedState(state)
    val view = remember(source) {
        (BernSvgViews.take(context, source) ?: bernSvgView(context)).apply {
            webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    if (externalLinks && request.hasGesture() && request.url.scheme == "https") {
                        try { context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, request.url)) } catch (_: Exception) {}
                    }
                    return true
                }
                override fun onPageFinished(view: WebView, url: String) {
                    view.evaluateJavascript("window.bernSvg?.update($currentState)", null)
                }
            }
            // Each display loads the document anew, in a kept view too: its
            // scripts and animations start over.
            val svg = context.assets.open("bern/resources/$source").bufferedReader().use { it.readText() }
            loadDataWithBaseURL(null, bernSvgPrefix + svg + bernSvgSuffix, "text/html", "UTF-8", null)
        }
    }
    DisposableEffect(lifecycle, view) {
        val observer = LifecycleEventObserver { _, _ ->
            resumed = lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)
            view.evaluateJavascript("window.bernSvg?.update({visible:$resumed})", null)
            if (resumed) view.onResume() else view.onPause()
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
    AndroidView(factory = { view }, modifier = Modifier.size(width, height), onRelease = {
        // AndroidView calls this after removing the view from the hierarchy.
        BernSvgViews.keep(context, source, it, lifecycle)
    }, update = {
        it.contentDescription = label
        it.importantForAccessibility = if (label == null) android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else android.view.View.IMPORTANT_FOR_ACCESSIBILITY_YES
        it.evaluateJavascript("window.bernSvg?.update($state)", null)
    })
}

private fun bernSvgView(context: Context) = WebView(context).apply {
    // AndroidView's default WRAP_CONTENT makes the page's viewport as tall
    // as its content, so `height: 100%` is 0; the view is as large as its
    // modifier says.
    layoutParams = android.view.ViewGroup.LayoutParams(
        android.view.ViewGroup.LayoutParams.MATCH_PARENT,
        android.view.ViewGroup.LayoutParams.MATCH_PARENT,
    )
    setBackgroundColor(Color.TRANSPARENT)
    settings.javaScriptEnabled = true
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.blockNetworkLoads = true
    settings.domStorageEnabled = false
    settings.javaScriptCanOpenWindowsAutomatically = false
    isVerticalScrollBarEnabled = false
    isHorizontalScrollBarEnabled = false
}

/// Web views of documents that left the screen, at most one per document
/// and activity: creating a web view takes about 50 ms on the main thread,
/// so a document shown again soon after takes the view it had and loads
/// anew. A kept view shows a blank page and keeps the web renderer process
/// alive; it is released 30 seconds after it left, when the activity
/// stops, or when its renderer process ends (which would otherwise end the
/// application with it).
private object BernSvgViews {
    private class Kept(val view: WebView, val release: Runnable)

    private val idle = HashMap<Context, HashMap<String, Kept>>()
    private val main = android.os.Handler(android.os.Looper.getMainLooper())

    fun take(context: Context, source: String): WebView? {
        val kept = idle[context]?.remove(source) ?: return null
        main.removeCallbacks(kept.release)
        return kept.view
    }

    fun keep(context: Context, source: String, view: WebView, lifecycle: Lifecycle) {
        view.stopLoading()
        if (!lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
            view.destroy()
            return
        }
        val views = idle.getOrPut(context) {
            val views = HashMap<String, Kept>()
            lifecycle.addObserver(object : LifecycleEventObserver {
                override fun onStateChanged(owner: androidx.lifecycle.LifecycleOwner, event: Lifecycle.Event) {
                    if (event != Lifecycle.Event.ON_STOP && event != Lifecycle.Event.ON_DESTROY) return
                    lifecycle.removeObserver(this)
                    if (idle[context] === views) idle.remove(context)
                    views.values.forEach { main.removeCallbacks(it.release); it.view.destroy() }
                    views.clear()
                }
            })
            views
        }
        lateinit var kept: Kept
        // Only while it is still kept: taking it back cancels this.
        val release = Runnable {
            if (views[source] !== kept) return@Runnable
            views.remove(source)
            view.destroy()
        }
        kept = Kept(view, release)
        view.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) = true
            override fun onRenderProcessGone(view: WebView, detail: android.webkit.RenderProcessGoneDetail): Boolean {
                main.removeCallbacks(release)
                release.run()
                return true
            }
        }
        // A blank page stops the document's scripts and animations.
        view.loadUrl("about:blank")
        views.put(source, kept)?.let { main.removeCallbacks(it.release); it.view.destroy() }
        main.postDelayed(release, 30_000)
    }
}
