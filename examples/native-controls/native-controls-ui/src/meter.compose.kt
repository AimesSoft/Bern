import android.widget.RatingBar
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

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
            ) {
                Text(
                    text = props.heading,
                    style = MaterialTheme.typography.titleMedium,
                    color = theme.textPrimary,
                )
                Spacer(Modifier.weight(1f))
                Text(
                    text = props.readout,
                    style = MaterialTheme.typography.bodyMedium,
                    color = theme.textSecondary,
                )
            }

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                val fraction = ((props.level - props.levelMin) / (props.levelMax - props.levelMin))
                    .toFloat()
                    .coerceIn(0f, 1f)
                CircularProgressIndicator(
                    progress = { fraction },
                    modifier = Modifier.size(40.dp),
                    color = theme.accent,
                )

                // Classic platform android.widget.RatingBar embedded directly via AndroidView.
                AndroidView(
                    factory = { context ->
                        RatingBar(context, null, android.R.attr.ratingBarStyle).apply {
                            numStars = 5
                            stepSize = 0.5f
                            setOnRatingBarChangeListener { _, rating, fromUser ->
                                if (fromUser) {
                                    actions.setLevel((rating * 2.0).coerceIn(props.levelMin, props.levelMax))
                                }
                            }
                        }
                    },
                    update = { bar ->
                        val stars = (props.level / 2.0).toFloat().coerceIn(0f, 5f)
                        if (kotlin.math.abs(bar.rating - stars) > 0.01f) {
                            bar.rating = stars
                        }
                    },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                FilterChip(
                    selected = props.boosted,
                    onClick = { actions.setBoosted(!props.boosted) },
                    label = { Text(props.boostedLabel) },
                )
                Spacer(Modifier.weight(1f))
                FilledTonalButton(onClick = actions.reset) {
                    Text(props.resetLabel)
                }
            }
        }
    }
}
