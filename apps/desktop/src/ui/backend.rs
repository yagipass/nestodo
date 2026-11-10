use nestodo_desktop::api::{Client, StatsResponse};
use nestodo_desktop::error::Error;
use nestodo_desktop::wire::{Reply, Request, Todo};

pub trait Backend: Send + Sync + 'static {
    fn list(&self) -> Result<Vec<Todo>, Error>;
    fn request(&self, request: &Request) -> Result<Reply, Error>;
    fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error>;
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
}
