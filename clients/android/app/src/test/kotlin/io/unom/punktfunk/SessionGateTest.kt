package io.unom.punktfunk

import org.junit.After
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure JVM test of [SessionGate] — the guard that makes one launch produce one session (#1068).
 * Run: `./gradlew -PexcludeScreenshots :app:testDebugUnitTest`.
 */
class SessionGateTest {
    @After
    fun reset() {
        SessionGate.live = null
        SessionGate.release()
    }

    @Test
    fun oneDialAtATime() {
        assertTrue(SessionGate.take())
        // The console's event pump raising a second Launch while the first is still on the host.
        assertFalse(SessionGate.take())
        SessionGate.release()
        assertTrue(SessionGate.take())
    }

    @Test
    fun noDialBehindALiveSession() {
        SessionGate.live = SessionGate.Live("host-1")
        assertFalse(SessionGate.take())
        // Another host is refused too: this app shows one stream, so a second dial replaces the
        // session on screen and strands the first handle on the host.
        assertFalse(SessionGate.take())
        SessionGate.live = null
        assertTrue(SessionGate.take())
    }

    @Test
    fun aRefusedDialNeverTakesTheGate() {
        SessionGate.live = SessionGate.Live(null)
        assertFalse(SessionGate.take())
        SessionGate.live = null
        // A refusal that had claimed the flag would wedge every later dial.
        assertTrue(SessionGate.take())
    }
}
