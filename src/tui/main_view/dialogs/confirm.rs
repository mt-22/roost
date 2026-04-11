use std::path::PathBuf;

pub enum ConfirmKind {
    SetPrimary {
        file_path: PathBuf,
        app_name: String,
    },
    RemoveApp {
        app_name: String,
    },
}

pub struct ConfirmState {
    pub kind: ConfirmKind,
}

impl ConfirmState {
    pub fn set_primary(file_path: PathBuf, app_name: String) -> Self {
        Self {
            kind: ConfirmKind::SetPrimary {
                file_path,
                app_name,
            },
        }
    }

    pub fn remove_app(app_name: String) -> Self {
        Self {
            kind: ConfirmKind::RemoveApp { app_name },
        }
    }

    pub fn app_name(&self) -> &str {
        match &self.kind {
            ConfirmKind::SetPrimary { app_name, .. } => app_name,
            ConfirmKind::RemoveApp { app_name } => app_name,
        }
    }

    pub fn accept(
        &mut self,
        config: &mut crate::app::SharedAppConfig,
        config_path: &std::path::Path,
    ) -> color_eyre::Result<bool> {
        if let ConfirmKind::SetPrimary {
            file_path,
            app_name,
        } = &self.kind
        {
            let Some(app) = config.apps.get_mut(app_name) else {
                return Ok(false);
            };
            app.primary_config = Some(file_path.clone());
            config.save(config_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
