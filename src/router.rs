use crate::utils::MyResult;
use std::future::Future;
use std::pin::Pin;
use std::vec::Vec;
use web_sys::*;

// We have to box everything in order to make them fit in a Vec
type RouteHandler = Box<dyn Sync + Fn(Request, Url) -> Pin<Box<dyn Future<Output = MyResult<Response>>>>>;

// Convert a async function to RouteHandler
// both boxes the function and the returned Future
macro_rules! async_fn_boxed {
    ($f:ident) => {
        Box::new(move |req, url| Box::pin($f(req, url)))
    };
}

struct Route {
    path: String,
    handler: RouteHandler
}

pub struct Router {
    routes: Vec<Route>,
    default_handler: RouteHandler
}

impl Router {
    pub fn new<F, T>(default_handler: &'static F) -> Router
        where F: Sync + Fn(Request, Url) -> T,
            T: 'static + Future<Output = MyResult<Response>> {
        Router {
            routes: vec![],
            default_handler: async_fn_boxed!(default_handler)
        }
    }

    pub fn add_route<F, T>(
        &mut self, 
        path: &str,
        handler: &'static F
    ) where F: Sync + Fn(Request, Url) -> T,
            T: 'static + Future<Output = MyResult<Response>>
    {
        self.routes.push(Route {
            path: path.into(),
            handler: async_fn_boxed!(handler)
        });
    }

    pub async fn execute(&self, req: Request, url: Url) -> MyResult<Response> {
        for route in self.routes.iter() {
            // Routes added earlier overrides routes added later
            // e.g. if '/path/aaa' was added before '/path/', then
            //      calls to '/path/aaa' will not be dispatched to '/path/'
            // Routes ending with '/' are considered prefixes.
            if route.path.ends_with("/") {
                if url.pathname().starts_with(&route.path) {
                    return (route.handler)(req, url).await;
                }
            } else {
                if url.pathname() == route.path {
                    return (route.handler)(req, url).await;
                }
            }
        }

        return (self.default_handler)(req, url).await;
    }
}