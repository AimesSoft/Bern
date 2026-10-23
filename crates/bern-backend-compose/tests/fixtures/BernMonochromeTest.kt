package dev.bern.runtime

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class BernMonochromeTest {
    @Test
    fun allMaterialRolesAreNeutralAndContentRemainsReadable() {
        for (dark in listOf(false, true)) {
            val light = Color(0xFF333333)
            val accent = if (dark) Color(0xFFD6D6D6) else light
            val scheme = bernAccentScheme(accent, light, dark)
            val roles = with(scheme) { listOf(
                primary, onPrimary, primaryContainer, onPrimaryContainer, inversePrimary,
                secondary, onSecondary, secondaryContainer, onSecondaryContainer,
                tertiary, onTertiary, tertiaryContainer, onTertiaryContainer,
                background, onBackground, surface, onSurface, surfaceVariant, onSurfaceVariant,
                surfaceTint, inverseSurface, inverseOnSurface, error, onError, errorContainer,
                onErrorContainer, outline, outlineVariant, scrim, surfaceBright, surfaceDim,
                surfaceContainer, surfaceContainerHigh, surfaceContainerHighest,
                surfaceContainerLow, surfaceContainerLowest,
                primaryFixed, primaryFixedDim, onPrimaryFixed, onPrimaryFixedVariant,
                secondaryFixed, secondaryFixedDim, onSecondaryFixed, onSecondaryFixedVariant,
                tertiaryFixed, tertiaryFixedDim, onTertiaryFixed, onTertiaryFixedVariant,
            ) }
            assertEquals("Every Material role is covered", 48, roles.size)
            roles.forEachIndexed { index, color ->
                assertEquals("R=G, dark=$dark role=$index", color.red, color.green, 0.000001f)
                assertEquals("G=B, dark=$dark role=$index", color.green, color.blue, 0.000001f)
            }
            assertEquals(accent, scheme.primary)
            for ((foreground, background) in listOf(
                scheme.onPrimary to scheme.primary,
                scheme.onBackground to scheme.background,
                scheme.onSecondaryContainer to scheme.secondaryContainer,
                scheme.onSurfaceVariant to scheme.surface,
            )) {
                val high = maxOf(foreground.luminance(), background.luminance())
                val low = minOf(foreground.luminance(), background.luminance())
                assertTrue("Normal text contrast, dark=$dark", (high + 0.05f) / (low + 0.05f) >= 4.5f)
            }
        }
    }
}
