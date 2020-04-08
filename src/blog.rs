// Utility functions and structs for the blogging system 
// Due to limitations of the Cloudflare Workers KV, we do not
// store the entire state in one record; instead, different
// parts are stroed in different records. This also increases
// efficiency, since the program won't need to load anything
// unnecessary from KV.
use crate::store;
use crate::utils::*;
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