import android.widget.CalendarView
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import java.util.Calendar
import java.util.Locale

private fun isoToMillis(iso: String): Long? {
    val parts = iso.split("-")
    if (parts.size != 3) return null
    val y = parts[0].toIntOrNull() ?: return null
    val m = parts[1].toIntOrNull() ?: return null
    val d = parts[2].toIntOrNull() ?: return null
    val cal = Calendar.getInstance()
    cal.set(y, m - 1, d, 12, 0, 0)
    cal.set(Calendar.MILLISECOND, 0)
    return cal.timeInMillis
}

@Composable
fun NativeContent(props: Props, actions: Actions, theme: Theme) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        border = BorderStroke(1.dp, theme.border),
        color = theme.surface,
    ) {
        Column(
            modifier = Modifier.padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(
                    text = props.heading,
                    style = MaterialTheme.typography.titleMedium,
                    color = theme.textPrimary,
                )
                Spacer(Modifier.weight(1f))
                Text(
                    text = props.date,
                    style = MaterialTheme.typography.bodyMedium,
                    color = theme.textSecondary,
                )
                FilledTonalButton(onClick = actions.today) {
                    Text(props.todayLabel)
                }
            }

            AndroidView(
                modifier = Modifier.fillMaxWidth(),
                factory = { context ->
                    CalendarView(context).apply {
                        setOnDateChangeListener { _, year, month, dayOfMonth ->
                            val iso = String.format(Locale.US, "%04d-%02d-%02d", year, month + 1, dayOfMonth)
                            actions.setDate(iso)
                        }
                    }
                },
                update = { view ->
                    isoToMillis(props.date)?.let { target ->
                        if (kotlin.math.abs(view.date - target) > 43_200_000L) {
                            view.setDate(target, false, true)
                        }
                    }
                },
            )
        }
    }
}
