package io.github.yagipass.nestodo.access

import android.content.SharedPreferences
import io.github.yagipass.nestodo.MemoryPreferences
import io.github.yagipass.nestodo.preferenceStore
import java.security.InvalidKeyException
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class KeystoreTokenStoreTest {
    @get:Rule val folder = TemporaryFolder()
    private var locked = false
    private val keystore =
        object : TokenCipher {
            var readable = true

            override fun seal(plain: String): String {
                if (locked) throw InvalidKeyException("the key needs an unlocked device")
                return "sealed:$plain"
            }

            override fun open(sealed: String): String {
                if (locked || !readable) throw InvalidKeyException("the key cannot open this")
                return sealed.removePrefix("sealed:")
            }
        }

    private fun TestScope.store(legacy: SharedPreferences? = null) =
        KeystoreTokenStore(preferenceStore(folder, legacy), keystore) { locked }

    @Test
    fun `keeps the saved sign-in when the locked phone cannot read it, so locking the screen during a sync never signs the member out`() =
        runTest {
            val store = store()
            store.save("nestodo.example.com", TokenKind.Org, "org")
            locked = true

            val error = runCatching { store.load("nestodo.example.com", TokenKind.Org) }
            assertTrue(error.exceptionOrNull() is DeviceLockedException)

            locked = false
            assertEquals("org", store.load("nestodo.example.com", TokenKind.Org))
        }

    @Test
    fun `forgets a sign-in that an unlocked phone cannot read, such as one sealed by an earlier key, so the member is asked to sign in once instead of failing every time`() =
        runTest {
            val store = store()
            store.save("nestodo.example.com", TokenKind.App, "app")
            keystore.readable = false

            assertNull(store.load("nestodo.example.com", TokenKind.App))

            keystore.readable = true
            assertNull(store.load("nestodo.example.com", TokenKind.App))
        }

    @Test
    fun `reports a sign-in that cannot be saved while the phone is locked as temporary, so the caller can try again once it is unlocked`() =
        runTest {
            val store = store()
            locked = true

            val error = runCatching { store.save("nestodo.example.com", TokenKind.App, "app") }
            assertTrue(error.exceptionOrNull() is DeviceLockedException)

            locked = false
            assertNull(store.load("nestodo.example.com", TokenKind.App))
        }

    @Test
    fun `a sign-in saved before the app moved to DataStore still works, so updating the app does not open the browser again`() =
        runTest {
            val store = store(MemoryPreferences("nestodo.example.com/Org" to "sealed:org"))

            assertEquals("org", store.load("nestodo.example.com", TokenKind.Org))
        }
}
