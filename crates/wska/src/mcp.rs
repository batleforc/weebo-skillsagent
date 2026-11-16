//! Interactive MCP configuration shared by the supported providers.

use crate::cli::{Agent, Scope};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
use toml_edit::{value, Array, DocumentMut, Item, Table};

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("terminal: {0}")]
    Terminal(#[from] io::Error),
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("invalid TOML in {path}: {source}")]
    Toml {
        path: PathBuf,
        source: toml_edit::TomlError,
    },
    #[error(transparent)]
    Target(#[from] crate::target::TargetError),
    #[error("cannot configure MCP for {0:?}: {1}")]
    Unsupported(Agent, String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Default)]
struct Form {
    name: String,
    transport: String,
    endpoint: String,
    args: String,
    env: String,
    field: usize,
}

impl Form {
    fn fields(&self) -> [&str; 5] {
        [
            &self.name,
            &self.transport,
            &self.endpoint,
            &self.args,
            &self.env,
        ]
    }

    fn current_mut(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.name,
            1 => &mut self.transport,
            2 => &mut self.endpoint,
            3 => &mut self.args,
            _ => &mut self.env,
        }
    }

    fn validate(&self) -> Result<(), McpError> {
        if self.name.trim().is_empty() {
            return Err(McpError::Invalid("the server name is required".into()));
        }
        if !matches!(self.transport.trim(), "stdio" | "http" | "sse") {
            return Err(McpError::Invalid(
                "transport must be stdio, http or sse".into(),
            ));
        }
        if self.endpoint.trim().is_empty() {
            return Err(McpError::Invalid("command or URL is required".into()));
        }
        Ok(())
    }
}

/// Run the terminal form and write one MCP server configuration.
pub fn setup(
    agent: Agent,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, McpError> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_form(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    let form = result?;
    form.validate()?;
    write_config(agent, scope, cwd, env, &form)
}

#[expect(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "fixed-size TUI fields and bounded navigation indices"
)]
fn run_form(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Form, McpError> {
    let mut form = Form {
        transport: "stdio".into(),
        ..Form::default()
    };
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(3)])
                .split(area);
            let fields = form.fields();
            let labels = ["Name", "Transport", "Command or URL", "Arguments (comma separated)", "Environment variables (names only)"];
            let items = fields.iter().enumerate().map(|(i, value)| {
                let marker = if i == form.field { ">" } else { " " };
                ListItem::new(format!("{marker} {:<32} {}", labels[i], value))
            }).collect::<Vec<_>>();
            frame.render_widget(
                List::new(items).block(Block::default().title(" wska mcp setup ").borders(Borders::ALL)),
                chunks[0],
            );
            frame.render_widget(
                Paragraph::new("Tab/↑↓: field   Enter: next/save   Esc: cancel   Credentials are referenced as ${ENV_VAR}")
                    .wrap(Wrap { trim: true })
                    .block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
        })?;
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                return Err(McpError::Invalid("cancelled".into()));
            }
            KeyEvent {
                code: KeyCode::Tab | KeyCode::Down,
                ..
            } => {
                form.field = (form.field + 1) % 5;
            }
            KeyEvent {
                code: KeyCode::BackTab | KeyCode::Up,
                ..
            } => {
                form.field = if form.field == 0 { 4 } else { form.field - 1 };
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } if form.field < 4 => {
                form.field += 1;
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => return Ok(form),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                form.current_mut().pop();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if !modifiers.contains(KeyModifiers::CONTROL) => {
                form.current_mut().push(c);
            }
            _ => {}
        }
    }
}

fn write_config(
    agent: Agent,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    form: &Form,
) -> Result<String, McpError> {
    match agent {
        Agent::Claude | Agent::Gemini => write_json(agent, scope, cwd, env, form),
        Agent::Codex => write_codex(scope, cwd, env, form),
        Agent::Antigravity => Err(McpError::Unsupported(
            agent,
            "its MCP configuration location is not standardized yet".into(),
        )),
    }
}

fn json_path(
    agent: Agent,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<PathBuf, McpError> {
    let home = env("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| McpError::Invalid("HOME is required".into()))?;
    Ok(match (agent, scope) {
        (Agent::Claude, Scope::Project) => crate::target::repo_root(cwd)?.join(".mcp.json"),
        (Agent::Claude, Scope::User) => home.join(".claude.json"),
        (Agent::Gemini, Scope::Project) => {
            crate::target::repo_root(cwd)?.join(".gemini/settings.json")
        }
        (Agent::Gemini, Scope::User) => home.join(".gemini/settings.json"),
        _ => {
            return Err(McpError::Invalid(
                "JSON configuration is not supported for this provider".into(),
            ))
        }
    })
}

fn server_json(form: &Form) -> Value {
    let mut server = Map::new();
    if matches!(form.transport.trim(), "http" | "sse") {
        server.insert("url".into(), Value::String(form.endpoint.trim().into()));
        if form.transport.trim() != "sse" {
            server.insert("type".into(), Value::String("http".into()));
        }
    } else {
        server.insert("command".into(), Value::String(form.endpoint.trim().into()));
        let args = csv(&form.args);
        if !args.is_empty() {
            server.insert("args".into(), json!(args));
        }
    }
    let env = env_refs(&form.env);
    if !env.is_empty() {
        server.insert("env".into(), json!(env));
    }
    Value::Object(server)
}

fn write_json(
    agent: Agent,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    form: &Form,
) -> Result<String, McpError> {
    let path = json_path(agent, scope, cwd, env)?;
    let mut root = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        serde_json::from_str::<Value>(&text).map_err(|source| McpError::Json {
            path: path.clone(),
            source,
        })?
    } else {
        json!({})
    };
    let object = root.as_object_mut().ok_or_else(|| {
        McpError::Invalid(format!("{} must contain a JSON object", path.display()))
    })?;
    let servers = object.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| McpError::Invalid("mcpServers must be a JSON object".into()))?;
    servers.insert(form.name.trim().into(), server_json(form));
    write_atomic(
        &path,
        serde_json::to_vec_pretty(&root)
            .map_err(|source| McpError::Json {
                path: path.clone(),
                source,
            })?
            .as_slice(),
    )?;
    Ok(format!(
        "configured {} in {}",
        form.name.trim(),
        path.display()
    ))
}

#[expect(
    clippy::indexing_slicing,
    reason = "toml_edit's indexed API is the supported way to create nested tables"
)]
fn write_codex(
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    form: &Form,
) -> Result<String, McpError> {
    if form.transport.trim() != "stdio" {
        return Err(McpError::Unsupported(
            Agent::Codex,
            "the first form supports stdio servers only".into(),
        ));
    }
    let home = env("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| McpError::Invalid("HOME is required".into()))?;
    let path = match scope {
        Scope::Project => crate::target::repo_root(cwd)?.join(".codex/config.toml"),
        Scope::User => home.join(".codex/config.toml"),
    };
    let mut doc = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        text.parse::<DocumentMut>()
            .map_err(|source| McpError::Toml {
                path: path.clone(),
                source,
            })?
    } else {
        DocumentMut::new()
    };
    let servers = doc["mcp_servers"].or_insert(Item::Table(Table::new()));
    let table = servers
        .as_table_mut()
        .ok_or_else(|| McpError::Invalid("mcp_servers must be a TOML table".into()))?;
    let mut server = Table::new();
    server["command"] = value(form.endpoint.trim());
    let args = csv(&form.args);
    if !args.is_empty() {
        let mut array = Array::new();
        for arg in args {
            array.push(arg);
        }
        server["args"] = Item::Value(array.into());
    }
    let vars = env_pairs(&form.env);
    if !vars.is_empty() {
        let mut env = Table::new();
        for (name, reference) in vars {
            env[&name] = value(reference);
        }
        server["env"] = Item::Table(env);
    }
    table[form.name.trim()] = Item::Table(server);
    write_atomic(&path, doc.to_string().as_bytes())?;
    Ok(format!(
        "configured {} in {}",
        form.name.trim(),
        path.display()
    ))
}

fn csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn env_refs(input: &str) -> Map<String, Value> {
    csv(input)
        .into_iter()
        .map(|name| (name.clone(), Value::String(format!("${{{name}}}"))))
        .collect()
}

fn env_pairs(input: &str) -> BTreeMap<String, String> {
    csv(input)
        .into_iter()
        .map(|name| (name.clone(), format!("${{{name}}}")))
        .collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), McpError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("wska-{}", std::process::id()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_fields_become_references_without_values() {
        let refs = env_refs("GITHUB_TOKEN, API_KEY");
        assert_eq!(refs["GITHUB_TOKEN"], "${GITHUB_TOKEN}");
        assert_eq!(refs["API_KEY"], "${API_KEY}");
    }

    #[test]
    fn stdio_server_serializes_command_args_and_environment() {
        let form = Form {
            name: "github".into(),
            transport: "stdio".into(),
            endpoint: "docker".into(),
            args: "run, -i, image".into(),
            env: "GITHUB_TOKEN".into(),
            field: 0,
        };
        let server = server_json(&form);
        assert_eq!(server["command"], "docker");
        assert_eq!(server["args"][1], "-i");
        assert_eq!(server["env"]["GITHUB_TOKEN"], "${GITHUB_TOKEN}");
    }

    #[test]
    fn form_rejects_unknown_transport() {
        let form = Form {
            transport: "ftp".into(),
            ..Form::default()
        };
        assert!(form.validate().is_err());
    }
}
