use std::{collections::BTreeMap, fs, time::Duration};

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use serde_json::json;

use crate::{
    cli::{
        AdrCommand, Cli, Command, IndexCommand, InfraCommand, MemoryCommand, ModelCommand,
        RecordCommand, SchemaCommand, SpaceCommand,
    },
    index, infra, model_cache,
    project::Project,
    qdrant::QdrantStore,
    schema, search,
    storage::{self, MemoryCreate, RecordCreate},
};

pub fn execute(cli: Cli) -> Result<()> {
    let json_output = cli.json;

    match cli.command {
        Command::Init(args) => handle_init(args.name, json_output),
        Command::Status => handle_status(json_output),
        Command::Sync => handle_sync(json_output),
        Command::Remember(args) => handle_remember(args, json_output),
        Command::Search(args) => handle_search(args, json_output),
        Command::Memory { command } => handle_memory(command, json_output),
        Command::Adr { command } => handle_adr(command, json_output),
        Command::Record { command } => handle_record(command, json_output),
        Command::Validate => handle_validate(json_output),
        Command::Index { command } => handle_index(command, json_output),
        Command::Infra { command } => handle_infra(command, json_output),
        Command::Model { command } => handle_model(command, json_output),
        Command::Space { command } => handle_space(command, json_output),
        Command::Doctor => handle_doctor(json_output),
        Command::Schema { command } => handle_schema(command, json_output),
    }
}

fn handle_init(name: Option<String>, json_output: bool) -> Result<()> {
    let root = std::env::current_dir().context("failed to resolve current directory")?;
    let project = Project::init(&root, name)?;

    if json_output {
        print_json(&json!({
            "project": project.config.name,
            "project_id": project.config.id,
            "root": project.root,
            "yad_dir": project.yad_dir,
            "embedding_model": project.config.embedding.model,
            "min_semantic_score": project.config.search.min_semantic_score,
            "qdrant_url": project.config.qdrant.url
        }))?;
    } else {
        println!("Initialized Yad for '{}'.", project.config.name);
        println!("Project ID: {}", project.config.id);
        println!("Data: {}", project.yad_dir.display());
        println!("Next: yad infra up");
        println!("Then: yad sync");
    }

    Ok(())
}

fn handle_status(json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let memories = storage::list_memories(&project)?;
    let records = storage::list_records(&project)?;
    let schemas = schema::list_definitions(&project.yad_dir)?;
    let spaces = project.list_spaces()?;
    let active_memories = memories
        .iter()
        .filter(|memory| memory.meta.status == "active")
        .count();
    let accepted_adrs = records
        .iter()
        .filter(|record| record.meta.schema == "adr" && record.meta.status == "accepted")
        .count();

    let store = QdrantStore::for_project(&project)?;
    let qdrant_available = store.is_available();
    let collection_exists = if qdrant_available {
        store.collection_exists().unwrap_or(false)
    } else {
        false
    };
    let index_status = index::status(&project)?;

    if json_output {
        print_json(&json!({
            "project": project.config.name,
            "project_id": project.config.id,
            "root": project.root,
            "spaces": spaces.len(),
            "memories": {
                "total": memories.len(),
                "active": active_memories
            },
            "records": {
                "total": records.len()
            },
            "schemas": schemas.len(),
            "adrs": {
                "total": records.iter().filter(|record| record.meta.schema == "adr").count(),
                "accepted": accepted_adrs
            },
            "search": {
                "min_semantic_score": project.config.search.min_semantic_score
            },
            "qdrant": {
                "available": qdrant_available,
                "collection_exists": collection_exists,
                "collection": store.collection_name()
            },
            "index": index_status
        }))?;
    } else {
        println!("Project: {}", project.config.name);
        println!("ID: {}", project.config.id);
        println!("Root: {}", project.root.display());
        println!("Spaces: {}", spaces.len());
        println!(
            "Memory: {} total / {} active",
            memories.len(),
            active_memories
        );
        println!("Records: {}", records.len());
        println!("Schemas: {}", schemas.len());
        println!(
            "ADR: {} total / {} accepted",
            records.iter().filter(|record| record.meta.schema == "adr").count(),
            accepted_adrs
        );
        println!(
            "Qdrant: {} / collection {}",
            if qdrant_available {
                "online"
            } else {
                "offline"
            },
            if collection_exists {
                "ready"
            } else {
                "missing"
            }
        );
        println!(
            "Index: {}",
            if index_status.stale {
                "stale"
            } else {
                "current"
            }
        );
    }

    Ok(())
}

fn handle_sync(json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let _write_guard = project.acquire_write_lock(write_lock_timeout())?;

    let schema_lock = schema::verify_lock(&project.yad_dir)?;
    if !schema_lock.valid {
        if json_output {
            print_json(&serde_json::to_value(&schema_lock)?)?;
        } else {
            print_schema_lock_report(&schema_lock);
        }
        bail!("sync stopped because schema lock verification failed");
    }

    storage::rebuild_memory_catalog(&project)?;
    let reports = validate_all_records(&project)?;
    let invalid = reports.iter().filter(|report| !report.valid).count();
    if invalid > 0 {
        if json_output {
            print_json(&json!({
                "ok": false,
                "stage": "validation",
                "invalid_count": invalid,
                "reports": reports
            }))?;
        } else {
            print_validation_reports(&reports);
        }
        bail!("sync stopped because formal record validation failed");
    }

    storage::rebuild_document_index(&project)?;
    let report = index::rebuild(&project)?;
    if json_output {
        print_json(&json!({
            "ok": true,
            "documents": report.documents,
            "chunks": report.chunks,
            "collection": report.collection
        }))?;
    } else {
        println!(
            "Synced {} documents as {} chunks into {}.",
            report.documents, report.chunks, report.collection
        );
    }

    Ok(())
}

fn handle_remember(args: crate::cli::RememberArgs, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let write_guard = project.acquire_write_lock(write_lock_timeout())?;
    let input = MemoryCreate {
        kind: args.kind,
        text: args.text,
        space: args.space,
        subject: args.subject,
        importance: args.importance,
        source: args.source,
        tags: args.tags,
        supersedes: None,
    };

    if let Some(existing) = storage::find_exact_active_memory(&project, &input)? {
        if json_output {
            print_json(&json!({
                "id": existing.meta.id,
                "kind": existing.meta.kind,
                "space": existing.meta.space,
                "status": existing.meta.status,
                "path": storage::relative_path(&project, &existing.path),
                "deduplicated": true,
                "indexed": false,
                "index_warning": null
            }))?;
        } else {
            println!("Already remembered {}", existing.meta.id);
            println!("Kind: {}", existing.meta.kind);
            println!("Space: {}", existing.meta.space);
            println!("Path: {}", storage::relative_path(&project, &existing.path));
        }
        return Ok(());
    }

    if let Some(existing) = storage::find_active_subject_conflict(&project, &input)? {
        bail!(
            "active {} memory {} already owns subject '{}' in space '{}'; use 'yad memory supersede {} <new text>' to change the current truth",
            existing.meta.kind,
            existing.meta.id,
            existing.meta.subject.as_deref().unwrap_or_default(),
            existing.meta.space,
            existing.meta.id
        );
    }

    let memory = storage::create_memory(&project, input)?;
    drop(write_guard);

    let indexed = false;
    let index_warning = Some(
        "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
            .to_string(),
    );

    if json_output {
        print_json(&json!({
            "id": memory.meta.id,
            "kind": memory.meta.kind,
            "space": memory.meta.space,
            "status": memory.meta.status,
            "path": storage::relative_path(&project, &memory.path),
            "deduplicated": false,
            "indexed": indexed,
            "index_warning": index_warning
        }))?;
    } else {
        println!("Remembered {}", memory.meta.id);
        println!("Kind: {}", memory.meta.kind);
        println!("Space: {}", memory.meta.space);
        println!("Path: {}", storage::relative_path(&project, &memory.path));
        if let Some(warning) = index_warning {
            eprintln!("index warning: {}", warning);
        }
    }

    Ok(())
}

fn handle_search(args: crate::cli::SearchArgs, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let outcome = search::search(
        &project,
        &args.query,
        args.limit,
        args.space.as_deref(),
        args.kind.as_deref(),
        args.include_history,
    )?;

    if json_output {
        print_json(&json!({
            "semantic_used": outcome.semantic_used,
            "warning": outcome.semantic_warning,
            "results": outcome.results
        }))?;
    } else {
        if let Some(warning) = &outcome.semantic_warning {
            eprintln!("search warning: {}", warning);
        }

        if outcome.results.is_empty() {
            println!("No results.");
        } else {
            for item in outcome.results {
                println!(
                    "[{}/{}] {}  score={:.3}  space={}  status={}",
                    item.source_type, item.kind, item.id, item.score, item.space, item.status
                );
                println!("{}", item.title);
                if !item.snippet.is_empty() {
                    println!("{}", item.snippet);
                }
                println!("path: {}", item.path);
                println!();
            }
        }
    }

    Ok(())
}

fn handle_memory(command: MemoryCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;

    match command {
        MemoryCommand::List {
            kind,
            space,
            status,
        } => {
            let mut items = storage::list_memories(&project)?;
            items.retain(|memory| {
                kind.as_ref()
                    .map(|value| memory.meta.kind.eq_ignore_ascii_case(value))
                    .unwrap_or(true)
                    && space
                        .as_ref()
                        .map(|value| {
                            memory.meta.space.eq_ignore_ascii_case(value)
                                || memory
                                    .meta
                                    .space
                                    .to_lowercase()
                                    .starts_with(&format!("{}/", value.to_lowercase()))
                        })
                        .unwrap_or(true)
                    && status
                        .as_ref()
                        .map(|value| memory.meta.status.eq_ignore_ascii_case(value))
                        .unwrap_or(true)
            });

            if json_output {
                let values = items
                    .iter()
                    .map(|memory| {
                        json!({
                            "id": memory.meta.id,
                            "kind": memory.meta.kind,
                            "subject": memory.meta.subject,
                            "space": memory.meta.space,
                            "status": memory.meta.status,
                            "importance": memory.meta.importance,
                            "created": memory.meta.created,
                            "path": storage::relative_path(&project, &memory.path)
                        })
                    })
                    .collect::<Vec<_>>();
                print_json(&json!(values))?;
            } else {
                for memory in items {
                    println!(
                        "{}  {:<11} {:<10} {}  [{}]",
                        memory.meta.id,
                        memory.meta.kind,
                        memory.meta.status,
                        memory.meta.subject.as_deref().unwrap_or("-"),
                        memory.meta.space
                    );
                }
            }
        }

        MemoryCommand::Get { id } => {
            let memory = storage::find_memory(&project, &id)?;
            if json_output {
                print_json(&json!({
                    "metadata": memory.meta,
                    "content": memory.body,
                    "path": storage::relative_path(&project, &memory.path)
                }))?;
            } else {
                println!("ID: {}", memory.meta.id);
                println!("Kind: {}", memory.meta.kind);
                println!("Status: {}", memory.meta.status);
                println!("Space: {}", memory.meta.space);
                println!("Importance: {}", memory.meta.importance);
                if let Some(subject) = &memory.meta.subject {
                    println!("Subject: {}", subject);
                }
                if let Some(source) = &memory.meta.source {
                    println!("Source: {}", source);
                }
                println!("Path: {}", storage::relative_path(&project, &memory.path));
                println!();
                println!("{}", memory.body);
            }
        }

        MemoryCommand::Archive { id } => {
            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let mut memory = storage::find_memory(&project, &id)?;

            if memory.meta.status == "superseded" {
                bail!("cannot archive a superseded memory");
            }
            if memory.meta.status == "archived" {
                bail!("memory {} is already archived", memory.meta.id);
            }

            memory.meta.status = "archived".to_string();
            memory.meta.updated = now();
            storage::save_memory(&project, &memory)?;
            drop(write_guard);

            let indexed = false;
            let warning = Some(
                "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
                    .to_string(),
            );

            if json_output {
                print_json(&json!({
                    "id": memory.meta.id,
                    "status": memory.meta.status,
                    "indexed": indexed,
                    "index_warning": warning
                }))?;
            } else {
                println!("Archived {}", memory.meta.id);
                if let Some(warning) = warning {
                    eprintln!("index warning: {}", warning);
                }
            }
        }

        MemoryCommand::Supersede {
            id,
            text,
            kind,
            subject,
            source,
            importance,
        } => {
            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let mut old = storage::find_memory(&project, &id)?;
            if old.meta.status != "active" {
                bail!("only active memories can be superseded");
            }

            let new = storage::create_memory(
                &project,
                MemoryCreate {
                    kind: kind.unwrap_or_else(|| old.meta.kind.clone()),
                    text,
                    space: old.meta.space.clone(),
                    subject: subject.or_else(|| old.meta.subject.clone()),
                    importance: importance.unwrap_or(old.meta.importance),
                    source: source.or_else(|| old.meta.source.clone()),
                    tags: old.meta.tags.clone(),
                    supersedes: Some(old.meta.id.clone()),
                },
            )?;

            old.meta.status = "superseded".to_string();
            old.meta.superseded_by = Some(new.meta.id.clone());
            old.meta.updated = now();
            storage::save_memory(&project, &old)?;

            drop(write_guard);

            let indexed = false;
            let warning = Some(
                "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
                    .to_string(),
            );

            if json_output {
                print_json(&json!({
                    "old_id": old.meta.id,
                    "old_status": old.meta.status,
                    "new_id": new.meta.id,
                    "new_status": new.meta.status,
                    "indexed": indexed,
                    "index_warning": warning
                }))?;
            } else {
                println!("Superseded {} -> {}", old.meta.id, new.meta.id);
                if let Some(warning) = warning {
                    eprintln!("index warning: {}", warning);
                }
            }
        }
    }

    Ok(())
}

fn handle_adr(command: AdrCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let definition = schema::load(&project.schema_path("adr"))?;

    match command {
        AdrCommand::New {
            title,
            space,
            context,
            decision,
            consequences,
            tags,
        } => {
            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let adr = storage::create_adr(
                &project,
                &title,
                &space,
                context.as_deref(),
                decision.as_deref(),
                consequences.as_deref(),
                tags,
            )?;
            let report = schema::validate_adr(&adr.path, &definition)?;
            drop(write_guard);

            let indexed = false;
            let warning = Some(if report.valid {
                "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
                    .to_string()
            } else {
                "ADR is invalid and was not indexed; complete it and run 'yad sync'".to_string()
            });

            if json_output {
                print_json(&json!({
                    "id": adr.meta.id,
                    "status": adr.meta.status,
                    "space": adr.meta.space,
                    "path": storage::relative_path(&project, &adr.path),
                    "valid": report.valid,
                    "issues": report.issues,
                    "indexed": indexed,
                    "index_warning": warning
                }))?;
            } else {
                println!("Created {}", adr.meta.id);
                println!("Path: {}", storage::relative_path(&project, &adr.path));
                println!("Schema valid: {}", yes_no(report.valid));
                for issue in report.issues {
                    println!("  - {}: {}", issue.code, issue.message);
                }
                if let Some(warning) = warning {
                    eprintln!("index warning: {}", warning);
                }
            }
        }

        AdrCommand::List { space, status } => {
            let mut items = storage::list_adrs(&project)?;
            items.retain(|adr| {
                space
                    .as_ref()
                    .map(|value| {
                        adr.meta.space.eq_ignore_ascii_case(value)
                            || adr
                                .meta
                                .space
                                .to_lowercase()
                                .starts_with(&format!("{}/", value.to_lowercase()))
                    })
                    .unwrap_or(true)
                    && status
                        .as_ref()
                        .map(|value| adr.meta.status.eq_ignore_ascii_case(value))
                        .unwrap_or(true)
            });

            if json_output {
                let values = items
                    .iter()
                    .map(|adr| {
                        json!({
                            "id": adr.meta.id,
                            "title": adr.meta.title,
                            "status": adr.meta.status,
                            "space": adr.meta.space,
                            "created": adr.meta.created,
                            "path": storage::relative_path(&project, &adr.path)
                        })
                    })
                    .collect::<Vec<_>>();
                print_json(&json!(values))?;
            } else {
                for adr in items {
                    println!(
                        "{}  {:<11} [{}] {}",
                        adr.meta.id, adr.meta.status, adr.meta.space, adr.meta.title
                    );
                }
            }
        }

        AdrCommand::Show { id } => {
            let adr = storage::find_adr(&project, &id)?;
            if json_output {
                print_json(&json!({
                    "metadata": adr.meta,
                    "content": adr.body,
                    "path": storage::relative_path(&project, &adr.path)
                }))?;
            } else {
                print!("{}", storage::read_text(&adr.path)?);
            }
        }

        AdrCommand::Validate { id } => {
            let lock = schema::verify_lock(&project.yad_dir)?;
            if !lock.valid {
                if json_output {
                    print_json(&serde_json::to_value(&lock)?)?;
                } else {
                    print_schema_lock_report(&lock);
                }
                bail!("schema lock verification failed");
            }

            storage::list_memories(&project)?;
            let reports = if let Some(id) = id {
                let adr = storage::find_adr(&project, &id)?;
                vec![schema::validate_adr(&adr.path, &definition)?]
            } else {
                validate_all_adrs(&project)?
            };
            let invalid = reports.iter().filter(|report| !report.valid).count();

            if json_output {
                print_json(&json!({
                    "valid": invalid == 0,
                    "invalid_count": invalid,
                    "reports": reports
                }))?;
            } else {
                print_validation_reports(&reports);
            }

            if invalid > 0 {
                bail!("ADR validation failed");
            }
        }

        AdrCommand::Accept { id } => {
            transition_adr(&project, &definition, &id, "accepted", json_output)?;
        }

        AdrCommand::Reject { id } => {
            transition_adr(&project, &definition, &id, "rejected", json_output)?;
        }

        AdrCommand::Deprecate { id } => {
            transition_adr(&project, &definition, &id, "deprecated", json_output)?;
        }

        AdrCommand::Supersede { id, by } => {
            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let mut old = storage::find_adr(&project, &id)?;
            let mut new = storage::find_adr(&project, &by)?;

            if new.meta.status != "accepted" {
                bail!("replacement ADR {} must be accepted first", new.meta.id);
            }
            schema::ensure_transition(&definition, &old.meta.status, "superseded")?;

            if !old
                .meta
                .superseded_by
                .iter()
                .any(|value| value == &new.meta.id)
            {
                old.meta.superseded_by.push(new.meta.id.clone());
            }
            old.meta.status = "superseded".to_string();
            old.meta.updated = now();

            if !new
                .meta
                .supersedes
                .iter()
                .any(|value| value == &old.meta.id)
            {
                new.meta.supersedes.push(old.meta.id.clone());
            }
            new.meta.updated = now();

            storage::save_adr(&project, &old)?;
            storage::save_adr(&project, &new)?;

            drop(write_guard);

            let warning = Some(
                "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
                    .to_string(),
            );

            if json_output {
                print_json(&json!({
                    "superseded": old.meta.id,
                    "by": new.meta.id,
                    "index_warning": warning
                }))?;
            } else {
                println!("Superseded {} by {}", old.meta.id, new.meta.id);
                if let Some(warning) = warning {
                    eprintln!("index warning: {}", warning);
                }
            }
        }
    }

    Ok(())
}

fn handle_record(command: RecordCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;

    match command {
        RecordCommand::New {
            schema: selector,
            title,
            space,
            sections,
            tags,
        } => {
            let lock = schema::verify_lock(&project.yad_dir)?;
            if !lock.valid {
                bail!("schema lock verification failed");
            }

            let definition = schema::resolve(&project.yad_dir, &selector)?;
            let values = parse_section_values(sections)?;
            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let record = storage::create_record(
                &project,
                &definition,
                RecordCreate {
                    title,
                    space,
                    sections: values,
                    tags,
                },
            )?;
            let report = schema::validate_record(&record.path, &definition)?;
            drop(write_guard);

            let warning = if report.valid {
                "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
                    .to_string()
            } else {
                "record skeleton was created but required sections are incomplete; complete it before 'yad sync'."
                    .to_string()
            };

            if json_output {
                print_json(&json!({
                    "id": record.meta.id,
                    "schema": definition.id,
                    "abbreviation": definition.abbreviation(),
                    "status": record.meta.status,
                    "space": record.meta.space,
                    "path": storage::relative_path(&project, &record.path),
                    "valid": report.valid,
                    "issues": report.issues,
                    "index_warning": warning
                }))?;
            } else {
                println!("Created {} ({})", record.meta.id, definition.title);
                println!("Status: {}", record.meta.status);
                println!("Path: {}", storage::relative_path(&project, &record.path));
                println!("Schema valid: {}", yes_no(report.valid));
                for issue in report.issues {
                    println!("  - {}: {}", issue.code, issue.message);
                }
                eprintln!("index warning: {}", warning);
            }
        }

        RecordCommand::List {
            schema: schema_selector,
            space,
            status,
        } => {
            let schema_id = match schema_selector {
                Some(selector) => Some(schema::resolve(&project.yad_dir, &selector)?.id),
                None => None,
            };

            let mut records = storage::list_records(&project)?;
            records.retain(|record| {
                schema_id
                    .as_ref()
                    .map(|value| record.meta.schema.eq_ignore_ascii_case(value))
                    .unwrap_or(true)
                    && space
                        .as_ref()
                        .map(|value| {
                            record.meta.space.eq_ignore_ascii_case(value)
                                || record
                                    .meta
                                    .space
                                    .to_ascii_lowercase()
                                    .starts_with(&format!("{}/", value.to_ascii_lowercase()))
                        })
                        .unwrap_or(true)
                    && status
                        .as_ref()
                        .map(|value| record.meta.status.eq_ignore_ascii_case(value))
                        .unwrap_or(true)
            });

            if json_output {
                let values = records
                    .iter()
                    .map(|record| {
                        json!({
                            "id": record.meta.id,
                            "schema": record.meta.schema,
                            "title": record.meta.title,
                            "status": record.meta.status,
                            "space": record.meta.space,
                            "created": record.meta.created,
                            "path": storage::relative_path(&project, &record.path)
                        })
                    })
                    .collect::<Vec<_>>();
                print_json(&json!(values))?;
            } else {
                for record in records {
                    println!(
                        "{}  {:<24} {:<12} [{}] {}",
                        record.meta.id,
                        record.meta.schema,
                        record.meta.status,
                        record.meta.space,
                        record.meta.title
                    );
                }
            }
        }

        RecordCommand::Show { id } => {
            let record = storage::find_record(&project, &id)?;
            if json_output {
                print_json(&json!({
                    "metadata": record.meta,
                    "content": record.body,
                    "path": storage::relative_path(&project, &record.path)
                }))?;
            } else {
                print!("{}", storage::read_text(&record.path)?);
            }
        }

        RecordCommand::Set { id, section, value } => {
            let lock = schema::verify_lock(&project.yad_dir)?;
            if !lock.valid {
                bail!("schema lock verification failed");
            }

            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let mut record = storage::find_record(&project, &id)?;
            let definition = schema::resolve(&project.yad_dir, &record.meta.schema)?;
            let section_definition = definition
                .section(&section)
                .with_context(|| {
                    format!(
                        "schema '{}' does not define section '{}'",
                        definition.id, section
                    )
                })?
                .clone();
            let value = resolve_section_value(&value)?;
            record.body =
                schema::replace_section(&record.body, &section_definition.title, &value);
            record.meta.updated = now();
            storage::save_record(&project, &record)?;
            let report = schema::validate_record(&record.path, &definition)?;
            drop(write_guard);

            if json_output {
                print_json(&json!({
                    "id": record.meta.id,
                    "section": section_definition.key,
                    "valid": report.valid,
                    "issues": report.issues
                }))?;
            } else {
                println!("Updated {} section '{}'.", record.meta.id, section_definition.key);
                println!("Schema valid: {}", yes_no(report.valid));
                for issue in report.issues {
                    println!("  - {}: {}", issue.code, issue.message);
                }
            }
        }

        RecordCommand::Validate {
            id,
            schema: schema_selector,
        } => {
            let lock = schema::verify_lock(&project.yad_dir)?;
            if !lock.valid {
                if json_output {
                    print_json(&serde_json::to_value(&lock)?)?;
                } else {
                    print_schema_lock_report(&lock);
                }
                bail!("schema lock verification failed");
            }

            let reports = if let Some(id) = id {
                let record = storage::find_record(&project, &id)?;
                let definition = schema::resolve(&project.yad_dir, &record.meta.schema)?;
                vec![schema::validate_record(&record.path, &definition)?]
            } else {
                let schema_id = match schema_selector {
                    Some(selector) => Some(schema::resolve(&project.yad_dir, &selector)?.id),
                    None => None,
                };
                validate_all_records(&project)?
                    .into_iter()
                    .filter(|report| {
                        schema_id
                            .as_ref()
                            .map(|value| {
                                report
                                    .schema
                                    .as_ref()
                                    .map(|schema| schema.eq_ignore_ascii_case(value))
                                    .unwrap_or(false)
                            })
                            .unwrap_or(true)
                    })
                    .collect()
            };

            let invalid = reports.iter().filter(|report| !report.valid).count();
            if json_output {
                print_json(&json!({
                    "valid": invalid == 0,
                    "invalid_count": invalid,
                    "reports": reports
                }))?;
            } else {
                print_validation_reports(&reports);
            }
            if invalid > 0 {
                bail!("record validation failed");
            }
        }

        RecordCommand::Transition { id, status } => {
            transition_record(&project, &id, &status, json_output)?;
        }

        RecordCommand::Supersede { id, by } => {
            let lock = schema::verify_lock(&project.yad_dir)?;
            if !lock.valid {
                bail!("schema lock verification failed");
            }

            let write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let mut old = storage::find_record(&project, &id)?;
            let mut new = storage::find_record(&project, &by)?;

            if !old.meta.schema.eq_ignore_ascii_case(&new.meta.schema) {
                bail!(
                    "replacement record must use the same schema: '{}' != '{}'",
                    old.meta.schema,
                    new.meta.schema
                );
            }

            let definition = schema::resolve(&project.yad_dir, &old.meta.schema)?;
            let old_report = schema::validate_record(&old.path, &definition)?;
            let new_report = schema::validate_record(&new.path, &definition)?;
            if !old_report.valid || !new_report.valid {
                bail!("both records must be schema-valid before supersession");
            }
            if definition.is_historical_status(&new.meta.status) {
                bail!(
                    "replacement record {} is historical ({})",
                    new.meta.id,
                    new.meta.status
                );
            }

            schema::ensure_transition(&definition, &old.meta.status, "superseded")?;

            if !old.meta.superseded_by.iter().any(|value| value == &new.meta.id) {
                old.meta.superseded_by.push(new.meta.id.clone());
            }
            old.meta.status = "superseded".to_string();
            old.meta.updated = now();

            if !new.meta.supersedes.iter().any(|value| value == &old.meta.id) {
                new.meta.supersedes.push(old.meta.id.clone());
            }
            new.meta.updated = now();

            storage::save_record(&project, &old)?;
            storage::save_record(&project, &new)?;
            drop(write_guard);

            if json_output {
                print_json(&json!({
                    "superseded": old.meta.id,
                    "by": new.meta.id,
                    "schema": definition.id
                }))?;
            } else {
                println!("Superseded {} by {}", old.meta.id, new.meta.id);
            }
        }

        RecordCommand::Types => {
            print_schema_catalog(&project, json_output)?;
        }
    }

    Ok(())
}

fn transition_record(
    project: &Project,
    id: &str,
    to: &str,
    json_output: bool,
) -> Result<()> {
    let lock = schema::verify_lock(&project.yad_dir)?;
    if !lock.valid {
        bail!("schema lock verification failed");
    }

    let write_guard = project.acquire_write_lock(write_lock_timeout())?;
    let mut record = storage::find_record(project, id)?;
    let definition = schema::resolve(&project.yad_dir, &record.meta.schema)?;
    let report = schema::validate_record(&record.path, &definition)?;
    if !report.valid {
        bail!(
            "{} is invalid; complete the record before changing lifecycle state",
            record.meta.id
        );
    }

    schema::ensure_transition(&definition, &record.meta.status, to)?;
    record.meta.status = to.to_string();
    record.meta.updated = now();
    storage::save_record(project, &record)?;
    drop(write_guard);

    if json_output {
        print_json(&json!({
            "id": record.meta.id,
            "schema": record.meta.schema,
            "status": record.meta.status
        }))?;
    } else {
        println!("{} -> {}", record.meta.id, record.meta.status);
    }
    Ok(())
}

fn parse_section_values(values: Vec<String>) -> Result<BTreeMap<String, String>> {
    let mut sections = BTreeMap::new();
    for item in values {
        let (key, value) = item
            .split_once('=')
            .with_context(|| format!("section '{}' must use key=value", item))?;
        let key = key.trim();
        if key.is_empty() {
            bail!("section key cannot be empty");
        }
        if sections.contains_key(key) {
            bail!("section '{}' was provided more than once", key);
        }
        sections.insert(key.to_string(), resolve_section_value(value)?);
    }
    Ok(sections)
}

fn resolve_section_value(value: &str) -> Result<String> {
    if let Some(literal) = value.strip_prefix("@@") {
        return Ok(format!("@{}", literal));
    }
    if let Some(path) = value.strip_prefix('@') {
        if path.trim().is_empty() {
            bail!("section file path cannot be empty");
        }
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read section content from '{}'", path));
    }
    Ok(value.to_string())
}

fn validate_all_records(project: &Project) -> Result<Vec<schema::ValidationReport>> {
    let mut reports = Vec::new();
    for record in storage::list_records(project)? {
        let definition = schema::resolve(&project.yad_dir, &record.meta.schema)
            .with_context(|| {
                format!(
                    "record {} references unavailable schema '{}'",
                    record.meta.id, record.meta.schema
                )
            })?;
        reports.push(schema::validate_record(&record.path, &definition)?);
    }
    Ok(reports)
}

fn print_schema_catalog(project: &Project, json_output: bool) -> Result<()> {
    let definitions = schema::list_definitions(&project.yad_dir)?;
    if json_output {
        print_json(&serde_json::to_value(definitions)?)?;
    } else {
        for definition in definitions {
            let standard = definition
                .standard
                .as_deref()
                .map(|value| format!(" | {}", value))
                .unwrap_or_default();
            println!(
                "{:<6} {:<30} {}{}",
                definition.abbreviation(),
                definition.id,
                definition.title,
                standard
            );
            if !definition.description.trim().is_empty() {
                println!("       {}", definition.description);
            }
        }
    }
    Ok(())
}

fn handle_validate(json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let schema_lock = schema::verify_lock(&project.yad_dir)?;
    if !schema_lock.valid {
        if json_output {
            print_json(&serde_json::to_value(&schema_lock)?)?;
        } else {
            print_schema_lock_report(&schema_lock);
        }
        bail!("schema lock verification failed");
    }

    let memories = storage::list_memories(&project)?;
    let reports = validate_all_records(&project)?;
    let invalid = reports.iter().filter(|report| !report.valid).count();

    if json_output {
        print_json(&json!({
            "valid": invalid == 0,
            "memory_count": memories.len(),
            "invalid_count": invalid,
            "reports": reports
        }))?;
    } else {
        println!("Memory: {} valid documents", memories.len());
        print_validation_reports(&reports);
    }

    if invalid > 0 {
        bail!("validation failed: {} invalid formal record(s)", invalid);
    }

    Ok(())
}

fn handle_index(command: IndexCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;

    match command {
        IndexCommand::Rebuild => {
            let _write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let report = index::rebuild(&project)?;
            if json_output {
                print_json(&json!({
                    "documents": report.documents,
                    "chunks": report.chunks,
                    "collection": report.collection
                }))?;
            } else {
                println!(
                    "Indexed {} documents as {} chunks into {}.",
                    report.documents, report.chunks, report.collection
                );
            }
        }

        IndexCommand::Status => {
            let store = QdrantStore::for_project(&project)?;
            let available = store.is_available();
            let exists = if available {
                store.collection_exists().unwrap_or(false)
            } else {
                false
            };
            let local = index::status(&project)?;

            if json_output {
                print_json(&json!({
                    "qdrant_available": available,
                    "collection_exists": exists,
                    "collection": store.collection_name(),
                    "has_local_state": local.has_state,
                    "stale": local.stale,
                    "state": local.state
                }))?;
            } else {
                println!("Qdrant: {}", if available { "online" } else { "offline" });
                println!(
                    "Collection {}: {}",
                    store.collection_name(),
                    if exists { "ready" } else { "missing" }
                );
                println!(
                    "Index freshness: {}",
                    if local.stale { "stale" } else { "current" }
                );
            }
        }
    }

    Ok(())
}

fn handle_infra(command: InfraCommand, json_output: bool) -> Result<()> {
    match command {
        InfraCommand::Up => {
            let message = infra::up()?;
            if json_output {
                print_json(&json!({ "ok": true, "message": message }))?;
            } else {
                println!("{}", message);
            }
        }
        InfraCommand::Down => {
            let message = infra::down()?;
            if json_output {
                print_json(&json!({ "ok": true, "message": message }))?;
            } else {
                println!("{}", message);
            }
        }
        InfraCommand::Status => {
            let state = infra::qdrant_container_state()?;
            if json_output {
                print_json(&json!({
                    "docker_available": infra::docker_available(),
                    "container": infra::QDRANT_CONTAINER,
                    "image": infra::QDRANT_IMAGE,
                    "state": state
                }))?;
            } else {
                println!(
                    "Docker: {}",
                    if infra::docker_available() {
                        "available"
                    } else {
                        "missing"
                    }
                );
                println!(
                    "Qdrant container: {}",
                    state.unwrap_or_else(|| "not-created".to_string())
                );
            }
        }
    }

    Ok(())
}

fn handle_model(command: ModelCommand, json_output: bool) -> Result<()> {
    match command {
        ModelCommand::Status => {
            let status = model_cache::status()?;
            if json_output {
                print_json(&serde_json::to_value(status)?)?;
            } else {
                println!("Model: {}", status.model);
                println!("Revision: {}", status.revision);
                println!("Directory: {}", status.directory);
                println!("Ready: {}", yes_no(status.ready));
                for file in status.files {
                    println!(
                        "  {}  {}  {} bytes",
                        if file.valid { "OK" } else { "ERR" },
                        file.name,
                        file.bytes
                    );
                }
            }
        }
        ModelCommand::Install { from } => {
            let status = model_cache::install_from(&from)?;
            if json_output {
                print_json(&serde_json::to_value(status)?)?;
            } else {
                println!("Installed model {}.", status.model);
                println!("Revision: {}", status.revision);
                println!("Directory: {}", status.directory);
                println!("Ready: {}", yes_no(status.ready));
            }
        }
    }

    Ok(())
}

fn handle_space(command: SpaceCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;

    match command {
        SpaceCommand::Add { id, name } => {
            let _write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let path = project.ensure_space_named(&id, name.as_deref())?;
            let metadata = project.get_space(&id)?;

            if json_output {
                print_json(&json!({
                    "space": metadata,
                    "path": storage::relative_path(&project, &path)
                }))?;
            } else {
                println!("Space: {}", metadata.id);
                println!("Name: {}", metadata.name);
                println!("Path: {}", storage::relative_path(&project, &path));
            }
        }
        SpaceCommand::List => {
            let spaces = project.list_spaces()?;
            if json_output {
                print_json(&serde_json::to_value(spaces)?)?;
            } else {
                for space in spaces {
                    println!("{}  {}", space.id, space.name);
                }
            }
        }
        SpaceCommand::Tree => {
            let spaces = project.list_spaces()?;
            if json_output {
                print_json(&serde_json::to_value(spaces)?)?;
            } else {
                for space in spaces {
                    let depth = space.id.matches('/').count();
                    let leaf = space.id.rsplit('/').next().unwrap_or(&space.id);
                    println!("{}{}", "  ".repeat(depth), leaf);
                }
            }
        }
        SpaceCommand::Show { id } => {
            let metadata = project.get_space(&id)?;
            let prefix = format!("{}/", id);
            let memories = storage::list_memories(&project)?
                .into_iter()
                .filter(|memory| memory.meta.space == id || memory.meta.space.starts_with(&prefix))
                .count();
            let records = storage::list_records(&project)?
                .into_iter()
                .filter(|record| {
                    record.meta.space == id || record.meta.space.starts_with(&prefix)
                })
                .collect::<Vec<_>>();
            let adrs = records
                .iter()
                .filter(|record| record.meta.schema == "adr")
                .count();

            if json_output {
                print_json(&json!({
                    "space": metadata,
                    "memories": memories,
                    "records": records.len(),
                    "adrs": adrs
                }))?;
            } else {
                println!("Space: {}", metadata.id);
                println!("Name: {}", metadata.name);
                println!("Memory: {}", memories);
                println!("Records: {}", records.len());
                println!("ADR: {}", adrs);
            }
        }
    }

    Ok(())
}

fn handle_doctor(json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let schemas = schema::list_definitions(&project.yad_dir)?;
    let schema_ok = !schemas.is_empty();
    let schema_lock = schema::verify_lock(&project.yad_dir)?;
    let store = QdrantStore::for_project(&project)?;
    let qdrant_available = store.is_available();
    let collection_exists = if qdrant_available {
        store.collection_exists().unwrap_or(false)
    } else {
        false
    };
    let memories = storage::list_memories(&project)?;
    let records = storage::list_records(&project)?;
    let reports = validate_all_records(&project)?;
    let invalid_records = reports.iter().filter(|report| !report.valid).count();
    let model_status = model_cache::status()?;
    let index_status = index::status(&project)?;

    let report = json!({
        "project": true,
        "schemas": schemas.len(),
        "schemas_valid": schema_ok,
        "schema_lock": schema_lock.valid,
        "docker": infra::docker_available(),
        "qdrant": qdrant_available,
        "qdrant_collection": collection_exists,
        "qdrant_collection_name": store.collection_name(),
        "memory_count": memories.len(),
        "record_count": records.len(),
        "invalid_records": invalid_records,
        "embedding_model": project.config.embedding.model,
        "embedding_dimensions": project.config.embedding.dimensions,
        "embedding_model_ready": model_status.ready,
        "embedding_model_directory": model_status.directory,
        "min_semantic_score": project.config.search.min_semantic_score,
        "index_stale": index_status.stale
    });

    if json_output {
        print_json(&report)?;
    } else {
        println!("Project: OK");
        println!("Schemas: {} ({})", schemas.len(), yes_no(schema_ok));
        println!("Schema lock: {}", yes_no(schema_lock.valid));
        println!("Memory documents: {}", memories.len());
        println!("Docker: {}", yes_no(infra::docker_available()));
        println!("Qdrant: {}", yes_no(qdrant_available));
        println!("Qdrant collection: {}", yes_no(collection_exists));
        println!("Record validation: {} invalid", invalid_records);
        println!("Embedding model files: {}", yes_no(model_status.ready));
        println!(
            "Embedding: {} ({} dimensions)",
            project.config.embedding.model, project.config.embedding.dimensions
        );
        println!(
            "Index: {}",
            if index_status.stale {
                "stale"
            } else {
                "current"
            }
        );
    }

    Ok(())
}

fn handle_schema(command: SchemaCommand, json_output: bool) -> Result<()> {
    let project = Project::discover(None)?;

    match command {
        SchemaCommand::List => {
            print_schema_catalog(&project, json_output)?;
        }

        SchemaCommand::Show { id } => {
            let definition = schema::resolve(&project.yad_dir, &id)?;
            let path = project.schema_path(&definition.id);
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("schema '{}' not found", definition.id))?;
            if json_output {
                print_json(&serde_json::to_value(definition)?)?;
            } else {
                print!("{}", raw);
            }
        }

        SchemaCommand::Verify => {
            let report = schema::verify_lock(&project.yad_dir)?;
            if json_output {
                print_json(&serde_json::to_value(&report)?)?;
            } else {
                print_schema_lock_report(&report);
            }
            if !report.valid {
                bail!("schema lock verification failed");
            }
        }

        SchemaCommand::Upgrade { force } => {
            let _write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let report = schema::install_builtins(&project.yad_dir, force)?;

            if json_output {
                print_json(&serde_json::to_value(&report)?)?;
            } else {
                println!("Built-in schema catalog: {}", report.total_builtins);
                println!("Installed: {}", report.installed.len());
                for id in &report.installed {
                    println!("  + {}", id);
                }
                println!("Updated: {}", report.updated.len());
                for id in &report.updated {
                    println!("  ~ {}", id);
                }
                println!("Unchanged/custom-preserved: {}", report.skipped.len());
                println!("schemas.lock refreshed.");
            }
        }

        SchemaCommand::Add { path, force } => {
            let _write_guard = project.acquire_write_lock(write_lock_timeout())?;
            let definition = schema::import_schema(&project.yad_dir, &path, force)?;
            if json_output {
                print_json(&serde_json::to_value(definition)?)?;
            } else {
                println!(
                    "Added schema {} ({}) - {}",
                    definition.id,
                    definition.abbreviation(),
                    definition.title
                );
                println!("{}", definition.description);
                println!("schemas.lock refreshed.");
            }
        }
    }

    Ok(())
}

fn transition_adr(
    project: &Project,
    definition: &schema::SchemaDefinition,
    id: &str,
    to: &str,
    json_output: bool,
) -> Result<()> {
    let write_guard = project.acquire_write_lock(write_lock_timeout())?;
    let lock = schema::verify_lock(&project.yad_dir)?;
    if !lock.valid {
        bail!("schema lock verification failed");
    }

    let mut adr = storage::find_adr(project, id)?;
    let report = schema::validate_adr(&adr.path, definition)?;
    if !report.valid {
        bail!(
            "ADR {} is invalid; fix it before changing lifecycle state",
            adr.meta.id
        );
    }

    schema::ensure_transition(definition, &adr.meta.status, to)?;
    adr.meta.status = to.to_string();
    adr.meta.updated = now();
    storage::save_adr(project, &adr)?;

    drop(write_guard);

    let indexed = false;
    let warning = Some(
        "source saved; semantic index is now stale. Run 'yad sync' before semantic retrieval."
            .to_string(),
    );

    if json_output {
        print_json(&json!({
            "id": adr.meta.id,
            "status": adr.meta.status,
            "indexed": indexed,
            "index_warning": warning
        }))?;
    } else {
        println!("{} -> {}", adr.meta.id, adr.meta.status);
        if let Some(warning) = warning {
            eprintln!("index warning: {}", warning);
        }
    }

    Ok(())
}

fn validate_all_adrs(project: &Project) -> Result<Vec<schema::ValidationReport>> {
    let definition = schema::load(&project.schema_path("adr"))?;
    let mut reports = Vec::new();

    for adr in storage::list_adrs(project)? {
        reports.push(schema::validate_adr(&adr.path, &definition)?);
    }

    Ok(reports)
}

fn print_validation_reports(reports: &[schema::ValidationReport]) {
    if reports.is_empty() {
        println!("No ADR records found.");
        return;
    }

    for report in reports {
        let id = report.id.as_deref().unwrap_or("<unknown>");
        if report.valid {
            println!("OK  {}", id);
        } else {
            println!("ERR {}", id);
            for issue in &report.issues {
                println!("    {}: {}", issue.code, issue.message);
            }
        }
    }
}

fn print_schema_lock_report(report: &schema::SchemaLockReport) {
    for item in &report.schemas {
        if item.valid {
            println!("OK  {}@{}", item.id, item.version);
        } else {
            println!("ERR {}@{}", item.id, item.version);
            if let Some(message) = &item.message {
                println!("    {}", message);
            }
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn write_lock_timeout() -> Duration {
    Duration::from_secs(15)
}

fn yes_no(value: bool) -> &'static str {
    if value { "OK" } else { "NO" }
}

fn print_json(value: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
