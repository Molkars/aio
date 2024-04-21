use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use crate::config;
use crate::config::error::FromConfigError;

#[derive(Debug)]
pub struct Context {
    pub address: SocketAddr,
    pub serve_dir: PathBuf,
    pub route_map: RouteMap,
    pub shared_code: CodeMap,
}

impl Context {
    pub fn from_config(config: &config::Config) -> Result<Self, FromConfigError> {
        let web_config = config.get_section("web")?;

        let host = web_config.get_string("host")?;
        let addr: IpAddr = host.parse()
            .map_err(FromConfigError::custom)?;
        let port = web_config.get_int("port")?;
        let address = SocketAddr::from((addr, port));

        let serve_dir = web_config.get_path("serve")?;
        let serve_dir = config.root.join(serve_dir).canonicalize()?;

        let mut route_map = RouteMap::default();
        build_route_map(serve_dir.as_path(), &mut route_map)?;

        let code_dir = web_config.get_path("path")?;
        let code_dir = config.root.join(code_dir).canonicalize()?;

        let mut shared_code = CodeMap::default();
        build_code_map(code_dir.as_path(), &mut shared_code)?;

        Ok(Context {
            address,
            serve_dir,
            route_map,
            shared_code,
        })
    }
}

#[derive(Default, Debug)]
pub struct RouteMap {
    pub embedded: BTreeMap<String, RouteMap>,
    pub handlers: BTreeMap<String, PathBuf>,
}

fn build_route_map(path: &Path, map: &mut RouteMap) -> Result<(), FromConfigError> {
    for entry in path.read_dir()? {
        let entry = entry?;

        let meta = entry.metadata()?;
        let name = entry.file_name().into_string().unwrap();
        if meta.is_file() {
            let path = entry.path();
            let Some(extension) = path.extension() else {
                continue;
            };

            let extension = extension.to_str().unwrap();
            if extension != "simp" {
                continue;
            }

            let name = name.strip_suffix(extension).unwrap();
            let name = name.strip_suffix('.').unwrap();
            map.handlers.insert(name.to_owned(), path);
        } else {
            let mut inner = RouteMap::default();
            build_route_map(&entry.path(), &mut inner)?;
            map.embedded.insert(name, inner);
        }
    }
    Ok(())
}

#[derive(Default, Debug)]
pub struct CodeMap {
    files: BTreeMap<String, PathBuf>,
    children: BTreeMap<String, CodeMap>,
}

fn build_code_map(path: &Path, map: &mut CodeMap) -> std::io::Result<()> {
    for entry in path.read_dir()? {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            let mut child = CodeMap::default();
            build_code_map(&entry.path(), &mut child)?;
            map.children.insert(name, child);
        } else {
            map.files.insert(name, entry.path());
        }
    }
    Ok(())
}

