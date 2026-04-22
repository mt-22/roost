use super::*;
use tempfile::TempDir;

fn create_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, "test content").unwrap();
    path
}

fn create_dir(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

fn setup_profile(tmp: &TempDir, profile: &str) -> PathBuf {
    let roost = tmp.path().join("roost");
    let profile_dir = roost.join(profile);
    fs::create_dir_all(&profile_dir).unwrap();
    roost
}

#[test]
fn ingest_dir_moves_and_symlinks() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_dir(tmp.path(), "config/nvim");
    create_file(&origin, "init.lua");

    ingest(&origin, &profile_dir, "nvim", true).unwrap();

    assert!(origin.is_symlink());
    assert!(profile_dir.join("nvim").is_dir());
    assert!(profile_dir.join("nvim/init.lua").exists());
    assert_eq!(fs::read_link(&origin).unwrap(), profile_dir.join("nvim"));
}

#[test]
fn ingest_file_moves_to_misc() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), ".gitconfig");

    ingest(&origin, &profile_dir, "gitconfig", false).unwrap();

    assert!(origin.is_symlink());
    assert!(profile_dir.join("misc/gitconfig").exists());
}

#[test]
fn ingest_rejects_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let result = ingest(Path::new("/no/such/path"), &profile_dir, "app", true);
    assert!(result.is_err());
}

#[test]
fn ingest_rejects_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let real = create_file(tmp.path(), "real.txt");
    let link = tmp.path().join("link.txt");
    create_symlink(&real, &link, false).unwrap();

    let result = ingest(&link, &profile_dir, "app", false);
    assert!(result.is_err());
}

#[test]
fn restore_creates_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");

    let origin = tmp.path().join("config/nvim");
    restore(&origin, &profile_dir, "nvim", true).unwrap();

    assert!(origin.is_symlink());
    assert_eq!(fs::read_link(&origin).unwrap(), profile_dir.join("nvim"));
}

#[test]
fn restore_skips_if_already_linked() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    let origin = tmp.path().join("config/nvim");
    fs::create_dir_all(origin.parent().unwrap()).unwrap();
    create_symlink(&profile_dir.join("nvim"), &origin, true).unwrap();

    restore(&origin, &profile_dir, "nvim", true).unwrap();
}

#[test]
fn restore_rejects_real_file() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    create_dir(&profile_dir, "nvim");
    let origin = create_file(tmp.path(), "config/nvim");

    let result = restore(&origin, &profile_dir, "nvim", true);
    assert!(result.is_err());
}

#[test]
fn unlink_removes_symlink_and_restores_files() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_dir(tmp.path(), "config/nvim");
    create_file(&origin, "init.lua");

    ingest(&origin, &profile_dir, "nvim", true).unwrap();
    assert!(origin.is_symlink());

    unlink(&origin, &profile_dir, "nvim", true).unwrap();

    assert!(!origin.is_symlink());
    assert!(origin.join("init.lua").exists());
    assert!(!profile_dir.join("nvim").exists());
}

#[test]
fn unlink_rejects_non_symlink() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "laptop");
    let profile_dir = roost.join("laptop");

    let origin = create_file(tmp.path(), "real.txt");
    let result = unlink(&origin, &profile_dir, "app", false);
    assert!(result.is_err());
}

#[test]
fn import_from_creates_chain() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    create_dir(&roost.join("shared"), "nvim");

    import_from("nvim", "shared", "laptop", &roost).unwrap();

    let target = roost.join("laptop/nvim");
    assert!(target.is_symlink());
    assert_eq!(fs::read_link(&target).unwrap(), roost.join("shared/nvim"));
}

#[test]
fn import_from_rejects_missing_source() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    let result = import_from("nvim", "shared", "laptop", &roost);
    assert!(result.is_err());
}

#[test]
fn copy_to_creates_independent_copy() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    let nvim = create_dir(&roost.join("shared"), "nvim");
    create_file(&nvim, "init.lua");

    copy_to("nvim", "shared", "laptop", &roost).unwrap();

    let target = roost.join("laptop/nvim");
    assert!(target.is_dir());
    assert!(target.join("init.lua").exists());
    assert!(!target.is_symlink());
}

#[test]
fn copy_to_rejects_existing_target() {
    let tmp = TempDir::new().unwrap();
    let roost = setup_profile(&tmp, "shared");
    let _ = setup_profile(&tmp, "laptop");

    create_dir(&roost.join("shared"), "nvim");
    create_dir(&roost.join("laptop"), "nvim");

    let result = copy_to("nvim", "shared", "laptop", &roost);
    assert!(result.is_err());
}

#[test]
fn app_dest_dir_vs_file() {
    assert_eq!(
        app_dest(Path::new("/roost"), "nvim", true),
        PathBuf::from("/roost/nvim")
    );
    assert_eq!(
        app_dest(Path::new("/roost"), "gitconfig", false),
        PathBuf::from("/roost/misc/gitconfig")
    );
}
