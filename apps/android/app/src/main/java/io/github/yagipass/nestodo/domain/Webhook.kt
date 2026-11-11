package io.github.yagipass.nestodo.domain

import kotlinx.serialization.Serializable

@Serializable data class WebhookHeader(val name: String, val value: String, val secret: Boolean)

@Serializable
data class Webhook(
    val url: String? = null,
    val hasToken: Boolean = false,
    val unreadable: Boolean = false,
    val headers: List<WebhookHeader> = emptyList(),
    val project: String,
    val lastSentAt: String? = null,
    val lastError: String? = null,
)

@Serializable data class Webhooks(val available: Boolean, val webhook: Webhook? = null)

@Serializable data class Analytics(val available: Boolean = false, val enabled: Boolean)

@Serializable
data class WebhookEdit(
    val url: String,
    val urlSecret: Boolean,
    val token: String?,
    val headers: List<WebhookHeader>,
    val project: String,
)

data class WebhookForm(
    val url: String,
    val urlSecret: Boolean,
    val token: String?,
    val headers: List<WebhookHeader>,
    val project: String,
    val existing: Webhook?,
) {
    private val saved: Webhook?
        get() = existing?.takeUnless { it.unreadable }

    val replacing: Boolean
        get() = saved != null

    val keepsHiddenUrl: Boolean
        get() = saved?.let { it.url == null } == true && urlSecret

    private val sameUrl: Boolean
        get() = url == (saved?.url ?: "")

    val tokenKept: Boolean
        get() = saved?.hasToken == true && sameUrl && token == ""

    fun valueKept(header: WebhookHeader): Boolean =
        header.secret &&
            sameUrl &&
            saved?.headers.orEmpty().any {
                it.secret && it.name.equals(header.name.trim(), ignoreCase = true)
            }

    fun withHeader(index: Int, header: WebhookHeader): WebhookForm =
        copy(headers = headers.mapIndexed { at, old -> if (at == index) header else old })

    fun withoutHeader(index: Int): WebhookForm =
        copy(headers = headers.filterIndexed { at, _ -> at != index })

    fun withNewHeader(): WebhookForm =
        copy(headers = headers + WebhookHeader(name = "", value = "", secret = true))

    val edit: WebhookEdit
        get() = WebhookEdit(url, urlSecret, token, headers, project)

    companion object {
        fun of(webhook: Webhook?): WebhookForm =
            WebhookForm(
                url = webhook?.url ?: "",
                urlSecret = webhook?.let { it.url == null } ?: true,
                token = "",
                headers = webhook?.headers.orEmpty(),
                project = webhook?.project ?: "",
                existing = webhook,
            )
    }
}
