Paprika
---

Yet another blog system that runs on Cloudflare Workers, and integrates with [Standard Notes](https://standardnotes.org/) (a self-hosted note-taking software) for a great editing experience, while giving you more freedom than the Listed service provided by Standard Notes.

This requires Cloudflare Workers KV for storage and thus needs the paid Unlimited plan to work. However, it is possible to swap out the storage and use something like S3 to use it 100% free on Workers (barring S3 costs).

As a practice, Paprika was written in Rust and compiled to WebAssembly for execution on Cloudflare Workers, using `wasm-bindgen` to interact with the JS environment. One single JS dependency, `highlight.js`, was used because there's simply no good alternative from the Rust ecosystem. `webpack` was used for an automatic, cached loading experience of the WebAssembly module (the official template for WebAssembly by Cloudflare is terrible because it tries to re-instantiate the module every time a request comes in; using `webpack` fixed the issue because it's much smarter), along with the ability to load `highlight.js` modularly.

__WARNING: I haven't even converted my own blog to Paprika yet.__

__WARNING: This project is neither complete nor tested. Use at your own risk. Always keep backups.__

Prerequisites
===

1. The `wrangler` cli tool from Cloudflare
2. node.js + npm
3. rustc + cargo, with the latest nightly toolchain
4. wasm-pack and wasm-opt

Deployment
===

1. Complete all configuration files according to the sections below (otherwise the project won't build)
2. Run `npm install --development` to install webpack and other node build dependencies.
3. Run `wrangler publish` to upload to Cloudflare Workers
4. Set up correct routes in Cloudflare control panel
5. Add your own instance of Paprika to your Standard Notes as a plugin (instructions available below)
6. Publish!

Note that all configuration and themes will be included statically in the final binary. To modify any of them, you will need to re-run `wrangler publish` to rebuild the entire program.

Configuration: wrangler.toml
===

The `wrangler.toml` is what instructs the `wrangler` tool to configure Cloudflare Workers correctly. An example for Paprika is listed below:

```toml
name = "paprika"
type = "webpack"
webpack_config = "webpack.config.js"
account_id = "<account_id>"
workers_dev = true
route = ""
zone_id = "<zone_id>"

kv-namespaces = [ 
         { binding = "PAPRIKA", id = "<kv_namespace_id>" } 
]
```

You need to replace everything within `<>`. The KV namespace can be created manually or via `wrangler`, but it must be binded in `wrangler.toml` with the name `PAPRIKA` like shown in the above example. Using any other name will not work.

Configuration: config.json
===

This is the main configuration file. The file will be compiled statically into the executable, and will be read in `build.rs`. Make sure you have the format correct and everything declared.

```json
{
  "secret": "<generate_some_random_secret>",
  "theme": "default",
  "title": "<title>",
  "lang": "en",
  "description": "<description>",
  "plugin_identifier": "com.example.change.this.to.whatever.you.like",
  "posts_per_page": 5,
  "redirects": {
    "/foo": "/bar",
    ...
  },
  "hljs": [
    "rust",
    "javascript",
    "bash",
    ...
  ]
}
```

`secret`: This will be the sole credential you can use to access the publishing endpoints (i.e. Standard Notes plugin endpoints). Generate something via `openssl rand` or whatever you think is secure.

`theme`: The name of the theme to use for your blog. Must be a subdirectory in `./theme`, and the default one is `default`. The selected theme will be compiled statically into the final `.wasm` binary. For more information on themes, continue reading this documentation.

`plugin_identifier`: Used in Standard Notes to distinguish plugins.

`redirects`: OPTIONAL. A map of URLs where the key will be mapped to the value by Paprika using 301 redirects. This is mainly useful for migration from another blogging platform.

`hljs`: An array of language support from `highlight.js` to be included in the final binary. The full `highlight.js` is notoriously huge and there's really no reason to include a bazillion languages you will never actually use in your blog posts. This will be read by `build.rs` to generate a JS shim that will load all languages in the array to the final binary via `webpack` support for `require`.

Configuration: theme_config.json
===

`theme_config.json` will be passed to Handlebars templates in the theme as `blog.theme_config` (See `BlogRootContext` and other `Context` structures in `src/render.rs`; `serde_json::Value` means arbitrary JSON object). A theme can thus use arbitrary extra information available via this configuration file. The `default` theme currently supports the following options:

```json
{
  "avatar_url": "<url_of_your_avatar>",
  "nav_links": [
    {
      "name": "<nav_name>",
      "url": "<nav_url>",
      "target": "_blank"
    },
    ...
  ]
}
```

`nav_links`: a set of navigation links to be displayed in the sidebar (or at the top on mobile). You can set `"target": "_blank"` to make the link open in new tabs, while omitting this attribute will make the link behave as a normal link, that is, open in the current page.

Installation in Standard Notes
===

After your blog is up and running, you can import the plugin to your Standard Notes account using the following URL:

```
https://<your_domain.com>/actions?secret=<your_secret>
```

The secret should be replaced with what you generated when creating `config.json`.

After the plugin is imported successfully, you can begin posting any of your notes from teh `Actions` menu of Standard Notes.

Post Format
===

You can use all valid Markdown syntax and some GitHub Flavoured Markdown supported by the `pulldown_cmark` crate. Read their documentation for what is actually supported.

In the post, you can insert `<!-- More -->` as a standalone paragraph to indicate that everything before this marker should be considered the summary, and should be displayed in place of the full text when viewing from the home page (post list). This does not affect the single post page and will not be displayed whatsoever.

By default, the timestamp and the URL of any new post will be generated automatically. You can override this behavior by inserting a fenced JSON code block at the very beginning of the post, followed by an empty line:

~~~
```json
{
    "url": "some-awesome-url",
    "timestamp": "YYYY-mm-dd",
    "unlist": true
}
```
~~~

`url`: Customize the URL of the post. Only the pathname part, and should __not__ include the starting `/`.

`timestamp`: Customize the displayed date of the post. This does not affect the order of posts on home page -- posts that were created later always take precedence, regardless of their timestamp. This is mainly useful when migrating old articles.

`unlist` / `unlisted`: when set to `true`, the post won't appear in home page, while still being accessible via its URL.

Normally, if such a customization header is not present, a post's metadata (URL and timestamp) will not be updated when you update a post. However, when this header is present, then the metadata will __always__ be updated.

When a post's `url` is changed, the old one will become an alias, 302-redirected to the new one.

Themes
===

A theme is a set of Handlebars templates and static assets located in a direct subdirectory of the `theme` folder. The structure should be as follows:

```
- theme/
  - <theme_name>/
    - static/
      - ....
    - home.hbs
    - post.hbs
    - ...
```

Everything inside the `static` folder will be available at `https://<your_domain>/static/`.

`home.hbs` will be used to render the home page (post list), while `post.hbs` will be used for single-post pages (i.e. the detail page). These templates can import other templates located in the same directory via the `{{> some_other_template.hbs }}` syntax.

The execution context of each template is defined in `src/render.rs`, as those `*Context` structs. Extra helpers are also defined in that file with the `handlebars_helper!` macros. Code there is pretty self-explanatory, please refer to the structs and the default theme for details on how to use the execution contexts.

The theme directory selected via `config.json` will be included into the final binary. Therefore, please make sure your assets are not too huge to fit in the 1MB binary limit of Cloudflare Worker.
