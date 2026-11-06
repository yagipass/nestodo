package io.github.yagipass.nestodo.access

import android.content.SharedPreferences
import java.security.InvalidKeyException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class KeystoreTokenStoreTest {
    private val prefs = MemoryPreferences()
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
    private val store = KeystoreTokenStore(prefs, keystore) { locked }

    @Test
    fun `keeps the saved sign-in when the locked phone cannot read it, so locking the screen during a sync never signs the member out`() {
        store.save("nestodo.example.com", TokenKind.Org, "org")
        locked = true

        assertThrows(DeviceLockedException::class.java) {
            store.load("nestodo.example.com", TokenKind.Org)
        }

        locked = false
        assertEquals("org", store.load("nestodo.example.com", TokenKind.Org))
    }

    @Test
    fun `forgets a sign-in that an unlocked phone cannot read, such as one sealed by an earlier key, so the member is asked to sign in once instead of failing every time`() {
        store.save("nestodo.example.com", TokenKind.App, "app")
        keystore.readable = false

        assertNull(store.load("nestodo.example.com", TokenKind.App))
        assertNull(prefs.getString("nestodo.example.com/App", null))
    }

    @Test
    fun `reports a sign-in that cannot be saved while the phone is locked as temporary, so the caller can try again once it is unlocked`() {
        locked = true

        assertThrows(DeviceLockedException::class.java) {
            store.save("nestodo.example.com", TokenKind.App, "app")
        }
        assertNull(prefs.getString("nestodo.example.com/App", null))
    }
}

private class MemoryPreferences : SharedPreferences {
    private val values = mutableMapOf<String, Any?>()

    override fun getAll(): Map<String, *> = values.toMap()

    override fun getString(key: String, defValue: String?) = values[key] as String? ?: defValue

    override fun getStringSet(key: String, defValues: Set<String>?) = defValues

    override fun getInt(key: String, defValue: Int) = defValue

    override fun getLong(key: String, defValue: Long) = defValue

    override fun getFloat(key: String, defValue: Float) = defValue

    override fun getBoolean(key: String, defValue: Boolean) = defValue

    override fun contains(key: String) = key in values

    override fun edit(): SharedPreferences.Editor = Editor()

    override fun registerOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) = Unit

    override fun unregisterOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) = Unit

    private inner class Editor : SharedPreferences.Editor {
        private val changes = mutableListOf<(MutableMap<String, Any?>) -> Unit>()

        override fun putString(key: String, value: String?): SharedPreferences.Editor {
            changes += { it[key] = value }
            return this
        }

        override fun putStringSet(key: String, values: Set<String>?) = this

        override fun putInt(key: String, value: Int) = this

        override fun putLong(key: String, value: Long) = this

        override fun putFloat(key: String, value: Float) = this

        override fun putBoolean(key: String, value: Boolean) = this

        override fun remove(key: String): SharedPreferences.Editor {
            changes += { it.remove(key) }
            return this
        }

        override fun clear(): SharedPreferences.Editor {
            changes += { it.clear() }
            return this
        }

        override fun commit(): Boolean {
            apply()
            return true
        }

        override fun apply() {
            changes.forEach { it(values) }
            changes.clear()
        }
    }
}
