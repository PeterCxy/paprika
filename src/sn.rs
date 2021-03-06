// Interface for Standard Notes (Actions)
use crate::{CONFIG, blog};
use crate::router::Router;
use crate::utils::*;
use js_sys::Date;
use serde::{Deserialize, Serialize, Serializer};
use std::vec::Vec;
use wasm_bindgen_futures::JsFuture;
use web_sys::*;

pub fn build_routes(router: &mut Router) {
    router.add_route("/actions", &get_actions);
    router.add_route("/post", &create_or_update_post);
    router.add_route("/delete", &delete_post);
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
    let preferred_url = crate::CONFIG.preferred_url.clone().unwrap_or(origin.clone());
    let mut actions = vec![];

    // Show different options depending on whether the post already exists
    // Use Post here because PostsList is larger to read into memory
    // also slower to check one-by-one
    // (also, because it may be a hidden post that does not live in PostsList)
    let post = match params.get("item_uuid") {
        Some(uuid) => match blog::Post::find_by_uuid(&uuid).await {
            Ok(post) => Some(post),
            Err(_) => None
        },
        None => None
    };
    let post_exists = post.is_some();

    actions.push(Action {
        label: if post_exists { "Update".into() } else { "Publish".into() },
        url: format!("{}/post?secret={}", origin, CONFIG.secret.clone()),
        verb: Verb::Post,
        context: Context::Item,
        content_types: vec![ContentType::Note],
        access_type: Some(AccessType::Decrypted)
    });

    if post_exists {
        actions.push(Action {
            label: "Delete".into(),
            url: format!("{}/delete?secret={}", origin, CONFIG.secret.clone()),
            verb: Verb::Post,
            context: Context::Item,
            content_types: vec![ContentType::Note],
            access_type: Some(AccessType::Decrypted)
        });

        actions.push(Action {
            label: "Open Post".into(),
            url: format!("{}/{}/", preferred_url, post.unwrap().url),
            verb: Verb::Show,
            context: Context::Item,
            content_types: vec![ContentType::Note],
            access_type: None
        });
    }

    actions.push(Action {
        label: "Open Blog".into(),
        url: preferred_url.clone(),
        verb: Verb::Show,
        context: Context::Item,
        content_types: vec![ContentType::Note],
        access_type: None
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
                "Content-Type" => "application/json",
                "Cache-Control" => "no-cache"
            }.add_cors().as_ref())
    ).internal_err()
}

#[derive(Deserialize)]
struct CustomMetadata {
    // If unlist is set to TRUE,
    // the post won't be listed publicly on the blog,
    // and if the post were published, it will
    // be removed from the public list
    unlist: Option<bool>,
    // An alias because I always want to use `unlisted` for some reason
    // `unlist` takes precedence over this alias
    unlisted: Option<bool>,
    url: Option<String>,
    // Same as Post.theme_config
    theme_config: Option<serde_json::Value>,
    timestamp: Option<String> // Should be something `js_sys::Date::parse` could handle
}

struct Metadata {
    unlist: bool,
    url: String,
    has_custom_url: bool,
    timestamp: u64, // Seconds
    has_custom_timestamp: bool
}

// You can customize metadata by adding something like
// 
// ```json
// {
//     "url": "xxx-xxx-xxx",
//     "timestamp": "YYYY-mm-dd"
// }
// ```
// 
// to the beginning of your article
// Normally when you update a post, the timestamp and URL will not be updated,
// but when you have custom metadata, they will always be updated.
// When the URL is updated, the old URL will automatically 301 to the new one
// An error will be returned if the JSON block exists but is malformed
// Otherwise, it's always Ok with metadata or no metadata
// This is to prevent people from sending a malformed header and accidentally
// making posts public
fn parse_custom_metadata_from_content(text: String) -> MyResult<(Option<CustomMetadata>, String)> {
    if !text.starts_with("```json\n") {
        Ok((None, text))
    } else {
        match text.find("```\n\n") {
            None => Ok((None, text)),
            Some(pos) => {
                let json = serde_json::from_str(&text[8..pos]).internal_err()?;
                return Ok((json, text[pos + 5..].to_owned()))
            }
        }
    }
}

// Generate metadata from uuid and title
// Fill in default value if custom value not present
fn build_metadata(custom: Option<CustomMetadata>, uuid: &str, title: &str) -> Metadata {
    // Default values
    let mut ret = Metadata {
        unlist: false,
        url: title_to_url(&uuid, &title),
        has_custom_url: false,
        timestamp: Date::now() as u64 / 1000, // Seconds
        has_custom_timestamp: false
    };

    if let Some(custom) = custom {
        if let Some(unlist) = custom.unlist {
            ret.unlist = unlist;
        } else if let Some(unlisted) = custom.unlisted {
            ret.unlist = unlisted;
        }

        if let Some(url) = custom.url {
            ret.url = url;
            ret.has_custom_url = true;
        }

        if let Some(date) = custom.timestamp {
            ret.timestamp = Date::parse(&date) as u64 / 1000; // Seconds
            ret.has_custom_timestamp = true;
        }
    }
    
    ret
}

macro_rules! load_post_body {
    ($data:ident, $req:expr) => {
        let $data: ActionsPostData = serde_json::from_str(
            &JsFuture::from($req.text().internal_err()?)
                .await.internal_err()?
                .as_string().ok_or(Error::BadRequest("Unable to parse POST body".into()))?
            ).internal_err()?;
        if $data.items.len() == 0 {
            return Err(Error::BadRequest("At least one item must be supplied".into()));
        }
    };
}

async fn create_or_update_post(req: Request, url: Url) -> MyResult<Response> {
    verify_secret!(url, params);
    if req.method() != "POST" {
        return Err(Error::BadRequest("Unsupported method".into()));
    }

    // Load the information sent as POST body
    load_post_body!(data, req);

    let uuid = data.items[0].uuid.clone();
    let text = data.items[0].content.text.clone();
    let title = data.items[0].content.title.clone();
    let (custom_metadata, text) = parse_custom_metadata_from_content(text)?;
    let theme_config = custom_metadata.as_ref().and_then(|it| it.theme_config.clone());
    let metadata = build_metadata(custom_metadata, &uuid, &title);
    let post = match blog::Post::find_by_uuid(&uuid).await {
        Ok(mut post) => {
            post.content = text;
            post.title = title;
            post.theme_config = theme_config;

            // Update metadata if custom ones are present
            if metadata.has_custom_url {
                post.url = metadata.url;
            }

            if metadata.has_custom_timestamp {
                post.timestamp = metadata.timestamp;
            }

            post
        },
        Err(_) => {
            blog::Post {
                url: metadata.url,
                uuid: uuid,
                title: title,
                content: text,
                timestamp: metadata.timestamp,
                theme_config: theme_config
            }
        }
    };

    // Write the new post to storage
    // As you may have seen by now, the process is far from atomic
    // This is fine because we don't expect users to update posts from
    // multiple endpoints simultaneously all the time
    let list = blog::PostsList::load().await;
    if !metadata.unlist {
        list.add_post(&post.uuid).await?;
    } else {
        list.remove_post(&post.uuid).await?;
    }
    // Also pre-render the post
    blog::PostContentCache::find_or_render(&post).await;
    // Finally, save the post
    post.write_to_kv().await?;

    Response::new_with_opt_str_and_init(
        None,
        ResponseInit::new()
            .status(200)
            .headers(headers!().add_cors().as_ref())
    ).internal_err()
}

async fn delete_post(req: Request, url: Url) -> MyResult<Response> {
    verify_secret!(url, params);
    if req.method() != "POST" {
        return Err(Error::BadRequest("Unsupported method".into()));
    }

    // Load the information sent as POST body
    load_post_body!(data, req);

    let uuid = &data.items[0].uuid;
    blog::PostsList::load().await.remove_post(uuid).await?;
    blog::Post::delete_by_uuid(uuid).await?;
    blog::PostContentCache::delete_by_uuid(uuid).await?;

    Response::new_with_opt_str_and_init(
        None,
        ResponseInit::new()
            .status(200)
            .headers(headers!().add_cors().as_ref())
    ).internal_err()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

// Many fields are omitted here since we don't use them for now
#[derive(Deserialize)]
pub struct ActionsPostItem {
    uuid: String,
    content: ActionsPostContent
}

#[derive(Deserialize)]
pub struct ActionsPostContent {
    title: String,
    text: String
}

#[derive(Deserialize)]
pub struct ActionsPostData {
    items: Vec<ActionsPostItem>
}