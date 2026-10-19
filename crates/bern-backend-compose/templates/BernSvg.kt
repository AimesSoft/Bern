// Bundled animated SVG documents; no JavaScript/native command bridge.
package dev.bern.runtime

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
            playing: Boolean = true, speed: Double = 1.0, restart: Double = 0.0, interactive: Boolean = true) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    var resumed by remember { mutableStateOf(lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) }
    val dark = isSystemInDarkTheme()
    val state = JSONObject().apply {
        put("playing", playing); put("visible", resumed)
        put("speed", if (speed.isFinite() && speed > 0) speed else 1.0)
        put("restart", if (restart.isFinite()) restart else 0.0)
        put("interactive", interactive); put("fit", fit); put("scheme", if (dark) "dark" else "light")
    }.toString()
    val currentState by rememberUpdatedState(state)
    val view = remember(source) {
        WebView(context).apply {
            // AndroidView's default WRAP_CONTENT makes the page's viewport
            // as tall as its content, so `height: 100%` is 0; the view is
            // as large as its modifier says.
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
            webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) = true
                override fun onPageFinished(view: WebView, url: String) {
                    view.evaluateJavascript("window.bernSvg?.update($currentState)", null)
                }
            }
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
        it.stopLoading()
        it.loadUrl("about:blank")
        it.destroy()
    }, update = {
        it.contentDescription = label
        it.importantForAccessibility = if (label == null) android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else android.view.View.IMPORTANT_FOR_ACCESSIBILITY_YES
        it.evaluateJavascript("window.bernSvg?.update($state)", null)
    })
}
