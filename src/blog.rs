// Utility functions and structs for the blogging system 
// Due to limitations of the Cloudflare Workers KV, we do not
// store the entire state in one record; instead, different
// parts are stroed in different records. This also increases
// efficiency, since the program won't need to load anything
// unnecessary from KV.
use crate::store;
use crate::utils::*;
use pulldown_cmark::*;
use serde::{Serialize, Deserialize};
use std::vec::Vec;

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
    pub content: String
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
}

// This should be bumped each time the parsing / compiling
// logic of Markdown changes, and each time the Markdown
// library updates. Updaing this value invalidates all
// existing cache and they will be recompiled when someone
// visits.
const CACHE_VERSION: &'static str = "0001";

// Cached version of rendered blog content HTMLs
// compiled from Markdown
#[derive(Serialize, Deserialize)]
pub struct PostContentCache {
    // UUID of the original post
    uuid: String,
    // If version != CACHE_VERSION, the cache is invalidated
    version: String,
    // Digest of the original content
    orig_digest: String,
    // Compiled content in HTML
    pub content: String
}

impl PostContentCache {
    fn uuid_to_cache_key(uuid: &str) -> String {
        format!("content_cache_{}", uuid)
    }

    async fn find_by_uuid(uuid: &str) -> MyResult<PostContentCache> {
        store::get_obj(&Self::uuid_to_cache_key(uuid)).await
    }

    pub async fn find_by_post(post: &Post) -> Option<PostContentCache> {
        let cache = match Self::find_by_uuid(&post.uuid).await {
            Ok(cache) => cache,
            Err(_) => return None
        };

        if cache.version != CACHE_VERSION {
            return None;
        }

        if cache.orig_digest != crate::utils::sha1(&post.content).await {
            return None;
        }

        Some(cache)
    }

    // Only renders the content and spits out a cache object
    // can be used to display the page or to write to cache
    // Despite the signature, this function BLOCKS
    // async only comes from digesting via SubtleCrypto
    pub async fn render(post: &Post) -> PostContentCache {
        // TODO: enable some options; pre-process posts to enable
        //       inline image caching; also generate a summary (?)
        //       from first few paragraphs
        let parser = Parser::new(&post.content);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        PostContentCache {
            uuid: post.uuid.clone(),
            version: CACHE_VERSION.to_owned(),
            orig_digest: crate::utils::sha1(&post.content).await,
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
}