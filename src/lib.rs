#![feature(vec_remove_item)]

#[macro_use]
extern crate handlebars;
#[macro_use]
extern crate lazy_static;

mod task;
#[macro_use]
mod utils;
mod router;
mod store;
mod hljs;
mod blog;
mod sn;
mod render;

use cfg_if::cfg_if;
use js_sys::{Promise};
use utils::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::*;

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

lazy_static! {
    static ref ROUTER: router::Router = {
        build_routes()
    };

    pub static ref CONFIG: utils::Config = {
        serde_json::from_str(std::include_str!("../config.json")).unwrap()
    };

    pub static ref CACHE_CONTROL_STATIC_FILE: String = {
        format!("max-age={}", CONFIG.cache_maxage)
    };
}

fn build_routes() -> router::Router {
    let mut router = router::Router::new(&default_route);
    router.add_route(blog::IMG_CACHE_PREFIX, &proxy_remote_image);
    sn::build_routes(&mut router);
    render::build_routes(&mut router);
    return router;
}

#[wasm_bindgen]
extern "C" {
    // This binds to the fetch function in global scope
    // In cloudflare workers, there's no Window object
    // and unfortunately the bionding in web_sys depends
    // on Window being present.
    fn fetch(req: &Request) -> Promise;
}

macro_rules! get_header {
    ($headers:expr, $name:expr) => {
        $headers.get($name).internal_err()?.ok_or(Error::InternalError())?
    };
}

// A caching proxy for images inserted into articles
// to protect user's privacy and accelerate page load
async fn proxy_remote_image(req: Request, url: Url) -> MyResult<Response> {
    if req.method() != "GET" {
        return Err(Error::BadRequest("Unsupported method".into()));
    }

    let path = url.pathname();
    let remote_url: String = js_sys::decode_uri_component(
        &path[blog::IMG_CACHE_PREFIX.len()..path.len()]
    ).internal_err()?.into();

    if !blog::PostContentCache::is_external_url_whitelisted_for_cache(&remote_url).await {
        return Err(Error::Unauthorized("This URL is not whitelisted".into()));
    }

    let new_req = Request::new_with_str_and_init(&remote_url,
        RequestInit::new()
            .method("GET")
            .redirect(RequestRedirect::Follow)).internal_err()?;
    let remote_resp: Response = JsFuture::from(fetch(&new_req)).await.internal_err()?.into();
    let remote_headers = remote_resp.headers();

    Response::new_with_opt_readable_stream_and_init(
        remote_resp.body().as_ref(),
        ResponseInit::new()
            .status(remote_resp.status())
            .headers(headers!{
                "Content-Type" => &get_header!(remote_headers, "content-type"),
                "Cache-Control" => &CACHE_CONTROL_STATIC_FILE
            }.as_ref())
    ).internal_err()
}

async fn default_route(_req: Request, url: Url) -> MyResult<Response> {
    // We assume that anything that falls into this catch-all handler
    // would be posts, 404, or hard-codede redirects
    let path = url.pathname();

    // Handle hard-coded redirects in config first
    if let Some(redirects) = &CONFIG.redirects {
        if let Some(new_path) = redirects.get(&path) {
            return Response::new_with_opt_str_and_init(
                None,
                ResponseInit::new()
                    .status(301)
                    .headers(headers!{
                        "Location" => &format!("{}{}", url.origin(), new_path)
                    }.as_ref())
            ).internal_err();
        }
    }

    // If the path doesn't end with `/`, normalize it first
    if !path.ends_with("/") {
        return Response::new_with_opt_str_and_init(
            None,
            ResponseInit::new()
                .status(302)
                .headers(headers!{
                    "Location" => &format!("{}{}/", url.origin(), path)
                }.as_ref())
        ).internal_err();
    }

    // Home page (this cannot be registered as a standalone route due to our Router)
    if path == "/" {
        return Response::new_with_opt_str_and_init(
            Some(&render::render_homepage(url).await?),
            ResponseInit::new()
                .status(200)
                .headers(headers!{
                    "Content-Type" => "text/html",
                    "Cache-Control" => "no-cache"
                }.as_ref())
        ).internal_err();
    }

    // Now we can be sure the path ends with `/`
    // (and of course it starts with `/` as per standard)
    if path.len() > 1 {
        let path = &path[1..path.len() - 1];
        if let Ok(post) = blog::Post::find_by_url(path).await {
            if post.url != path {
                // Redirect to the latest path of the post
                return Response::new_with_opt_str_and_init(
                    None,
                    ResponseInit::new()
                        .status(301)
                        .headers(headers!{
                            "Location" => &format!("{}/{}/", url.origin(), post.url)
                        }.as_ref())
                ).internal_err();
            } else {
                // Render the page
                return Response::new_with_opt_str_and_init(
                    Some(&render::render_post(url, post).await?),
                    ResponseInit::new()
                        .status(200)
                        .headers(headers!{
                            "Content-Type" => "text/html",
                            "Cache-Control" => "no-cache"
                        }.as_ref())
                ).internal_err();
            }
        }
    }

    Err(Error::NotFound("This page is not available".into()))
}

task_local! {
    pub static EVENT: ExtendableEvent;
}

#[wasm_bindgen]
pub async fn handle_request_rs(ev: ExtendableEvent, req: Request) -> Response {
    let url = Url::new(&req.url()).unwrap();

    if req.method() == "OPTIONS" {
        return Response::new_with_opt_str_and_init(
            None, ResponseInit::new()
                .status(200)
                .headers(headers!().add_cors().as_ref())
        ).unwrap();
    }

    let result = EVENT.scope(ev, async move {
        ROUTER.execute(req, url).await
    }).await;

    match result {
        Ok(resp) => resp,
        Err(err) => {
            let code = err.status_code();
            let reason: String = err.into();
            Response::new_with_opt_str_and_init(
                Some(&reason), ResponseInit::new().status(code)
            ).unwrap()
        }
    }
}