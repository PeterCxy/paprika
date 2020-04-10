// Front-end page rendering
use crate::blog;
use crate::router::Router;
use crate::utils::*;
use chrono::NaiveDateTime;
use handlebars::Handlebars;
use include_dir::{include_dir, Dir};
use js_sys::{Date, Uint8Array};
use serde::Serialize;
use std::vec::Vec;
use web_sys::*;

// TODO: allow static configuration of which theme to use
const THEME_DIR: Dir = include_dir!("theme/default");

pub fn build_routes(router: &mut Router) {
    router.add_route("/static/", &serve_static);
}

async fn serve_static(_req: Request, url: Url) -> MyResult<Response> {
    let path = url.pathname();

    if let Some(file) = THEME_DIR.get_file(&path[1..path.len()]) {
        let u8arr: Uint8Array = file.contents().into();
        Response::new_with_opt_buffer_source_and_init(
            Some(&u8arr),
            ResponseInit::new()
                .status(200)
                .headers(headers!{
                    "Content-Type" => mime_guess::from_path(path).first().unwrap().essence_str()
                }.as_ref())
        ).internal_err()
    } else {
        Err(Error::NotFound("This file does not exist".into()))
    }
}

// Context objects used when rendering pages
#[derive(Serialize)]
struct BlogRootContext {
    theme_config: &'static serde_json::Value,
    title: &'static str,
    description: &'static str,
}

#[derive(Serialize)]
struct HomePagePost {
    title: String,
    url: String,
    timestamp: u64,
    summary: String
}

#[derive(Serialize)]
struct HomePageContext {
    blog: &'static BlogRootContext,
    posts: Vec<HomePagePost>
}

lazy_static! {
    static ref THEME_CONFIG: serde_json::Value = serde_json::from_str(
        include_str!("../theme_config.json")).unwrap();

    static ref ROOT_CONTEXT: BlogRootContext = {
        BlogRootContext {
            theme_config: &THEME_CONFIG,
            title: &crate::CONFIG.title,
            description: &crate::CONFIG.description
        }
    };
}

handlebars_helper!(cur_year: |dummy: u64| Date::new_0().get_full_year());
// TODO: actually implement this helper
handlebars_helper!(format_date: |date: u64, format: str| {
    NaiveDateTime::from_timestamp(date as i64, 0).format(format).to_string()
});

fn build_handlebars() -> Handlebars<'static> {
    let mut hbs = Handlebars::new();

    // Helpers
    hbs.register_helper("cur_year", Box::new(cur_year));
    hbs.register_helper("format_date", Box::new(format_date));

    // Templates
    for file in THEME_DIR.files() {
        let path = file.path().to_str().unwrap();
        if path.ends_with(".hbs") {
            // Register all .hbs templates
            hbs.register_template_string(
                path, file.contents_utf8().unwrap()).unwrap();
        }
    }
    return hbs;
}

pub async fn render_homepage() -> MyResult<String> {
    let hbs = build_handlebars();
    let mut context = HomePageContext {
        blog: &ROOT_CONTEXT,
        posts: vec![]
    };
    let posts_list = blog::PostsList::load().await;
    for uuid in posts_list.0.iter() {
        let post = blog::Post::find_by_uuid(uuid).await?;
        let post_cache = blog::PostContentCache::find_or_render(&post).await;
        context.posts.push(HomePagePost {
            title: post.title,
            url: post.url,
            timestamp: post.timestamp,
            summary: post_cache.content // TODO: make actual summaries
        });
    }
    hbs.render("home.hbs", &context)
        .map_err(|e| Error::BadRequest(format!("{:#?}", e)))
}