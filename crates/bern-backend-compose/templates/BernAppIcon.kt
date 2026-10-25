// Bern Compose runtime: the application icon `WindowSet(icon:)` chooses.
// Generated build input; do not edit.
//
// Each icon is an activity alias of the main activity with the launcher
// intent; the chosen alias is enabled and the others disabled, so the
// launcher shows that alias's icon. Disabling the alias the task started
// from closes the app, so the change waits until the app leaves the screen.
// Launchers take a moment to redraw.

package dev.bern.runtime

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

/// Enable the alias `chosen` of `aliases` (fully qualified class names) and
/// disable the rest once the app stops showing; `aliases.first()` is the one
/// the manifest enables.
@Composable
fun BernIconChoice(chosen: String, aliases: List<String>) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val latest by rememberUpdatedState(chosen)
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP) bernApplyIcon(context, latest, aliases)
        }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }
}

private fun bernApplyIcon(context: Context, chosen: String, aliases: List<String>) {
    val packages = context.packageManager
    for (alias in aliases) {
        val component = ComponentName(context.packageName, alias)
        val enabled = when (packages.getComponentEnabledSetting(component)) {
            PackageManager.COMPONENT_ENABLED_STATE_ENABLED -> true
            PackageManager.COMPONENT_ENABLED_STATE_DEFAULT -> alias == aliases.first()
            else -> false
        }
        if (enabled != (alias == chosen)) {
            packages.setComponentEnabledSetting(
                component,
                if (alias == chosen) PackageManager.COMPONENT_ENABLED_STATE_ENABLED
                else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                PackageManager.DONT_KILL_APP,
            )
        }
    }
}
