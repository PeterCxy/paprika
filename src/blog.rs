// Utility functions and structs for the blogging system 
// Due to limitations of the Cloudflare Workers KV, we do not
// store the entire state in one record; instead, different
// parts are stroed in different records. This also increases
// efficiency, since the program won't need to load anything
// unnecessary from KV.
use crate::store;
use crate::utils::*;
use js_sys::{JsString, RegExp};
use pulldown_cmark::*;
use serde::{Serialize, Deserialize};
use std::vec::Vec;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::spawn_local;

// A list of the UUIDs of all published blog posts
// This should be SORTED with the newest posts at lower indices (closer to 0)
// The user may edit this via KV UI to change ordering and such
// by default new posts are always added to the top
#[derive(Serialize, Deserialize)]
pub struct PostsList(pub Vec<String>);

impl PostsList {
    pub async fn load() -> PostsList {
        match store::get_obj("posts_list").await {
            Ok(v) => PostsList(v),
            // Don't panic on empty
            // TODO: What if the user messed up when editing?
            //       That would cause a decode failure and all data will be gone
            //       if a new post is added or updated (overwriting the KV value) 
            //       under this logic
            //       (if no new post is added then nothing bad would happen;
            //        the user would probably notice when trying to visit the blog home page)
            Err(_) => PostsList(vec![])
        }
    }

    pub fn has_post(&self, uuid: &str) -> bool {
        self.0.contains(&uuid.into())
    }

    // Add a post to the list and then update the record in KV
    // Also consumes self, as this should normally be the last action
    // in an API call
    pub async fn add_post(mut self, uuid: &str) -> MyResult<()> {
        if self.has_post(uuid) {
            return Ok(());
        }

        self.0.insert(0, uuid.into());
        store::put_obj_pretty("posts_list", self.0).await
    }

    // Remove a post from published list
    // may be used when deleting / unpublishing a post
    // Does nothing if uuid not found in list
    pub async fn remove_post(mut self, uuid: &str) -> MyResult<()> {
        self.0.remove_item(&uuid);
        store::put_obj_pretty("posts_list", self.0).await
    }
}

#[derive(Serialize, Deserialize)]
pub struct Post {
    // The UUID of the post (a Standard Notes UUID)
    pub uuid: String,
    // The UNIX timestamp (in seconds) for the post
    pub timestamp: u64,
    // URL of the post (relative to the root of the site)
    pub url: String,
    // Title of the post
    pub title: String,
    // The Markdown content of the post
    // We keep the original content here
    // so that we could make changes to the Markdown parser
    // in the future; we won't be stuck with a parsed version
    pub content: String,
    // Some arbitrary data that could be used by the theme
    pub theme_config: Option<serde_json::Value>
}

impl Post {
    fn uuid_to_post_key(uuid: &str) -> String {
        format!("post_by_uuid_{}", uuid)
    }

    fn url_to_mapping_key(url: &str) -> String {
        format!("url_mapping_{}", url)
    }

    async fn create_url_mapping(url: &str, uuid: &str) -> MyResult<()> {
        store::put_str(&Self::url_to_mapping_key(url), uuid).await
    }

    // Returns Err(InternalError) if the post is not found
    // Note that the existence status of a post here must
    // be synchronized with the PostsList; that is, if a
    // post is not found in PostsList, it must not be found
    // here either; if a post is found in PostsList, then
    // this method should not return any error.
    // (except for hidden posts, in which case they won't be
    //  present in PostsList)
    pub async fn find_by_uuid(uuid: &str) -> MyResult<Post> {
        store::get_obj(&Self::uuid_to_post_key(uuid)).await
    }

    pub async fn find_by_url(url: &str) -> MyResult<Post> {
        let uuid = store::get_str(&Self::url_to_mapping_key(url)).await?;
        Self::find_by_uuid(&uuid).await
    }

    // Write the Post to KV storage; this can be a new post or
    // update to an existing post; either way, the CALLER is
    // responsible for making sure PostsList is updated with the
    // latest set of posts sorted in order.
    // This function will also create a mapping from URL to UUID in the KV
    pub async fn write_to_kv(self) -> MyResult<()> {
        Self::create_url_mapping(&self.url, &self.uuid).await?;
        store::put_obj(&Self::uuid_to_post_key(&self.uuid), self).await
    }

    pub async fn delete_by_uuid(uuid: &str) -> MyResult<()> {
        store::delete(&Self::uuid_to_post_key(uuid)).await
    }
}

lazy_static! {
    // Whenever this is changed, all cache will be invalided
    // Use build timestamp string
    static ref CACHE_VERSION: String = {
        format!("{}", BUILD_TIMESTAMP)
    };
}

// The prefix path used for caching remote images
pub const IMG_CACHE_PREFIX: &'static str = "/imgcache/";

// The divider for summary
// Insert this into the article as a standalone line to
// make everything above it the summary. DO NOT insert
// it within paragraph or anything else otherwise
// the layout may break.
const SUMMARY_DIVIDER: &'static str = "<!-- More -->";

// Cached version of rendered blog content HTMLs
// compiled from Markdown
// This is needed because 
// (1) we have to extract some information from
//     the Markdown source code before anyone
//     visits, e.g. pictures it refers to (
//     for whitelisting the cache URL)
// (2) Markdown parsing is slower than filling in
//     HTML templates of the entire page. If these
//     Markdown compilation results are cached,
//     page generation can be very fast while still
//     keeping some dynamic features available to us
#[derive(Serialize, Deserialize)]
pub struct PostContentCache {
    // UUID of the original post
    uuid: String,
    // If version != CACHE_VERSION, the cache is invalidated
    version: String,
    // Digest of the original content
    orig_digest: String,
    // Summary can be defined by inserting SUMMARY_DIVIDER
    // into the article. Everything before this tag will be
    // the summary. Becuase it's an HTML comment, it won't
    // show up in the rendered result.
    pub summary: String,
    // Compiled content in HTML
    pub content: String
}

impl PostContentCache {
    fn uuid_to_cache_key(uuid: &str) -> String {
        format!("content_cache_{}", uuid)
    }

    fn url_to_cache_whitelist_key(url: &str) -> String {
        format!("cache_whitelist_{}", url)
    }

    pub async fn is_external_url_whitelisted_for_cache(url: &str) -> bool {
        if let Some(list) = &crate::CONFIG.extra_remote_proxy_whitelist {
            if list.contains(&url.into()) {
                return true;
            }
        }

        match store::get_str(&Self::url_to_cache_whitelist_key(url)).await {
            Ok(s) => s == "Y",
            Err(_) => false
        }
    }

    async fn find_by_uuid(uuid: &str) -> MyResult<PostContentCache> {
        store::get_obj(&Self::uuid_to_cache_key(uuid)).await
    }

    pub async fn find_by_post(post: &Post) -> Option<PostContentCache> {
        let cache = match Self::find_by_uuid(&post.uuid).await {
            Ok(cache) => cache,
            Err(_) => return None
        };

        if cache.version != *CACHE_VERSION {
            return None;
        }

        if cache.orig_digest != crate::utils::sha1(&post.content).await {
            return None;
        }

        Some(cache)
    }

    fn transform_tag<'a>(tag: &mut Tag<'a>) {
        match tag {
            Tag::Image(_, url, _) => {
                // Convert all external image to our cached URL
                // to protect users and speed up page loading
                let url_encoded: String = js_sys::encode_uri_component(url).into();
                // Also write this URL to whitelist
                // (just throw the task onto the JS ev loop,
                //  because to make this function async we MUST need to
                //  allocate Vec later in the render function)
                // we don't care about if this write succeeds or not,
                // because even if it breaks we still can recover by a simple refresh
                // and once it's written, it's permanent, so we expect the write
                // to succeed as soon as the article is submitted
                let url_cache_key = Self::url_to_cache_whitelist_key(url);
                spawn_local(async move {
                    let _ = store::put_str(&url_cache_key, "Y").await;
                    ()
                });
                // Now we can overwrite the tag URL
                *url = format!("{}{}", IMG_CACHE_PREFIX, url_encoded).into();
            },
            _ => ()
        }
    }

    fn transform_tags<'ev>(
        parser: impl Iterator<Item = Event<'ev>>
    ) -> impl Iterator<Item = Event<'ev>> {
        parser.map(|mut ev| {
            match ev {
                Event::Start(ref mut tag) | Event::End(ref mut tag) => {
                    Self::transform_tag(tag);
                    ev
                },
                _ => ev
            }
        })
    }

    fn transform_code_block_highlight<'ev>(
        parser: impl Iterator<Item = Event<'ev>>
    ) -> impl Iterator<Item = Event<'ev>> {
        let mut in_code_block = false;
        let mut code_block_lang = None;
        parser.map(move |ev| {
            match &ev {
                Event::Start(Tag::CodeBlock(block)) => {
                    in_code_block = true;
                    match block {
                        CodeBlockKind::Fenced(lang) => code_block_lang = Some(lang.to_string()),
                        CodeBlockKind::Indented => code_block_lang = None
                    }
                },
                Event::End(Tag::CodeBlock(_)) => {
                    in_code_block = false;
                    code_block_lang = None;
                },
                Event::Text(text) => {
                    if in_code_block {
                        let highlighted = if let Some(ref code_block_lang) = code_block_lang {
                            crate::hljs::highlight(&code_block_lang, text)
                        } else {
                            crate::hljs::highlight_auto(text)
                        };

                        return Event::Html(
                            highlighted.into());
                    }
                }
                _ => ()
            }

            ev
        })
    }

    // Do some HTML-level transformations to the compiled result
    // Because the Markdown parser doesn't always allow us to do
    // everything, like adding `id` attributes to tags
    fn transform_html(html: String) -> String {
        let js_html: JsString = html.into();

        // Add `id="xxx"` to all headings for anchoring
        // Replacing is done in a Closure in order to generate
        // the proper anchor string for each heading
        // This matches only a single line, which is good because
        // we only want it to match a single heading tag
        // If it matched multiple lines, then it may match the
        // ending tag of another heading.
        let regex_heading = RegExp::new(r"<h(\d)>([^<]*)<\/h\1>", "ig");
        let closure = Closure::wrap(Box::new(|_m: String, p1: String, p2: String| {
            let anchor = filter_non_ascii_alphanumeric(
                &p2.to_lowercase()).replace(" ", "-");
            format!("<h{} id=\"{}\">{}</h{}>", p1, anchor, p2, p1)
        }) as Box<dyn Fn(String, String, String) -> String>);
        let js_html = js_html.replace_by_pattern_with_function(&regex_heading, closure.as_ref().unchecked_ref());

        // Transform all <pre><code> to <pre><code class="hljs">
        // For syntax highlighting
        // We don't match the end tag because it may span multiple lines
        // trying to match the end tag could result in accidentally matching
        // the end tag of another code block.
        let regex_code = RegExp::new("<pre><code( class=\"language-([^\"]*)\")?>", "ig");
        let js_html = js_html.replace_by_pattern(&regex_code, "<pre><code class=\"hljs\">");

        // Transform all non-self-refernece links (does not start with "#") to target="_blank"
        let regex_links = RegExp::new("<a href=\"((?!#)[^\"]*)\">", "ig");
        let js_html = js_html.replace_by_pattern(&regex_links, "<a target=\"_blank\" href=\"$1\">");

        js_html.into()
    }

    // Only renders the content and spits out a cache object
    // can be used to display the page or to write to cache
    // Despite the signature, this function BLOCKS
    // async only comes from digesting via SubtleCrypto
    pub async fn render(post: &Post) -> PostContentCache {
        let parser = Parser::new_ext(&post.content, Options::all());
        // Apply code highlighting via Highlight.js
        let parser = Self::transform_code_block_highlight(parser);
        // Apply tag transform
        let parser = Self::transform_tags(parser);

        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        html_output = Self::transform_html(html_output);
        PostContentCache {
            uuid: post.uuid.clone(),
            version: CACHE_VERSION.to_owned(),
            orig_digest: crate::utils::sha1(&post.content).await,
            summary: match html_output.find(SUMMARY_DIVIDER) {
                None => html_output.clone(),
                Some(x) => (&html_output[0..x]).to_owned()
            },
            content: html_output
        }
    }

    // Tries to find the rendered content cache of post
    // if a valid cache cannot be found, this method
    // will render the content, write that into cache
    // and return this newly-rendered one
    // This will block if it tries to render; if that's a
    // concern, use find_by_post
    pub async fn find_or_render(post: &Post) -> PostContentCache {
        match Self::find_by_post(post).await {
            Some(cache) => cache,
            None => {
                let ret = Self::render(post).await;
                // Ignore save error since if save failed, it can be regenerated anyway
                let _ = ret.save().await;
                ret
            }
        }
    }

    // Save the current cache object to KV
    pub async fn save(&self) -> MyResult<()> {
        store::put_obj(&Self::uuid_to_cache_key(&self.uuid), self).await
    }

    pub async fn delete_by_uuid(uuid: &str) -> MyResult<()> {
        store::delete(&Self::uuid_to_cache_key(uuid)).await
    }
}