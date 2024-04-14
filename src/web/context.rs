use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use crate::config;
use crate::config::error::FromConfigError;

#[derive(Debug)]
pub struct Context {
    pub port: u16,
    pub serve_dir: PathBuf,
    pub route_map: RouteMap,
}

impl Context {
    pub fn from_config(config: &config::Config) -> Result<Self, FromConfigError> {
        let web_config = config.get_section("web")?;
        let port = web_config.get_int("port")?;
        let serve_dir = web_config.get_path("serve")?;
        let serve_dir = config.root.join(serve_dir).canonicalize()?;

        let mut route_map = RouteMap::default();
        readdir(serve_dir.as_path(), &mut route_map)?;

        Ok(Context {
            port,
            serve_dir,
            route_map,
        })
    }
}


fn readdir(path: &Path, map: &mut RouteMap) -> Result<(), FromConfigError> {
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
            readdir(&entry.path(), &mut inner)?;
            map.embedded.insert(name, inner);
        }
    }
    Ok(())
}

#[derive(Default, Debug)]
pub struct RouteMap {
    pub embedded: BTreeMap<String, RouteMap>,
    pub handlers: BTreeMap<String, PathBuf>,
}
