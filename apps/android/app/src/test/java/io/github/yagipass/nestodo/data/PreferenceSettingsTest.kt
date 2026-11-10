package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.MemoryPreferences
import io.github.yagipass.nestodo.preferenceStore
import kotlinx.coroutines.test.runTest
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class PreferenceSettingsTest {
    @get:Rule val folder = TemporaryFolder()

    @Test
    fun `a fresh install shows completed rows, so no todo looks missing before the member chooses to hide them`() =
        runTest {
            val settings = PreferenceSettings(preferenceStore(folder))

            assertTrue(settings.showCompleted())
        }

    @Test
    fun `the server is kept until it is changed, so each launch connects to it without asking again`() =
        runTest {
            val settings = PreferenceSettings(preferenceStore(folder))
            assertNull(settings.baseUrl())

            settings.setBaseUrl("http://10.0.2.2:5173/".toHttpUrl())
            assertEquals("http://10.0.2.2:5173/".toHttpUrl(), settings.baseUrl())

            settings.setBaseUrl(null)
            assertNull(settings.baseUrl())
        }

    @Test
    fun `settings saved before the app moved to DataStore are kept, so updating the app neither shows hidden completed rows nor forgets where the last share went`() =
        runTest {
            val legacy = MemoryPreferences("show_completed" to false, "share_parent" to 4L)
            val settings = PreferenceSettings(preferenceStore(folder, legacy))

            assertFalse(settings.showCompleted())
            assertEquals(4L, settings.shareParent())
        }

    @Test
    fun `sharing at the top level after sharing under a todo forgets the todo, so the next share starts from the top level`() =
        runTest {
            val settings = PreferenceSettings(preferenceStore(folder))
            settings.setShareParent(4)

            settings.setShareParent(null)

            assertNull(settings.shareParent())
        }
}
