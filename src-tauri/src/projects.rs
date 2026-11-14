//! Project store: plain files under `Documents\Livery AI\<slug>\`.
//!
//! ```text
//! <slug>\project.json   metadata (schema `project/1`)
//! <slug>\brief.md       the Driver's brief
//! <slug>\revisions\     one `rev-N` folder per render
//! ```

use crate::error::{CommandError, CommandResult};
use crate::fsutil::{now_unix, rfc3339_utc, write_atomic};
use crate::settings::validate_customer_id;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECTS_FOLDER: &str = "Livery AI";
const PROJECT_FILE: &str = "project.json";
const BRIEF_FILE: &str = "brief.md";
const REVISIONS_DIR: &str = "revisions";
const SCHEMA: &str = "project/1";
const MAX_SLUG_LEN: usize = 64;
const MAX_NAME_LEN: usize = 80;
const MAX_BRIEF_LEN: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintType {
    /// `car_<id>.tga`
    #[default]
    Driver,
    /// `car_team_<id>.tga`
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarNumberMode {
    /// The sim stamps the number.
    #[default]
    Sim,
    /// Custom number paint.
    Custom,
}

/// What the UI sends to create a project.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInput {
    pub name: String,
    pub car_id: String,
    #[serde(default)]
    pub template_path: Option<String>,
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub paint_type: PaintType,
    #[serde(default)]
    pub car_number: CarNumberMode,
}

/// `project.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub schema: String,
    /// Folder name; also the project id.
    pub id: String,
    pub name: String,
    pub car_id: String,
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub paint_type: PaintType,
    #[serde(default)]
    pub car_number: CarNumberMode,
    /// Where the Driver's iRacing template lives. The template is never copied.
    #[serde(default)]
    pub template_path: Option<String>,
    /// RFC 3339 UTC.
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: Project,
    pub revision_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub project: Project,
    pub revision_count: u32,
    pub brief: String,
    /// Absolute project folder, for display.
    pub folder: String,
}

/// `Documents\Livery AI`.
pub fn default_root() -> CommandResult<PathBuf> {
    dirs::document_dir()
        .map(|d| d.join(PROJECTS_FOLDER))
        .ok_or_else(|| CommandError::new("no_documents", "Documents folder was not found."))
}

/// Windows device names that cannot be used as a folder name.
fn is_reserved_name(slug: &str) -> bool {
    let stem = slug.split('.').next().unwrap_or(slug);
    matches!(stem, "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit())
}

/// A project id is a single folder name: lowercase ASCII letters, digits and
/// hyphens, starting with a letter or digit. This is the path-traversal
/// guard: no separators, dots, drive letters or device names can pass.
pub fn validate_slug(slug: &str) -> CommandResult<()> {
    let bytes = slug.as_bytes();
    let ok = !bytes.is_empty()
        && bytes.len() <= MAX_SLUG_LEN
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && !is_reserved_name(slug);
    if !ok {
        return Err(CommandError::invalid("Project id is not valid."));
    }
    Ok(())
}

/// Derive a folder-safe slug from a display name. Always returns a slug that
/// passes [`validate_slug`].
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= 48 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "livery".to_string()
    } else if is_reserved_name(&slug) {
        format!("livery-{slug}")
    } else {
        slug
    }
}

fn validate_template_path(path: &str) -> CommandResult<()> {
    let p = Path::new(path);
    let is_psd = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("psd"));
    if !is_psd {
        return Err(CommandError::invalid("Template must be a .psd file."));
    }
    if !p.is_absolute() || !p.is_file() {
        return Err(CommandError::not_found("Template file was not found."));
    }
    Ok(())
}

fn count_revisions(project_dir: &Path) -> u32 {
    let Ok(entries) = fs::read_dir(project_dir.join(REVISIONS_DIR)) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_prefix("rev-"))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .count() as u32
}

fn read_project(project_dir: &Path) -> CommandResult<Project> {
    let text = fs::read_to_string(project_dir.join(PROJECT_FILE))
        .map_err(|e| CommandError::io("Read project.json", &e))?;
    serde_json::from_str(&text).map_err(|e| CommandError::data("Parse project.json", &e))
}

/// Create a project folder under `root`. The caller has already checked that
/// `car_id` names a car in the library.
pub fn create(root: &Path, input: ProjectInput) -> CommandResult<ProjectDetail> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(CommandError::invalid("Livery name is required."));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(CommandError::invalid(format!(
            "Livery name is limited to {MAX_NAME_LEN} characters."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(CommandError::invalid(
            "Livery name has unsupported characters.",
        ));
    }
    if input.car_id.is_empty() {
        return Err(CommandError::invalid("Car is required."));
    }
    let customer_id = input.customer_id.trim();
    validate_customer_id(customer_id)?;
    if input.brief.len() > MAX_BRIEF_LEN {
        return Err(CommandError::invalid("Brief is too long."));
    }
    let template_path = input
        .template_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    if let Some(path) = &template_path {
        validate_template_path(path)?;
    }

    fs::create_dir_all(root).map_err(|e| CommandError::io("Create projects folder", &e))?;

    // Claim a unique folder. `create_dir` fails if the folder exists, so two
    // projects can never share one.
    let base = slugify(name);
    let mut claimed = None;
    for n in 1..=999u32 {
        let slug = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        validate_slug(&slug)?;
        match fs::create_dir(root.join(&slug)) {
            Ok(()) => {
                claimed = Some(slug);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(CommandError::io("Create project folder", &e)),
        }
    }
    let slug = claimed
        .ok_or_else(|| CommandError::new("conflict", "Too many projects share this name."))?;
    let dir = root.join(&slug);

    let now = rfc3339_utc(now_unix());
    let project = Project {
        schema: SCHEMA.to_string(),
        id: slug,
        name: name.to_string(),
        car_id: input.car_id,
        customer_id: customer_id.to_string(),
        paint_type: input.paint_type,
        car_number: input.car_number,
        template_path,
        created_at: now.clone(),
        updated_at: now,
    };

    let written = (|| {
        fs::create_dir(dir.join(REVISIONS_DIR))
            .map_err(|e| CommandError::io("Create revisions folder", &e))?;
        write_atomic(&dir.join(BRIEF_FILE), input.brief.as_bytes())?;
        let json = serde_json::to_vec_pretty(&project)
            .map_err(|e| CommandError::data("Serialise project.json", &e))?;
        write_atomic(&dir.join(PROJECT_FILE), &json)
    })();
    if let Err(err) = written {
        // Do not leave a half-made project behind.
        let _ = fs::remove_dir_all(&dir);
        return Err(err);
    }

    Ok(ProjectDetail {
        project,
        revision_count: 0,
        brief: input.brief,
        folder: dir.to_string_lossy().into_owned(),
    })
}

/// Load one project by id.
pub fn get(root: &Path, id: &str) -> CommandResult<ProjectDetail> {
    validate_slug(id)?;
    let dir = root.join(id);
    if !dir.join(PROJECT_FILE).is_file() {
        return Err(CommandError::not_found("Project was not found."));
    }
    let mut project = read_project(&dir)?;
    // The folder name is the id, whatever the file says.
    project.id = id.to_string();
    let brief = fs::read_to_string(dir.join(BRIEF_FILE)).unwrap_or_default();
    Ok(ProjectDetail {
        revision_count: count_revisions(&dir),
        project,
        brief,
        folder: dir.to_string_lossy().into_owned(),
    })
}

/// All projects under `root`, most recently updated first. Folders that are
/// not valid projects are skipped. A missing root is an empty garage.
pub fn list(root: &Path) -> CommandResult<Vec<ProjectSummary>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(CommandError::io("Read projects folder", &e)),
    };
    let mut projects: Vec<ProjectSummary> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_string();
            validate_slug(&id).ok()?;
            let dir = entry.path();
            let mut project = read_project(&dir).ok()?;
            project.id = id;
            Some(ProjectSummary {
                revision_count: count_revisions(&dir),
                project,
            })
        })
        .collect();
    projects.sort_by(|a, b| {
        b.project
            .updated_at
            .cmp(&a.project.updated_at)
            .then_with(|| a.project.id.cmp(&b.project.id))
    });
    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            name: name.to_string(),
            car_id: "porsche992rgt3".to_string(),
            template_path: None,
            customer_id: "123456".to_string(),
            brief: "Apex Orange body.\nTelemetry Blue pinstripes.".to_string(),
            paint_type: PaintType::Driver,
            car_number: CarNumberMode::Sim,
        }
    }

    #[test]
    fn slugify_makes_folder_safe_names() {
        assert_eq!(slugify("Works broadcast livery"), "works-broadcast-livery");
        assert_eq!(slugify("  Halloween 2026!  "), "halloween-2026");
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify(r"C:\Windows\System32"), "c-windows-system32");
        assert_eq!(slugify("日本語"), "livery");
        assert_eq!(slugify("---"), "livery");
        assert_eq!(slugify("CON"), "livery-con");
        assert_eq!(slugify("com1"), "livery-com1");
        assert!(slugify(&"a".repeat(200)).len() <= 48);
        for name in ["x", "A b", "..", "NUL", "é", &"long name ".repeat(30)] {
            validate_slug(&slugify(name)).unwrap();
        }
    }

    #[test]
    fn validate_slug_blocks_traversal_and_odd_names() {
        for good in ["a", "works-broadcast-livery", "2026-halloween", "x-2"] {
            validate_slug(good).unwrap();
        }
        let long = "a".repeat(65);
        for bad in [
            "",
            ".",
            "..",
            "../x",
            "..\\x",
            "a/b",
            "a\\b",
            "/abs",
            "c:",
            "c:\\x",
            "a.b",
            "-lead",
            "Upper",
            "sp ace",
            "nul",
            "con",
            "com1",
            "lpt9",
            "a\0b",
            "ünï",
            long.as_str(),
        ] {
            assert_eq!(
                validate_slug(bad).unwrap_err().code,
                "invalid_input",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn create_writes_plain_files() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().join("Livery AI");
        let detail = create(&root, input("Works broadcast livery")).unwrap();
        assert_eq!(detail.project.id, "works-broadcast-livery");
        assert_eq!(detail.project.schema, "project/1");
        assert_eq!(detail.revision_count, 0);
        assert_eq!(detail.project.created_at, detail.project.updated_at);

        let dir = root.join("works-broadcast-livery");
        assert!(dir.join("revisions").is_dir());
        assert_eq!(
            fs::read_to_string(dir.join("brief.md")).unwrap(),
            "Apex Orange body.\nTelemetry Blue pinstripes."
        );
        let on_disk: Project =
            serde_json::from_str(&fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        assert_eq!(on_disk, detail.project);

        assert_eq!(get(&root, "works-broadcast-livery").unwrap(), detail);
    }

    #[test]
    fn create_gives_duplicate_names_their_own_folder() {
        let root = tempfile::tempdir().unwrap();
        let a = create(root.path(), input("GT3")).unwrap();
        let b = create(root.path(), input("GT3")).unwrap();
        let c = create(root.path(), input("gt3!")).unwrap();
        assert_eq!(a.project.id, "gt3");
        assert_eq!(b.project.id, "gt3-2");
        assert_eq!(c.project.id, "gt3-3");
    }

    #[test]
    fn create_rejects_bad_input_without_leaving_folders() {
        let root = tempfile::tempdir().unwrap();
        let cases = [
            ProjectInput {
                name: "   ".into(),
                ..input("x")
            },
            ProjectInput {
                name: "n".repeat(81),
                ..input("x")
            },
            ProjectInput {
                name: "line\nbreak".into(),
                ..input("x")
            },
            ProjectInput {
                car_id: String::new(),
                ..input("x")
            },
            ProjectInput {
                customer_id: "12ab".into(),
                ..input("x")
            },
            ProjectInput {
                template_path: Some("template.zip".into()),
                ..input("x")
            },
            ProjectInput {
                template_path: Some(
                    root.path()
                        .join("missing.psd")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..input("x")
            },
        ];
        for case in cases {
            assert!(create(root.path(), case.clone()).is_err(), "{case:?}");
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn create_stores_the_template_path_without_copying_the_template() {
        let root = tempfile::tempdir().unwrap();
        let templates = tempfile::tempdir().unwrap();
        let psd = templates.path().join("Template.PSD");
        fs::write(&psd, b"8BPS").unwrap();
        let detail = create(
            root.path(),
            ProjectInput {
                template_path: Some(psd.to_string_lossy().into_owned()),
                ..input("With template")
            },
        )
        .unwrap();
        assert_eq!(
            detail.project.template_path.as_deref(),
            Some(psd.to_string_lossy().as_ref())
        );
        let names: Vec<_> = fs::read_dir(root.path().join("with-template"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.to_lowercase().ends_with(".psd")));
    }

    #[test]
    fn get_rejects_traversal_and_reports_missing_projects() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("Livery AI");
        create(&root, input("Inside")).unwrap();
        // A project-shaped folder outside the root must not be reachable.
        fs::create_dir(outer.path().join("outside")).unwrap();
        fs::copy(
            root.join("inside").join("project.json"),
            outer.path().join("outside").join("project.json"),
        )
        .unwrap();

        for id in ["../outside", "..\\outside", "inside/../../outside", ".."] {
            assert_eq!(get(&root, id).unwrap_err().code, "invalid_input", "{id:?}");
        }
        assert_eq!(get(&root, "outside").unwrap_err().code, "not_found");
        assert_eq!(get(&root, "missing").unwrap_err().code, "not_found");
    }

    #[test]
    fn list_is_empty_without_a_root_and_skips_non_projects() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("Livery AI");
        assert_eq!(list(&root).unwrap(), Vec::new());

        create(&root, input("Alpha")).unwrap();
        create(&root, input("Bravo")).unwrap();
        fs::create_dir(root.join("not-a-project")).unwrap();
        fs::create_dir(root.join("Bad Name")).unwrap();
        fs::write(root.join("Bad Name").join("project.json"), "{}").unwrap();
        fs::create_dir(root.join("broken")).unwrap();
        fs::write(root.join("broken").join("project.json"), "{nope").unwrap();
        fs::write(root.join("notes.txt"), "x").unwrap();

        let mut ids: Vec<_> = list(&root)
            .unwrap()
            .into_iter()
            .map(|p| p.project.id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["alpha", "bravo"]);
    }

    #[test]
    fn list_sorts_newest_first_and_counts_revisions() {
        let root = tempfile::tempdir().unwrap();
        create(root.path(), input("Old")).unwrap();
        create(root.path(), input("New")).unwrap();

        // Age one project by rewriting its timestamp.
        let file = root.path().join("old").join("project.json");
        let mut old: Project = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        old.updated_at = "2020-01-01T00:00:00Z".to_string();
        fs::write(&file, serde_json::to_vec(&old).unwrap()).unwrap();

        let revs = root.path().join("new").join("revisions");
        fs::create_dir(revs.join("rev-1")).unwrap();
        fs::create_dir(revs.join("rev-2")).unwrap();
        fs::create_dir(revs.join("scratch")).unwrap();
        fs::write(revs.join("rev-3"), "a file, not a revision").unwrap();

        let projects = list(root.path()).unwrap();
        assert_eq!(projects[0].project.id, "new");
        assert_eq!(projects[0].revision_count, 2);
        assert_eq!(projects[1].project.id, "old");
        assert_eq!(projects[1].revision_count, 0);
    }

    #[test]
    fn the_folder_name_is_the_project_id() {
        let root = tempfile::tempdir().unwrap();
        create(root.path(), input("Original")).unwrap();
        fs::rename(root.path().join("original"), root.path().join("renamed")).unwrap();
        assert_eq!(get(root.path(), "renamed").unwrap().project.id, "renamed");
        assert_eq!(list(root.path()).unwrap()[0].project.id, "renamed");
    }
}
