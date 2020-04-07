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
        self.0.insert(0, uuid.into());
        store::put_obj_pretty("posts_list", self.0).await
    }
}