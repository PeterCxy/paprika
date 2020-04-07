// Interface for Standard Notes (Actions)
use crate::{CONFIG, blog};
use crate::router::Router;
use crate::utils::*;
use serde::{Serialize, Serializer};
use std::vec::Vec;
use web_sys::*;

pub fn build_routes(router: &mut Router) {
    router.add_route("/actions", &get_actions);
}

macro_rules! verify_secret {
    ($url:expr, $params:ident) => {
        let $params = UrlSearchParams::new_with_str(&$url.search())
            .map_err(|_| Error::BadRequest("Failed to parse query string".into()))?;
        if !$params.has("secret") {
            return Err(Error::BadRequest("Secret needed".into()));
        } else if $params.get("secret").unwrap() != crate::CONFIG.secret {
            return Err(Error::Unauthorized("Secret mismatch".into()));
        }
    };
}

async fn get_actions(_req: Request, url: Url) -> MyResult<Response> {
    verify_secret!(url, params);

    let origin = url.origin();
    let mut actions = vec![];

    // Show different options depending on whether the post already exists
    let post_exists = match params.get("item_uuid") {
        Some(uuid) => {
            let posts = blog::PostsList::load().await;
            posts.has_post(&uuid)
        },
        None => false
    };

    actions.push(Action {
        label: if post_exists { "Update".into() } else { "Publish".into() },
        url: format!("{}/post?secret={}", origin, CONFIG.secret.clone()),
        verb: Verb::Post,
        context: Context::Item,
        content_types: vec![ContentType::Note],
        access_type: Some(AccessType::Decrypted)
    });

    let info = ActionsExtension {
        identifier: CONFIG.plugin_identifier.clone(),
        name: CONFIG.title.clone(),
        description: format!("Standard Notes plugin for {}", CONFIG.title.clone()),
        url: format!("{}/actions?secret={}", origin, CONFIG.secret.clone()),
        content_type: ContentType::Extension,
        supported_types: vec![ContentType::Note],
        actions
    };

    Response::new_with_opt_str_and_init(
        Some(&serde_json::to_string(&info).internal_err()?),
        ResponseInit::new()
            .status(200)
            .headers(headers!{
                "Content-Type" => "application/json"
            }.add_cors().as_ref())
    ).internal_err()
}

pub enum Verb {
    Show,
    Post,
    Render
}

impl Serialize for Verb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(match *self {
            Verb::Show => "show",
            Verb::Post => "post",
            Verb::Render => "render"
        })
    }
}

pub enum Context {
    Item
}

impl Serialize for Context {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(match *self {
            Context::Item => "Item"
        })
    }
}

pub enum ContentType {
    Note,
    Extension
}

impl Serialize for ContentType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(match *self {
            ContentType::Note => "Note",
            ContentType::Extension => "Extension"
        })
    }
}

pub enum AccessType {
    Decrypted,
    Encrypted
}

impl Serialize for AccessType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        serializer.serialize_str(match *self {
            AccessType::Decrypted => "decrypted",
            AccessType::Encrypted => "encrypted"
        })
    }
}

#[derive(Serialize)]
pub struct Action {
    label: String,
    url: String,
    verb: Verb,
    context: Context,
    content_types: Vec<ContentType>,
    access_type: Option<AccessType>
}

#[derive(Serialize)]
pub struct ActionsExtension {
    identifier: String,
    name: String,
    description: String,
    url: String,
    content_type: ContentType,
    supported_types: Vec<ContentType>,
    actions: Vec<Action>
}