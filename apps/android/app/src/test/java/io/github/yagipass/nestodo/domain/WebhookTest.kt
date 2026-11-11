package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WebhookTest {
    private val saved =
        Webhook(
            url = null,
            hasToken = true,
            headers = listOf(WebhookHeader("X-Api-Key", "", secret = true)),
            project = "Side projects",
        )

    @Test
    fun `a new webhook keeps its URL secret unless asked, since a URL can itself hold a secret`() {
        val form = WebhookForm.of(null)
        assertTrue(form.urlSecret)
        assertEquals(WebhookEdit("", true, "", emptyList(), ""), form.edit)
    }

    @Test
    fun `the saved token is kept only while the URL stays the same, so the form never promises to send it elsewhere`() {
        val form = WebhookForm.of(saved)
        assertTrue(form.tokenKept)
        assertFalse(form.copy(url = "https://other.example/hook").tokenKept)
        assertFalse(form.copy(token = null).tokenKept)
    }

    @Test
    fun `a secret header value is kept only for the same name and URL, whatever the letter case`() {
        val form = WebhookForm.of(saved)
        assertTrue(form.valueKept(WebhookHeader(" x-api-key ", "", secret = true)))
        assertFalse(form.valueKept(WebhookHeader("X-Api-Version", "", secret = true)))
        assertFalse(form.valueKept(WebhookHeader("X-Api-Key", "", secret = false)))
        assertFalse(
            form
                .copy(url = "https://other.example/hook")
                .valueKept(WebhookHeader("X-Api-Key", "", secret = true))
        )
    }

    @Test
    fun `a hidden saved URL may be left blank, and the form says so only while it stays secret`() {
        val form = WebhookForm.of(saved)
        assertTrue(form.keepsHiddenUrl)
        assertFalse(form.copy(urlSecret = false).keepsHiddenUrl)
        assertFalse(WebhookForm.of(saved.copy(url = "https://example.com/hook")).keepsHiddenUrl)
    }

    @Test
    fun `a webhook that can no longer be read keeps nothing, so every value is asked for again`() {
        val form = WebhookForm.of(saved.copy(unreadable = true))
        assertFalse(form.replacing)
        assertFalse(form.tokenKept)
        assertFalse(form.keepsHiddenUrl)
        assertFalse(form.valueKept(WebhookHeader("X-Api-Key", "", secret = true)))
    }

    @Test
    fun `headers are added as secret and removed by position, leaving the others as typed`() {
        val form =
            WebhookForm.of(null)
                .withNewHeader()
                .withHeader(0, WebhookHeader("X-Api-Version", "2", secret = false))
                .withNewHeader()
        assertEquals(WebhookHeader("", "", secret = true), form.headers[1])
        assertEquals(
            listOf(WebhookHeader("", "", secret = true)),
            form.withoutHeader(0).headers,
        )
    }
}
