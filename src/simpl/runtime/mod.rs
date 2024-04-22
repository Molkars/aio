use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_recursion::async_recursion;
use hashbrown::{HashMap, HashSet};
use crate::simpl::parser::{Import, SimplFile};
use crate::simpl::runtime::error::RuntimeError;
use crate::simpl::runtime::scope::Scope;
use crate::web::Context;

pub mod scope;
pub mod error;

pub struct Runtime {
    context: Arc<Context>,
    scope: Scope,
    processed_files: HashSet<FileId>,
}

impl Runtime {
    pub fn new(context: Arc<Context>) -> Self {
        Self {
            context,
            scope: Scope::default(),
            processed_files: Default::default(),
        }
    }
}

#[derive(Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct FileId(Arc<str>);

impl FileId {
    pub fn new(s: impl Into<Arc<str>>) -> Self {
        Self(s.into())
    }
}

impl From<&'_ Import> for FileId {
    fn from(import: &'_ Import) -> Self {
        use std::fmt::Write;
        let mut s = String::new();
        let f = &mut s;
        for (i, link) in import.path.iter().enumerate() {
            if i > 0 {
                write!(f, "/").unwrap();
            }
            write!(f, "{}", link).unwrap();
        }
        if import.path.is_empty() {
            write!(f, "/").unwrap();
        }
        write!(f, "{}", import.file).unwrap();
        FileId::new(s)
    }
}

impl Runtime {
    pub async fn run(&mut self, file: &SimplFile) -> Result<(), RuntimeError> {
        let file_id = FileId::new("<main>");
        self.process_file(file_id, file).await?;
        Ok(())
    }

    pub async fn process_file(&mut self, file_id: FileId, file: &SimplFile) -> Result<(), RuntimeError> {
        if self.processed_files.contains(&file_id) {
            return Ok(());
        }
        self.processed_files.insert(file_id.clone());

        self.resolve_imports(file.imports.as_slice()).await?;

        Ok(())
    }

    #[async_recursion]
    pub async fn resolve_imports(&mut self, imports: &[Import]) -> Result<(), RuntimeError> {
        let mut paths = HashMap::new();
        for (i, import) in imports.iter().enumerate() {
            paths.insert(self.resolve_import_path(import)?, i);
        }

        use futures::stream::{iter, StreamExt, TryStreamExt};

        let futures = paths.into_iter()
            .map(|(path, index)| async move {
                let file = Self::resolve_import(path.clone()).await?;
                Ok((path, index, file))
            });
        let imported_files = iter(futures)
            .buffer_unordered(10)
            .try_collect::<Vec<_>>().await?;

        for (_path, i, file) in imported_files {
            let import = &imports[i];
            let file_id = FileId::from(import);
            self.process_file(file_id, &file).await?;
        }

        Ok(())
    }

    async fn resolve_import(path: PathBuf) -> Result<SimplFile, RuntimeError> {
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| RuntimeError::ReadFileError {
                path: path.clone(),
                error: e,
            })?;

        let file = content.parse()
            .map_err(|e| RuntimeError::ParseFileError {
                path: path.clone(),
                error: e,
            })?;

        Ok(file)
    }

    pub fn resolve_import_path(&self, import: &Import) -> Result<PathBuf, RuntimeError> {
        let mut code_map = &self.context.shared_code;
        for link in &import.path {
            code_map = code_map.children
                .get(link.as_str())
                .ok_or_else(|| RuntimeError::UnknownImportPath { path: link.clone() })?;
        }
        let path = Path::new(&import.file);
        let name = path.file_name().unwrap().to_string_lossy();

        let file_path = code_map.files.get(name.as_ref())
            .ok_or_else(|| RuntimeError::UnknownImportPath {
                path: import.file.clone(),
            })?;

        Ok(file_path.clone())
    }
}