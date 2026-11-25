package io.github.yagipass.nestodo.data

import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import io.github.yagipass.nestodo.access.StorageCipher
import io.github.yagipass.nestodo.domain.ImageIcon
import io.github.yagipass.nestodo.domain.MyIcons
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.outlineOf
import io.github.yagipass.nestodo.preferenceStore
import java.security.InvalidKeyException
import java.util.Base64
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class PreferenceOutlineCacheTest {
    @get:Rule val folder = TemporaryFolder()
    private val saved = stringPreferencesKey("outline")
    private var locked = false
    private val keystore =
        object : StorageCipher {
            override fun seal(plain: String): String {
                if (locked) throw InvalidKeyException("the key needs an unlocked device")
                return Base64.getEncoder().encodeToString(plain.toByteArray())
            }

            override fun open(sealed: String): String {
                if (locked) throw InvalidKeyException("the key needs an unlocked device")
                return Base64.getDecoder().decode(sealed).decodeToString()
            }
        }
    private val outline =
        outlineOf(
            listOf(
                Todo(1, null, 0, "Groceries", collapsed = true),
                Todo(2, 1, 0, "Milk", completedAt = "2026-11-13 09:00:00", commentCount = 2),
                Todo(4, null, 1, "Dentist", icon = "\uD83E\uDDB7"),
                Todo(5, null, 2, "Bank", icon = "line:bank"),
            )
        )

    private val icons =
        MyIcons(
            listOf(
                ImageIcon("3f2a9c1e-0000-4000-8000-000000000001", "ミケ", "/api/v1/icons/a/image"),
                ImageIcon("3f2a9c1e-0000-4000-8000-000000000002", "Crest", "/api/v1/icons/b/image"),
            ),
            mapOf("3f2a9c1e-0000-4000-8000-000000000001" to byteArrayOf(0x52, 0x49, -1, 0, 7)),
        )

    private fun TestScope.iconStore() = preferenceStore(folder, name = "icons")

    private fun TestScope.cache() =
        PreferenceOutlineCache(preferenceStore(folder), iconStore(), keystore)

    @Test
    fun `saved todos come back as they were, so the next launch shows the same order, ticks, folds, icons and comment counts`() =
        runTest {
            val store = preferenceStore(folder)
            val iconStore = iconStore()
            PreferenceOutlineCache(store, iconStore, keystore).save("nestodo.example.com", outline)

            assertEquals(
                outline,
                PreferenceOutlineCache(store, iconStore, keystore).load("nestodo.example.com"),
            )
        }

    @Test
    fun `todos are sealed before they are written, so the file on the phone never holds a title in the clear`() =
        runTest {
            val store = preferenceStore(folder)

            val cache = PreferenceOutlineCache(store, iconStore(), keystore)
            assertTrue(cache.save("nestodo.example.com", outline))

            assertFalse(store.data.first()[saved]!!.contains("Groceries"))
        }

    @Test
    fun `todos saved from one server are not read for another, so a changed server never opens on the old server's list`() =
        runTest {
            val cache = cache()
            cache.save("nestodo.example.com", outline)

            assertNull(cache.load("other.example.com"))
        }

    @Test
    fun `cleared todos are gone from the phone, so signing out leaves nothing for the next launch to show`() =
        runTest {
            val cache = cache()
            cache.save("nestodo.example.com", outline)

            cache.clear()

            assertNull(cache.load("nestodo.example.com"))
        }

    @Test
    fun `todos the locked phone cannot seal are reported as not saved and the earlier ones are kept, so the caller can save again once it is unlocked`() =
        runTest {
            val cache = cache()
            cache.save("nestodo.example.com", outline)
            locked = true

            assertFalse(cache.save("nestodo.example.com", outlineOf(emptyList())))

            locked = false
            assertEquals(outline, cache.load("nestodo.example.com"))
        }

    @Test
    fun `saved todos the phone cannot unseal count as none, so a lost key opens on the server's list instead of crashing`() =
        runTest {
            val cache = cache()
            cache.save("nestodo.example.com", outline)
            locked = true

            assertNull(cache.load("nestodo.example.com"))
        }

    @Test
    fun `saved todos this version can't read count as none, so an update that changes how they are saved opens on the server's list instead of crashing`() =
        runTest {
            val store = preferenceStore(folder)
            store.edit {
                it[stringPreferencesKey("server")] = "nestodo.example.com"
                it[saved] = keystore.seal("{\"nodes\":")
            }

            assertNull(
                PreferenceOutlineCache(store, iconStore(), keystore).load("nestodo.example.com")
            )
        }

    @Test
    fun `saved images come back with their names and bytes, so the next launch draws them before the server answers`() =
        runTest {
            val store = preferenceStore(folder)
            val iconStore = iconStore()
            PreferenceOutlineCache(store, iconStore, keystore)
                .saveIcons("nestodo.example.com", icons)

            val loaded =
                PreferenceOutlineCache(store, iconStore, keystore)
                    .loadIcons("nestodo.example.com")!!

            assertEquals(icons.list, loaded.list)
            assertEquals(icons.images.keys, loaded.images.keys)
            assertArrayEquals(
                icons.images.values.single(),
                loaded.images.getValue(icons.list.first().id),
            )
        }

    @Test
    fun `images are sealed before they are written, so the file on the phone never holds an image name in the clear`() =
        runTest {
            val iconStore = iconStore()

            assertTrue(
                PreferenceOutlineCache(preferenceStore(folder), iconStore, keystore)
                    .saveIcons("nestodo.example.com", icons)
            )

            assertFalse(iconStore.data.first()[stringPreferencesKey("icons")]!!.contains("Crest"))
        }

    @Test
    fun `images saved from one server are not read for another, so a changed server never draws the old server's pictures`() =
        runTest {
            val cache = cache()
            cache.saveIcons("nestodo.example.com", icons)

            assertNull(cache.loadIcons("other.example.com"))
        }

    @Test
    fun `clearing removes the images too, so signing out leaves no pictures on the phone`() =
        runTest {
            val cache = cache()
            cache.save("nestodo.example.com", outline)
            cache.saveIcons("nestodo.example.com", icons)

            cache.clear()

            assertNull(cache.loadIcons("nestodo.example.com"))
        }
}
