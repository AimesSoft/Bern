// `WebPage`: embedded browser view, included only when the application
// declares `WebPage`.
package dev.bern.runtime

import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import org.json.JSONObject

/// `WebPage`: a page of the web, signed in with the cookies of a file. The
/// cookies go with it: the system keeps one cookie store for the whole app.
@Composable
fun BernWebPage(url: String, cookiesFile: String, cookieDomain: String, userAgent: String, closeOn: String, label: String?, close: () -> Unit) {
    val context = LocalContext.current
    val currentClose by rememberUpdatedState(close)
    val listener = "if(!window.__bernClose){window.__bernClose=1;document.addEventListener('click',function(e){var t=e.target;" +
        "if(t&&t.closest&&t.closest(${JSONObject.quote(closeOn)})){e.stopPropagation();e.preventDefault();location.href='bern-web-page://close'}},true)}"
    val view = remember {
        WebView(context).apply {
            layoutParams = android.view.ViewGroup.LayoutParams(
                android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                android.view.ViewGroup.LayoutParams.MATCH_PARENT,
            )
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.mediaPlaybackRequiresUserGesture = false
            if (userAgent.isNotEmpty()) settings.userAgentString = userAgent
            webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val scheme = request.url.scheme
                    if (scheme == "bern-web-page") currentClose()
                    return scheme != "http" && scheme != "https"
                }
                override fun onPageFinished(view: WebView, url: String) {
                    if (closeOn.isNotEmpty()) view.evaluateJavascript(listener, null)
                }
            }
        }
    }
    LaunchedEffect(url) {
        val cookies = CookieManager.getInstance()
        cookies.setAcceptCookie(true)
        cookies.setAcceptThirdPartyCookies(view, true)
        if (cookiesFile.isNotEmpty() && cookieDomain.isNotEmpty()) {
            val attributes = setOf("path", "domain", "expires", "max-age", "secure", "httponly", "samesite")
            val text = runCatching { java.io.File(cookiesFile).readText() }.getOrDefault("")
            for (part in text.split(";")) {
                val pair = part.trim().split("=", limit = 2)
                if (pair.size != 2 || pair[0].lowercase() in attributes) continue
                cookies.setCookie(url, "${pair[0]}=${pair[1]}; Domain=$cookieDomain; Path=/; Secure")
            }
            cookies.flush()
        }
        if (url.isNotEmpty()) view.loadUrl(url)
    }
    DisposableEffect(view) {
        onDispose {
            CookieManager.getInstance().removeAllCookies(null)
            view.destroy()
        }
    }
    AndroidView(factory = { view }, modifier = Modifier.fillMaxSize(), update = {
        it.contentDescription = label
    })
}
