#![feature(vec_remove_item)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
mod utils;
mod router;
mod store;
mod blog;
mod sn;

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
}

fn build_routes() -> router::Router {
    let mut router = router::Router::new(&default_route);
    router.add_route("/hello", &hello_world);
    router.add_route(blog::IMG_CACHE_PREFIX, &proxy_remote_image);
    sn::build_routes(&mut router);
    return router;
}

#[wasm_bindgen]
extern "C" {
    fn fetch(req: &Request) -> Promise;
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
    Ok(JsFuture::from(fetch(&new_req)).await.internal_err()?.into())
}

async fn default_route(_req: Request, url: Url) -> MyResult<Response> {
    // We assume that anything that falls into this catch-all handler
    // would be either posts or 404
    // If the path doesn't end with `/`, normalize it first
    let path = url.pathname();
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

    // TODO: handle home page and pagination on home page
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
                // TODO: Actually render the page...
                return Response::new_with_opt_str_and_init(
                    Some(&blog::PostContentCache::find_or_render(&post).await.content),
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

async fn hello_world(_req: Request, _url: Url) -> MyResult<Response> {
    Response::new_with_opt_str_and_init(
        Some("Hello, world from Rust"),
        ResponseInit::new().status(200)
    ).internal_err()
}

#[wasm_bindgen]
pub async fn handle_request_rs(req: Request) -> Response {
    let url = Url::new(&req.url()).unwrap();

    if req.method() == "OPTIONS" {
        return Response::new_with_opt_str_and_init(
            None, ResponseInit::new()
                .status(200)
                .headers(headers!().add_cors().as_ref())
        ).unwrap();
    }

    let result = ROUTER.execute(req, url).await;

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