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
use utils::*;
use wasm_bindgen::prelude::*;
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
    sn::build_routes(&mut router);
    return router;
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
                    Some(&post.content),
                    ResponseInit::new()
                        .status(200)
                        .headers(headers!{
                            "Content-Type" => "text/html"
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