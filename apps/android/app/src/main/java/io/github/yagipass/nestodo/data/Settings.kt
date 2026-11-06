package io.github.yagipass.nestodo.data

import android.content.Context

interface Settings {
    var showCompleted: Boolean
    var shareParent: Long?
}

class PreferenceSettings(context: Context) : Settings {
    private val prefs = context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    override var showCompleted: Boolean
        get() = prefs.getBoolean(SHOW_COMPLETED, true)
        set(value) = prefs.edit().putBoolean(SHOW_COMPLETED, value).apply()

    override var shareParent: Long?
        get() = if (prefs.contains(SHARE_PARENT)) prefs.getLong(SHARE_PARENT, 0) else null
        set(value) {
            val edit = prefs.edit()
            if (value == null) edit.remove(SHARE_PARENT) else edit.putLong(SHARE_PARENT, value)
            edit.apply()
        }

    private companion object {
        const val SHOW_COMPLETED = "show_completed"
        const val SHARE_PARENT = "share_parent"
    }
}
