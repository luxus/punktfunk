package io.unom.punktfunk

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure JVM test of the safe-area stream geometry ([SafeArea]) and the sentinel that selects it —
 * the width-only inset that keeps the picture clear of the cutout.
 * Run: `./gradlew -PexcludeScreenshots :app:testDebugUnitTest`.
 */
class SafeAreaTest {
    @Test
    fun aHoleOnOneSideIsPaidForOnce() {
        // OnePlus 9 Pro, from the reporter's `dumpsys display` (#1068): one punch-hole, 95 px tall
        // on the 1080-wide portrait panel, which on the 1440 × 3216 physical grid is 127 px on ONE
        // landscape side. 3216 − 127 is odd, so the even neighbour is what the host will take.
        assertEquals(3088, SafeArea.insetWidth(3216, 127, 0))
        // …and the picture starts at the hole, not half-way into it.
        assertEquals(127, SafeArea.offsetX(3216, 127, 0))
        // What the symmetric inset cost: 127 px of glass nothing covers.
        assertEquals(2962, SafeArea.insetWidth(3216, 127, 127))
    }

    @Test
    fun cornersAreOptInAndATypedInsetWins() {
        val auto = Settings()
        // Corner radius 28 is reported but not charged: the hole alone decides.
        assertEquals(SafeInsets(127, 0, 28), SafeArea.resolve(127, 0, 28, auto))
        // …until the opt-in, which lifts the side that has nothing else on it.
        assertEquals(
            SafeInsets(127, 28, 28),
            SafeArea.resolve(127, 0, 28, auto.copy(safeAreaClearCorners = true)),
        )
        // A typed override replaces its own side and leaves the other alone.
        assertEquals(
            SafeInsets(0, 0, 28),
            SafeArea.resolve(127, 0, 28, auto.copy(safeAreaLeftPx = 0)),
        )
        assertEquals(
            SafeInsets(127, 40, 28),
            SafeArea.resolve(127, 0, 28, auto.copy(safeAreaRightPx = 40)),
        )
        // …and outranks the corner opt-in, which is the point of an escape hatch.
        assertEquals(
            SafeInsets(10, 28, 28),
            SafeArea.resolve(127, 0, 28, auto.copy(safeAreaClearCorners = true, safeAreaLeftPx = 10)),
        )
    }

    @Test
    fun insetsEachSideAndStaysHostValid() {
        // A punch-hole phone: 2400 px wide, 96 px of unsafe edge per side → 2208.
        assertEquals(2400 - 96 * 2, SafeArea.insetWidth(2400, 96, 96))
        // Odd results even-floor — the host rejects odd dimensions outright, and an inset
        // subtraction lands odd about half the time.
        assertEquals(0, SafeArea.insetWidth(2401, 95, 0) % 2)
        // No cutout and square corners → the native width, unchanged, centred.
        assertEquals(2400, SafeArea.insetWidth(2400, 0, 0))
        assertEquals(0, SafeArea.offsetX(2400, 0, 0))
    }

    @Test
    fun absurdInsetsCannotDriveTheModeUnderTheHostFloor() {
        assertEquals(SafeArea.MIN_WIDTH, SafeArea.insetWidth(1280, 5000, 5000))
        // A negative reading is treated as no inset rather than widening past the panel.
        assertEquals(1280, SafeArea.insetWidth(1280, -40, -40))
        // The floor can make the picture wider than the room between the insets. It is pulled
        // back to the panel's edge rather than hung off it — the one invariant that matters.
        assertEquals(960, SafeArea.offsetX(1280, 5000, 5000))
        for ((w, l, r) in listOf(Triple(1280, 5000, 5000), Triple(400, 300, 0), Triple(3216, 127, 0))) {
            assertTrue(SafeArea.offsetX(w, l, r) + SafeArea.insetWidth(w, l, r) <= w)
        }
    }

    @Test
    fun safeModeIsNarrowerThanNativeWheneverThereIsAnInset() {
        val native = 2556
        assertTrue(SafeArea.insetWidth(native, 60, 0) < native)
    }

    @Test
    fun theSentinelIsAPresetAndNeverReadsAsCustom() {
        // The safe-area mode is a stored preset, not a typed size: `isCustomResolution` must be
        // false for it, or the touch settings would open the custom width/height fields on it and
        // the gamepad screen would prepend a bogus "Custom · -2 × -2" row.
        val s = Settings(width = SAFE_AREA_MODE, height = SAFE_AREA_MODE)
        assertTrue(!s.isCustomResolution())
        // And it must be distinct from the UI's own "Custom…" sentinel (-1).
        assertTrue(SAFE_AREA_MODE != -1)
        assertTrue(NATIVE_RESOLUTION_OPTIONS.any { it.first == SAFE_AREA_MODE && it.second == SAFE_AREA_MODE })
    }
}
