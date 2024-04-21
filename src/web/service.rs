use std::collections::LinkedList;
use std::convert::Infallible;
use std::io::{stdout, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use hashbrown::HashMap;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use crate::simpl::parser::SimplFile;
use crate::web::Context;

use crate::web::context::RouteMap;

pub struct Service {
    address: SocketAddr,
    router: Arc<Router>,
}

#[derive(Default, Debug)]
struct Router {
    inner: RouteLink,
}

#[derive(Default, Debug)]
struct RouteLink {
    handler: Option<Handler>,
    children: HashMap<String, RouteLink>,
}

#[derive(Debug, Default)]
struct Handler {
    methods: HashMap<String, PathBuf>,
}

impl Router {
    fn new() -> Self {
        Self::default()
    }
}

impl Service {
    pub fn try_new(context: &Context) -> anyhow::Result<Self> {
        let mut router = Router::new();
        Self::build_router(&mut router.inner, &context.route_map)?;

        Ok(Self {
            address: context.address.clone(),
            router: Arc::new(router),
        })
    }

    fn build_router(link: &mut RouteLink, map: &RouteMap) -> anyhow::Result<()> {
        if !map.handlers.is_empty() {
            let mut handler = Handler::default();
            for (method, path) in &map.handlers {
                handler.methods.insert(method.clone(), path.clone());
            }
            link.handler = Some(handler);
        }
        for (name, map) in &map.embedded {
            let mut inner = RouteLink::default();
            Self::build_router(&mut inner, map)?;
            link.children.insert(name.clone(), inner);
        }
        Ok(())
    }

    pub fn run(self) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async move {
            let router = self.router.clone();

            let addr = self.address;
            let listener = TcpListener::bind(&addr).await?;
            println!("listening on {}", addr);
            Self::print_router(router.as_ref())?;

            loop {
                let connection_result: tokio::io::Result<(TcpStream, SocketAddr)> = select! {
                    _ = tokio::signal::ctrl_c() => break,
                    r = listener.accept() => r,
                };
                let (stream, _addr) = match connection_result {
                    Ok((stream, addr)) => (stream, addr),
                    Err(e) => {
                        eprintln!("connection error: {e}");
                        continue;
                    }
                };

                let http = http1::Builder::new();
                let http = Arc::new(http);

                let io = TokioIo::new(stream);
                let routes = router.clone();
                tokio::spawn(async move {
                    let handler_result = http.serve_connection(io, service_fn(move |req| service(req, routes.clone()))).await;

                    match handler_result {
                        Ok(()) => {}
                        Err(e) => {
                            eprintln!("handler error: {e}");
                        }
                    }
                });
            };

            eprintln!("shutting down...");
            Result::<_, anyhow::Error>::Ok(())
        })?;

        rt.shutdown_background();
        println!("shutdown gracefully");
        Ok(())
    }

    fn print_router(router: &Router) -> std::io::Result<()> {
        let mut out = stdout();
        writeln!(&mut out, "routes:")?;
        Self::print_route_link(&mut out, &mut LinkedList::new(), &router.inner)?;
        Ok(())
    }

    fn print_route_link<'a>(f: &mut impl Write, prefix: &mut LinkedList<&'a str>, link: &'a RouteLink) -> std::io::Result<()> {
        if let Some(handler) = &link.handler {
            // get, head.simp, post, put, delete, connect, options, trace, patch
            write!(f, "  ")?;
            write!(f, "{}", if handler.methods.contains_key("GET") { 'G' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("HEAD") { 'H' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("POST") { 'P' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("PUT") { 'P' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("DELETE") { 'D' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("CONNECT") { 'C' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("OPTIONS") { 'O' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("TRACE") { 'T' } else { '-' })?;
            write!(f, "{}", if handler.methods.contains_key("PATCH") { 'P' } else { '-' })?;
            write!(f, " ")?;
            for part in &*prefix {
                write!(f, "/{part}")?;
            }
            writeln!(f, "/")?;
        }
        for (name, link) in &link.children {
            prefix.push_back(name.as_str());
            Self::print_route_link(f, &mut *prefix, link)?;
            prefix.pop_back();
        }

        Ok(())
    }
}
async fn service(
    req: Request<hyper::body::Incoming>,
    routes: Arc<Router>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let path = path.strip_prefix('/').unwrap_or(path);

    let handler = if path.is_empty() {
        routes.inner.handler.as_ref()
    } else {
        let mut route_link = &routes.inner;
        for link in path.split('/') {
            let Some(next_link) = route_link.children.get(link) else {
                return Ok(not_found());
            };
            route_link = next_link;
        }
        route_link.handler.as_ref()
    };

    let Some(handler) = handler else {
        return Ok(not_found());
    };

    let Some(path) = handler.methods.get(req.method().as_str()) else {
        return Ok(not_found());
    };

    let (sc, content) = match parse_file(path).await {
        Ok(file) => {
            (StatusCode::OK, format!("{:#?}", file))
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, if cfg!(debug_assertions) {
                format!("error: {e}")
            } else {
                format!("an error occurred.")
            })
        }
    };
    let mut res = Response::new(Full::new(Bytes::from(content)));
    *res.status_mut() = sc;
    return Ok(res);
}

async fn parse_file(path: &Path) -> anyhow::Result<SimplFile> {
    let contents = tokio::fs::read_to_string(path).await?;
    let file: SimplFile = contents.parse()?;
    Ok(file)
}

#[inline]
fn not_found() -> Response<Full<Bytes>> {
    let mut res = Response::new(Full::new(Bytes::new()));
    *res.status_mut() = StatusCode::NOT_FOUND;
    res
}