// How pictures are asked for: what one try at an address gave, and when to
// try again. Plain Kotlin, so that a test runs it on a computer against a
// server that fails on purpose; decoding is in `BernPictures`.
package dev.bern.runtime

import java.io.IOException
import java.net.HttpURLConnection
import java.net.MalformedURLException
import java.net.ProtocolException
import java.net.URL
import java.net.UnknownServiceException
import java.security.cert.CertificateException
import java.util.concurrent.ConcurrentHashMap
import javax.net.ssl.SSLPeerUnverifiedException
import kotlin.random.Random
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withTimeoutOrNull

/// What one try at an address gave.
sealed interface BernTry<out T> {
    class Came<T>(val value: T) : BernTry<T>
    sealed interface Failed : BernTry<Nothing>
    /// The network failed, or the server could not answer now.
    data object Unreachable : Failed
    /// The server answered, and not with a picture.
    data object Refused : Failed
    /// No such file, or no address at all. Looking again costs nothing, so
    /// nothing is remembered.
    data object Absent : Failed
}

/// An address the network could not reach, or that stays silent, is tried
/// again, each wait longer than the one before, for as long as something
/// waits for its picture. After the last wait it rests until the network
/// changes or the app comes back to the front; a view that appears again
/// starts over at once. An address the server answered without a picture is
/// not asked again on the same network.
object BernPictureTries {
    /// Seconds before each further try, give or take a quarter, so that
    /// pictures that failed together do not all return together.
    private val waits = listOf(1.0, 2.0, 4.0, 8.0, 16.0, 32.0)
    /// Milliseconds a try may hear nothing before it counts as failed. A
    /// stalled connection reports no error, and would wait for ever.
    private const val silence = 15_000
    /// Addresses the server refused on this network.
    private val refused = ConcurrentHashMap.newKeySet<String>()
    /// Moves on to wake every load waiting for its next try.
    private val round = MutableStateFlow(0L)
    /// Whether the network was there when last heard of; null before that.
    @Volatile private var online: Boolean? = null

    /// What `attempt` gives for `url` once it gives something, trying again
    /// as long as the caller waits; null when there is no such file. A
    /// view's effect ends with the view, and a view that appears again
    /// starts over.
    suspend fun <T : Any> until(url: String, attempt: suspend () -> BernTry<T>): T? {
        var tries = 0
        while (true) {
            currentCoroutineContext().ensureActive()
            if (url in refused) {
                pause(null)
                continue
            }
            when (val result = attempt()) {
                is BernTry.Came -> return result.value
                BernTry.Unreachable -> {
                    val wait = waits.getOrNull(tries)
                    pause(wait?.let { it * Random.nextDouble(0.75, 1.25) })
                    tries = if (wait == null) 0 else tries + 1
                }
                BernTry.Refused -> refused.add(url)
                BernTry.Absent -> return null
            }
        }
    }

    /// One try at the bytes of an http(s) address, on the caller's thread.
    /// A connection that timed out is closed, so the next try has another.
    fun download(url: String): BernTry<ByteArray> {
        var connection: HttpURLConnection? = null
        return try {
            val opened = URL(url).openConnection() as HttpURLConnection
            connection = opened
            opened.connectTimeout = silence
            opened.readTimeout = silence
            val status = opened.responseCode
            when {
                status in 200..299 -> BernTry.Came(opened.inputStream.use { it.readBytes() })
                status < 100 || status == 408 || status == 429 || status >= 500 -> BernTry.Unreachable
                else -> BernTry.Refused
            }
        } catch (error: IOException) {
            if (lasting(error)) BernTry.Refused else BernTry.Unreachable
        } catch (_: IllegalArgumentException) {
            BernTry.Refused
        } finally {
            connection?.disconnect()
        }
    }

    /// Errors another try would only repeat. Any other is worth one.
    private fun lasting(error: IOException): Boolean =
        error is MalformedURLException || error is UnknownServiceException || error is ProtocolException ||
            error is SSLPeerUnverifiedException || error.cause is CertificateException

    /// Waits `seconds`, or with null for as long as it takes, until the
    /// network changes, the app comes to the front or the caller stops.
    private suspend fun pause(seconds: Double?) {
        val seen = round.value
        withTimeoutOrNull(seconds?.let { (it * 1000).toLong() } ?: Long.MAX_VALUE) {
            round.first { it != seen }
        }
    }

    /// Every load waiting for its next try goes now.
    fun tryAgain() {
        round.update { it + 1 }
    }

    /// The system's word on the network: once it is there after a change, a
    /// new way out may answer what the last one could not.
    fun networkChanged(online: Boolean) {
        val first = this.online == null
        this.online = online
        if (!online || first) return
        refused.clear()
        tryAgain()
    }
}
