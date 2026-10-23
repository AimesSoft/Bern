
// Process-scoped, like BernHost. Only the private router has a native bridge.
@Suppress("SetJavaScriptEnabled")
private object BernScriptHost {
    private var view: android.webkit.WebView? = null
    private val handler = Handler(Looper.getMainLooper())
    private var ready = false
    private var appContext: Context? = null
    private val pending = mutableListOf<String>()
    @Keep
    class Bridge {
        @android.webkit.JavascriptInterface
        fun complete(json: String) { BernNative.scriptComplete(json.toByteArray(Charsets.UTF_8)) }
    }
    private val pump = object : Runnable {
        override fun run() {
            val context = appContext ?: return
            val batch = String(BernNative.scriptPoll(android.os.SystemClock.uptimeMillis().toDouble()), Charsets.UTF_8)
            if (batch.isNotEmpty()) pending.add(batch)
            if (view == null && pending.isNotEmpty()) open(context)
            if (ready) {
                for (queued in pending) view?.evaluateJavascript("bernScriptDispatch($queued)", null)
                pending.clear()
            }
            handler.postDelayed(this, 16)
        }
    }
    fun start(context: Context) {
        if (appContext != null) return
        appContext = context.applicationContext
        BernNative.scriptStart()
        handler.post(pump)
    }
    private fun open(context: Context) {
        val web = android.webkit.WebView(context.applicationContext)
        view = web
        web.settings.javaScriptEnabled = true
        web.settings.allowFileAccess = false
        web.settings.allowContentAccess = false
        web.addJavascriptInterface(Bridge(), "BernScriptBridge")
        web.webViewClient = object : android.webkit.WebViewClient() {
            override fun onPageFinished(view: android.webkit.WebView, url: String) { ready = true }
            override fun onRenderProcessGone(view: android.webkit.WebView, detail: android.webkit.RenderProcessGoneDetail): Boolean { stop(); return true }
        }
        web.loadDataWithBaseURL("https://bern.invalid/", /*bern:script-html*/, "text/html", "UTF-8", null)
    }
    fun stop() {
        handler.removeCallbacks(pump)
        view?.removeJavascriptInterface("BernScriptBridge")
        view?.destroy(); view = null; ready = false; appContext = null; pending.clear()
        BernNative.scriptStop()
    }
}
