use nestodo_desktop::api::{Client, StatsResponse};
use nestodo_desktop::error::Error;
use nestodo_desktop::webhook::{Analytics, WebhookEdit, Webhooks};
use nestodo_desktop::wire::{Reply, Request, Todo};

pub trait Backend: Send + Sync + 'static {
    fn list(&self) -> Result<Vec<Todo>, Error>;
    fn request(&self, request: &Request) -> Result<Reply, Error>;
    fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error>;
    fn webhooks(&self) -> Result<Webhooks, Error>;
    fn save_webhook(&self, edit: &WebhookEdit) -> Result<Webhooks, Error>;
    fn remove_webhook(&self) -> Result<(), Error>;
    fn analytics(&self) -> Result<Analytics, Error>;
    fn set_analytics(&self, enabled: bool) -> Result<bool, Error>;
}

impl Backend for Client {
    fn list(&self) -> Result<Vec<Todo>, Error> {
        Client::list(self)
    }

    fn request(&self, request: &Request) -> Result<Reply, Error> {
        Client::request(self, request)
    }

    fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error> {
        Client::stats(self, time_zone)
    }

    fn webhooks(&self) -> Result<Webhooks, Error> {
        Client::webhooks(self)
    }

    fn save_webhook(&self, edit: &WebhookEdit) -> Result<Webhooks, Error> {
        Client::save_webhook(self, edit)
    }

    fn remove_webhook(&self) -> Result<(), Error> {
        Client::remove_webhook(self)
    }

    fn analytics(&self) -> Result<Analytics, Error> {
        Client::analytics(self)
    }

    fn set_analytics(&self, enabled: bool) -> Result<bool, Error> {
        Client::set_analytics(self, enabled)
    }
}
