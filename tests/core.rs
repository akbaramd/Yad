use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use tempfile::tempdir;
use yad::{
    atomic, index,
    project::Project,
    schema,
    storage::{self, MemoryCreate},
};

fn init_project() -> (tempfile::TempDir, Project) {
    let dir = tempdir().expect("temp dir");
    fs::create_dir_all(dir.path().join(".git")).expect("git marker");
    let project = Project::init(dir.path(), Some("Test Project".to_string())).expect("init");
    (dir, project)
}

#[test]
fn init_uses_git_root_and_creates_team_owned_layout() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    let nested = dir.path().join("src").join("feature");
    fs::create_dir_all(&nested).unwrap();

    let project = Project::init(&nested, Some("Demo".to_string())).unwrap();

    assert_eq!(project.root, dir.path().canonicalize().unwrap());
    assert!(project.yad_dir.join("project.yaml").is_file());
    assert!(project.yad_dir.join("schemas.lock").is_file());
    assert!(
        project
            .yad_dir
            .join("schemas")
            .join("adr.schema.yaml")
            .is_file()
    );
    assert!(
        project
            .yad_dir
            .join("spaces")
            .join("general")
            .join("adr")
            .is_dir()
    );
    assert!(project.yad_dir.join("memory").is_dir());

    let nested_discovery = Project::discover(Some(&nested)).unwrap();
    assert_eq!(nested_discovery.config.id, project.config.id);
}

#[test]
fn init_refuses_to_overwrite_existing_yad_data() {
    let (_dir, project) = init_project();
    let error = Project::init(&project.root, None).unwrap_err().to_string();
    assert!(error.contains("already initialized"));
}

#[test]
fn remember_is_markdown_with_structured_frontmatter() {
    let (_dir, project) = init_project();

    let memory = storage::create_memory(
        &project,
        MemoryCreate {
            kind: "lesson".to_string(),
            text: "Increasing timeout did not solve the pool exhaustion issue.".to_string(),
            space: "integrations/aps".to_string(),
            subject: Some("APS timeout".to_string()),
            importance: 5,
            source: Some("agent".to_string()),
            tags: vec!["aps".to_string(), "timeout".to_string()],
            supersedes: None,
        },
    )
    .unwrap();

    assert!(memory.path.is_file());
    let raw = fs::read_to_string(&memory.path).unwrap();
    assert!(raw.starts_with("---\n"));
    assert!(raw.contains("schema: memory"));
    assert!(raw.contains("space: integrations/aps"));
    assert!(raw.contains("Increasing timeout"));

    let loaded = storage::find_memory(&project, &memory.meta.id).unwrap();
    assert_eq!(loaded.meta.kind, "lesson");
    assert_eq!(loaded.meta.importance, 5);
    assert_eq!(loaded.body, memory.body);
}

#[test]
fn adr_schema_rejects_placeholders_and_accepts_complete_record() {
    let (_dir, project) = init_project();
    let definition = schema::load(&project.schema_path("adr")).unwrap();

    let incomplete = storage::create_adr(
        &project,
        "Choose PostgreSQL",
        "architecture/persistence",
        None,
        None,
        None,
        vec![],
    )
    .unwrap();

    let report = schema::validate_adr(&incomplete.path, &definition).unwrap();
    assert!(!report.valid);
    assert_eq!(report.issues.len(), 3);

    let complete = storage::create_adr(
        &project,
        "Use Qdrant as a rebuildable semantic index",
        "architecture/search",
        Some("Agents need semantic retrieval across project memory and records."),
        Some("Use Qdrant only as a derived vector index; Markdown remains the source of truth."),
        Some("The vector index can be rebuilt after clone, pull, or loss."),
        vec!["qdrant".to_string()],
    )
    .unwrap();

    let report = schema::validate_adr(&complete.path, &definition).unwrap();
    assert!(report.valid, "{:?}", report.issues);
}

#[test]
fn adr_lifecycle_is_schema_enforced() {
    let (_dir, project) = init_project();
    let definition = schema::load(&project.schema_path("adr")).unwrap();

    schema::ensure_transition(&definition, "proposed", "accepted").unwrap();
    schema::ensure_transition(&definition, "accepted", "superseded").unwrap();
    assert!(schema::ensure_transition(&definition, "accepted", "proposed").is_err());
    assert!(schema::ensure_transition(&definition, "superseded", "accepted").is_err());
}

#[test]
fn fingerprint_changes_when_team_source_of_truth_changes() {
    let (_dir, project) = init_project();
    let before = index::fingerprint(&project).unwrap();

    storage::create_memory(
        &project,
        MemoryCreate {
            kind: "fact".to_string(),
            text: "Approved requests must not be re-evaluated.".to_string(),
            space: "facilities/approval".to_string(),
            subject: Some("FacilityWorker".to_string()),
            importance: 5,
            source: Some("test".to_string()),
            tags: vec![],
            supersedes: None,
        },
    )
    .unwrap();

    let after = index::fingerprint(&project).unwrap();
    assert_ne!(before, after);
}

#[test]
fn spaces_prevent_path_traversal() {
    let (_dir, project) = init_project();
    let err = project.ensure_space("../outside").unwrap_err().to_string();
    assert!(err.contains("invalid space"));
}

#[test]
fn source_files_are_portable_and_runtime_is_ignored() {
    let (_dir, project) = init_project();
    let ignore = fs::read_to_string(project.yad_dir.join(".gitignore")).unwrap();
    assert!(ignore.contains(".runtime/"));

    let readme = fs::read_to_string(project.yad_dir.join("README.md")).unwrap();
    assert!(readme.contains("source of truth"));
}

#[allow(dead_code)]
fn assert_exists(path: &Path) {
    assert!(path.exists(), "expected {} to exist", path.display());
}

#[test]
fn cloned_workspaces_get_distinct_local_vector_collections() {
    let (dir_one, project_one) = init_project();
    let collection_one = project_one.qdrant_collection_name().unwrap();

    let dir_two = tempdir().unwrap();
    fs::create_dir_all(dir_two.path().join(".git")).unwrap();
    fs::create_dir_all(dir_two.path().join(".yad").join("schemas")).unwrap();
    fs::create_dir_all(dir_two.path().join(".yad").join("memory")).unwrap();
    fs::create_dir_all(dir_two.path().join(".yad").join("spaces")).unwrap();

    fs::copy(
        project_one.yad_dir.join("project.yaml"),
        dir_two.path().join(".yad").join("project.yaml"),
    )
    .unwrap();
    fs::copy(
        project_one.yad_dir.join("schemas").join("adr.schema.yaml"),
        dir_two
            .path()
            .join(".yad")
            .join("schemas")
            .join("adr.schema.yaml"),
    )
    .unwrap();

    let project_two = Project::discover(Some(dir_two.path())).unwrap();
    let collection_two = project_two.qdrant_collection_name().unwrap();

    assert_eq!(project_one.config.id, project_two.config.id);
    assert_ne!(collection_one, collection_two);

    drop(dir_one);
}

#[test]
fn spaces_are_first_class_and_hierarchical() {
    let (_dir, project) = init_project();

    project
        .ensure_space_named("facilities/approval", Some("Facility Approval"))
        .unwrap();

    let parent = project.get_space("facilities").unwrap();
    let child = project.get_space("facilities/approval").unwrap();

    assert_eq!(parent.id, "facilities");
    assert_eq!(child.id, "facilities/approval");
    assert_eq!(child.name, "Facility Approval");

    let ids = project
        .list_spaces()
        .unwrap()
        .into_iter()
        .map(|space| space.id)
        .collect::<Vec<_>>();

    assert!(ids.contains(&"general".to_string()));
    assert!(ids.contains(&"facilities".to_string()));
    assert!(ids.contains(&"facilities/approval".to_string()));
}

#[test]
fn space_ids_are_portable_and_case_stable() {
    let (_dir, project) = init_project();

    assert!(project.ensure_space("Facilities/Approval").is_err());
    assert!(project.ensure_space("facilities/approval?").is_err());
    assert!(project.ensure_space("con").is_err());
    assert!(project.ensure_space("facilities\\approval").is_err());
}

#[test]
fn schema_lock_detects_tampering() {
    let (_dir, project) = init_project();

    let initial = schema::verify_lock(&project.yad_dir).unwrap();
    assert!(initial.valid);

    let schema_path = project.schema_path("adr");
    let mut raw = fs::read_to_string(&schema_path).unwrap();
    raw.push_str("\n# unauthorized schema drift\n");
    fs::write(&schema_path, raw).unwrap();

    let tampered = schema::verify_lock(&project.yad_dir).unwrap();
    assert!(!tampered.valid);
    assert_eq!(tampered.schemas.len(), 1);
    assert!(!tampered.schemas[0].valid);
}

#[test]
fn schema_lock_is_stable_across_crlf_checkouts() {
    let (_dir, project) = init_project();
    let schema_path = project.schema_path("adr");
    let raw = fs::read_to_string(&schema_path).unwrap();
    fs::write(&schema_path, raw.replace("\n", "\r\n")).unwrap();

    let report = schema::verify_lock(&project.yad_dir).unwrap();
    assert!(report.valid);
}

#[test]
fn project_write_lock_prevents_overlapping_mutations() {
    let (_dir, project) = init_project();
    let _first = project
        .acquire_write_lock(Duration::from_secs(1))
        .expect("first lock");

    let second = project.acquire_write_lock(Duration::from_millis(150));
    match second {
        Ok(_) => panic!("second writer unexpectedly acquired the project lock"),
        Err(err) => assert!(err.to_string().contains("project is busy")),
    }
}

#[test]
fn exact_active_memory_can_be_deduplicated_deterministically() {
    let (_dir, project) = init_project();
    let input = MemoryCreate {
        kind: "fact".to_string(),
        text: "  Approved requests   remain final.  ".to_string(),
        space: "facilities/approval".to_string(),
        subject: Some("Approval rule".to_string()),
        importance: 5,
        source: Some("agent-a".to_string()),
        tags: vec!["workflow".to_string()],
        supersedes: None,
    };

    let created = storage::create_memory(&project, input.clone()).unwrap();
    let duplicate = storage::find_exact_active_memory(
        &project,
        &MemoryCreate {
            text: "Approved requests remain final.".to_string(),
            source: Some("agent-b".to_string()),
            importance: 3,
            tags: vec![],
            ..input
        },
    )
    .unwrap()
    .expect("exact normalized duplicate");

    assert_eq!(created.meta.id, duplicate.meta.id);
}

#[test]
fn malformed_memory_is_not_silently_ignored() {
    let (_dir, project) = init_project();
    let dir = project.yad_dir.join("memory").join("2026").join("10");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("MEM-BROKEN.md"),
        "---\nschema: memory\nid: [\n---\nbroken",
    )
    .unwrap();

    let error = storage::list_memories(&project).unwrap_err().to_string();
    assert!(error.contains("invalid memory document"));
}

#[test]
fn existing_unindexed_documents_disable_incremental_freshness() {
    let (_dir, project) = init_project();
    assert!(index::can_incremental_update(&project).unwrap());

    storage::create_memory(
        &project,
        MemoryCreate {
            kind: "fact".to_string(),
            text: "Arrived from a simulated git pull.".to_string(),
            space: "general".to_string(),
            subject: Some("Pull probe".to_string()),
            importance: 3,
            source: Some("test".to_string()),
            tags: vec![],
            supersedes: None,
        },
    )
    .unwrap();

    assert!(!index::can_incremental_update(&project).unwrap());
}

#[test]
fn atomic_replacement_never_exposes_partial_content() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("state.md");
    let first = vec![b'A'; 64 * 1024];
    let second = vec![b'B'; 64 * 1024];
    atomic::write(&path, &first).unwrap();

    let done = Arc::new(AtomicBool::new(false));
    let writer_done = Arc::clone(&done);
    let writer_path = path.clone();
    let writer_first = first.clone();
    let writer_second = second.clone();

    let writer = thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            for index in 0..30 {
                let content = if index % 2 == 0 {
                    &writer_second
                } else {
                    &writer_first
                };
                atomic::write(&writer_path, content)?;
            }
            Ok(())
        })();

        writer_done.store(true, Ordering::Release);
        result
    });

    while !done.load(Ordering::Acquire) {
        let observed = fs::read(&path).unwrap();
        assert!(
            observed == first || observed == second,
            "reader observed a partial or mixed tracked file"
        );
        thread::sleep(Duration::from_millis(1));
    }

    writer.join().unwrap().unwrap();
    let final_value = fs::read(&path).unwrap();
    assert!(final_value == first || final_value == second);

    let temp_files = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(temp_files, 0);
}

#[test]
fn active_fact_decision_and_state_subjects_require_supersession() {
    let (_dir, project) = init_project();

    let original = MemoryCreate {
        kind: "decision".to_string(),
        text: "Retry count is 3.".to_string(),
        space: "runtime/policy".to_string(),
        subject: Some("Retry policy".to_string()),
        importance: 4,
        source: Some("agent-a".to_string()),
        tags: vec![],
        supersedes: None,
    };

    let created = storage::create_memory(&project, original.clone()).unwrap();

    let conflict = storage::find_active_subject_conflict(
        &project,
        &MemoryCreate {
            text: "Retry count is 5.".to_string(),
            source: Some("agent-b".to_string()),
            ..original
        },
    )
    .unwrap()
    .expect("conflicting active decision");

    assert_eq!(created.meta.id, conflict.meta.id);
}

#[test]
fn experiential_memory_can_repeat_subjects_without_truth_conflict() {
    let (_dir, project) = init_project();

    storage::create_memory(
        &project,
        MemoryCreate {
            kind: "lesson".to_string(),
            text: "First incident lesson.".to_string(),
            space: "runtime/incidents".to_string(),
            subject: Some("Timeout".to_string()),
            importance: 3,
            source: Some("agent-a".to_string()),
            tags: vec![],
            supersedes: None,
        },
    )
    .unwrap();

    let conflict = storage::find_active_subject_conflict(
        &project,
        &MemoryCreate {
            kind: "lesson".to_string(),
            text: "Second independent incident lesson.".to_string(),
            space: "runtime/incidents".to_string(),
            subject: Some("Timeout".to_string()),
            importance: 3,
            source: Some("agent-b".to_string()),
            tags: vec![],
            supersedes: None,
        },
    )
    .unwrap();

    assert!(conflict.is_none());
}
