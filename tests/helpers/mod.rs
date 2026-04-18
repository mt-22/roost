#![allow(dead_code)]
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

pub struct TestRoost {
    pub tmp_dir: tempfile::TempDir,
    pub home_dir: PathBuf,
    pub roost_dir: PathBuf,
    pub roost_config: PathBuf,
    pub local_config: PathBuf,
}

impl TestRoost {
    pub fn new() -> Self {
        let tmp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let home_dir = tmp_dir.path().join("home");
        let config_dir = home_dir.join(".config");
        let roost_dir = home_dir.join(".roost");
        fs::create_dir_all(&home_dir).expect("failed to create home dir");
        fs::create_dir_all(&config_dir).expect("failed to create .config dir");
        fs::create_dir_all(&roost_dir).expect("failed to create .roost dir");

        let roost_config = roost_dir.join("roost.toml");
        let local_config = roost_dir.join("local.toml");

        Self {
            tmp_dir,
            home_dir,
            roost_dir,
            roost_config,
            local_config,
        }
    }

    pub fn init_minimal(&self) {
        let shared = roost::app::SharedAppConfig {
            remote: None,
            profiles: HashMap::from([(
                "default".to_string(),
                roost::app::Profile {
                    apps: HashSet::new(),
                    app_sources: HashMap::new(),
                },
            )]),
            apps: HashMap::new(),
            ignored: HashSet::new(),
        };
        let local = roost::app::LocalAppConfig {
            active_profile: "default".to_string(),
            os_info: roost::os_detect::OsInfo {
                family: "unix".to_string(),
                name: "test".to_string(),
                version: Some("1.0.0".to_string()),
                arch: std::env::consts::ARCH.to_string(),
            },
            link_paths: HashMap::new(),
        };
        shared
            .save(&self.roost_config)
            .expect("failed to save shared config");
        local
            .save(&self.local_config)
            .expect("failed to save local config");
    }

    pub fn init_git(&self) {
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&self.roost_dir)
            .output()
            .expect("failed to init git repo");
        std::process::Command::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(&self.roost_dir)
            .output()
            .expect("failed to set git user.name");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&self.roost_dir)
            .output()
            .expect("failed to set git user.email");
        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.roost_dir)
            .output()
            .expect("failed to git add");
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&self.roost_dir)
            .output()
            .expect("failed to git commit");
    }

    pub fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("roost").unwrap();
        cmd.env("ROOST_DIR", &self.roost_dir);
        cmd.env("HOME", &self.home_dir);
        cmd
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.home_dir.join(relative)
    }
}
