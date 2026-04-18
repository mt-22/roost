use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::fs;

// Mocking necessary parts of the app module
mod app {
    use super::*;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Application {
        pub name: String,
        pub on_profiles: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Profile {
        pub apps: HashSet<String>,
        pub app_sources: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SharedAppConfig {
        pub remote: Option<String>,
        pub profiles: HashMap<String, Profile>,
        pub apps: HashMap<String, Application>,
        pub ignored: HashSet<String>,
    }

    impl SharedAppConfig {
        pub fn save(&self, path: &std::path::Path) {
            let content = toml::to_string(self).unwrap();
            fs::write(path, content).unwrap();
        }
        pub fn load(path: &std::path::Path) -> Self {
            let content = fs::read_to_string(path).unwrap();
            toml::from_str(&content).unwrap()
        }
    }
}

fn main() {
    let config_path = PathBuf::from("repro_roost.toml");

    let mut apps = HashMap::new();
    apps.insert("app1".to_string(), app::Application {
        name: "app1".to_string(),
        on_profiles: vec!["default".to_string()],
    });

    let mut profiles = HashMap::new();
    profiles.insert("default".to_string(), app::Profile {
        apps: vec!["app1".to_string()].into_iter().collect(),
        app_sources: HashMap::new(),
    });
    profiles.insert("new_prof".to_string(), app::Profile {
        apps: HashSet::new(),
        app_sources: HashMap::new(),
    });

    let mut config = app::SharedAppConfig {
        remote: None,
        profiles,
        apps,
        ignored: HashSet::new(),
    };

    println!("Initial config: {:?}", config);
    config.save(&config_path);

    // Simulate import_app_from_profile
    let app_name = "app1";
    let to_profile = "new_prof";
    let source_profile = "default";

    {
        let to_prof = config.profiles.get_mut(to_profile).unwrap();
        to_prof.apps.insert(app_name.to_string());
        to_prof.app_sources.insert(app_name.to_string(), source_profile.to_string());
    }
    if let Some(app) = config.apps.get_mut(app_name) {
        if !app.on_profiles.contains(&to_profile.to_string()) {
            app.on_profiles.push(to_profile.to_string());
        }
    }

    println!("Config after import: {:?}", config);
    config.save(&config_path);

    let reloaded = app::SharedAppConfig::load(&config_path);
    println!("Reloaded config: {:?}", reloaded);

    assert!(reloaded.apps.get("app1").unwrap().on_profiles.contains(&"default".to_string()));
    assert!(reloaded.apps.get("app1").unwrap().on_profiles.contains(&"new_prof".to_string()));
    assert!(reloaded.profiles.get("default").unwrap().apps.contains("app1"));
    assert!(reloaded.profiles.get("new_prof").unwrap().apps.contains("app1"));

    println!("Reproduction successful (no bug found in basic logic).");
}
