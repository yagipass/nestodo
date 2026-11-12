package io.github.yagipass.nestodo.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.emptyPreferences
import io.github.yagipass.nestodo.MemoryPreferences
import io.github.yagipass.nestodo.preferenceStore
import java.io.IOException
import kotlinx.coroutines.flow.flowOf
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
    fun `a server that can't be written is reported, unlike other settings, so the app never acts on a server it will not find on the next launch`() =
        runTest {
            val unwritable =
                object : DataStore<Preferences> {
                    override val data = flowOf(emptyPreferences())

                    override suspend fun updateData(
                        transform: suspend (t: Preferences) -> Preferences
                    ): Preferences = throw IOException("disk full")
                }
            val settings = PreferenceSettings(unwritable)

            val failure = runCatching {
                settings.setBaseUrl("https://nestodo.example.com/".toHttpUrl())
            }
                .exceptionOrNull()

            assertTrue(failure is IOException)
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
    fun `the usage data ID is kept until it is removed, so one phone is counted as one install across launches`() =
        runTest {
            val settings = PreferenceSettings(preferenceStore(folder))
            assertNull(settings.analyticsId())

            settings.setAnalyticsId("install-id")
            assertEquals("install-id", settings.analyticsId())

            settings.setAnalyticsId(null)
            assertNull(settings.analyticsId())
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
