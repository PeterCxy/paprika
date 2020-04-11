// Front-end page rendering
use crate::blog;
use crate::router::Router;
use crate::utils::*;
use chrono::NaiveDateTime;
use handlebars::Handlebars;
use include_dir::{include_dir, Dir};
use js_sys::{Date, Uint8Array};
use rbtag::BuildDateTime;
use serde::Serialize;
use std::vec::Vec;
use web_sys::*;

// Allows user-configurable theme at build-time
// See build.rs
include!(concat!(env!("OUT_DIR"), "/load_theme.rs"));

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
    posts: Vec<HomePagePost>,
    prev: Option<String>,
    next: Option<String>
}

#[derive(Serialize)]
struct PostContext {
    blog: &'static BlogRootContext,
    title: String,
    url: String,
    timestamp: u64,
    content: String
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

handlebars_helper!(cur_year: | | Date::new_0().get_full_year());
handlebars_helper!(build_num: | | BuildTag{}.get_build_timestamp());
handlebars_helper!(format_date: |date: u64, format: str| {
    NaiveDateTime::from_timestamp(date as i64, 0).format(format).to_string()
});

fn build_handlebars() -> Handlebars<'static> {
    let mut hbs = Handlebars::new();

    // Helpers
    hbs.register_helper("cur_year", Box::new(cur_year));
    hbs.register_helper("build_num", Box::new(build_num));
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

pub async fn render_homepage(url: Url) -> MyResult<String> {
    let params = UrlSearchParams::new_with_str(&url.search())
        .map_err(|_| Error::BadRequest("Failed to parse query string".into()))?;
    let hbs = build_handlebars();
    let mut context = HomePageContext {
        blog: &ROOT_CONTEXT,
        posts: vec![],
        prev: None,
        next: None
    };
    let posts_list = blog::PostsList::load().await;

    // Pagination
    let mut posts_len = posts_list.0.len();
    let mut offset: isize = 0;
    if let Some(offset_str) = params.get("offset") {
        offset = offset_str.parse().internal_err()?;
        if offset > posts_len as isize || offset < 0 {
            return Err(Error::BadRequest("invalid offset".into()));
        }
        posts_len = posts_len - offset as usize;
    }

    if offset > 0 {
        let new_offset =
            std::cmp::max(offset - crate::CONFIG.posts_per_page as isize, 0) as usize;
        if new_offset != 0 {
            context.prev = Some(format!("/?offset={}", new_offset));
        } else {
            context.prev = Some("/".into());
        }
    }

    if posts_len == 0 {
        return Err(Error::BadRequest("offset too large".into()));
    }

    if posts_len > crate::CONFIG.posts_per_page {
        context.next = Some(
            format!("/?offset={}",
                offset + crate::CONFIG.posts_per_page as isize));
    }
    
    // List posts
    for uuid in posts_list.0.iter().skip(offset as usize).take(crate::CONFIG.posts_per_page) {
        let post = blog::Post::find_by_uuid(uuid).await?;
        let post_cache = blog::PostContentCache::find_or_render(&post).await;
        context.posts.push(HomePagePost {
            title: post.title,
            url: post.url,
            timestamp: post.timestamp,
            summary: post_cache.summary
        });
    }
    hbs.render("home.hbs", &context)
        .map_err(|e| Error::BadRequest(format!("{:#?}", e)))
}

pub async fn render_post(post: blog::Post) -> MyResult<String> {
    let hbs = build_handlebars();
    let post_cache = blog::PostContentCache::find_or_render(&post).await;
    let context = PostContext {
        blog: &ROOT_CONTEXT,
        title: post.title,
        url: post.url,
        timestamp: post.timestamp,
        content: post_cache.content
    };

    hbs.render("post.hbs", &context)
        .map_err(|e| Error::BadRequest(format!("{:#?}", e)))
}