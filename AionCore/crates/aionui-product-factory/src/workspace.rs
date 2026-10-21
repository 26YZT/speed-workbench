//! Filesystem authorization for authenticated Product Factory workspaces.
//! This controls which directories the service may use; it is not a process sandbox.
use std::path::Path;

use aionui_db::ProductFactoryRunRow;
use sha2::{Digest, Sha256};

use crate::{ProductFactoryError, ProductFactoryService};

impl ProductFactoryService {
    /// Remote callers receive server-allocated directories and cannot nominate
    /// an arbitrary host path. Local desktop mode keeps its explicit picker.
    pub fn with_user_workspace_isolation(mut self) -> Self {
        self.isolate_user_workspaces = true;
        self
    }

    pub(crate) fn user_relative_path(&self, user: &str, relative: &str) -> String {
        if self.isolate_user_workspaces {
            // Hash the full identity: stripping prefixes or normalizing case
            // would let distinct identities share a filesystem namespace.
            let mut hash = Sha256::new();
            hash.update(b"product-factory-workspace-user-v1\0");
            hash.update(user.as_bytes());
            format!("{:x}/{relative}", hash.finalize())
        } else {
            relative.to_owned()
        }
    }

    pub(crate) fn authorize_workspace(&self, run: &ProductFactoryRunRow) -> Result<(), ProductFactoryError> {
        if !self.isolate_user_workspaces {
            return Ok(());
        }
        let root = self.managed_workspace_root.as_ref().ok_or_else(workspace_blocked)?;
        let relative = self.user_relative_path(&run.user_id, &run.id);
        crate::versions::validate_workspace_relative(&relative).map_err(|_| workspace_blocked())?;
        let expected = root.join(relative);
        if Path::new(&run.workspace_path) != expected {
            return Err(workspace_blocked());
        }
        // Existing records are checked too. Never canonicalize a client path
        // into another user's namespace, and reject symlinked ancestors.
        crate::versions::validate_managed_workspace(&expected).map_err(|_| workspace_blocked())
    }
}

pub(crate) fn workspace_blocked() -> ProductFactoryError {
    ProductFactoryError::InvalidRequest("workspace is outside the user's managed directory or unavailable".into())
}
