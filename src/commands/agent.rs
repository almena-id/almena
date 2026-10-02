//! `almena agent`: the Almena agent, over A2A 1.0 (JSON-RPC binding). Each
//! message opens a task whose reply streams back as one text artifact;
//! messages sharing a context are one conversation.

use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::time::Duration;

use anyhow::{Context as _, bail};
use clap::Subcommand;
use reqwest::blocking::Client as Http;
use serde_json::{Value, json};

use crate::cli::VERSION;
use crate::context::{Context, prompt};
use crate::output::{col, notice};

const JSONRPC: &str = "a2a/jsonrpc";
const CARD: &str = ".well-known/agent-card.json";
const A2A_VERSION: &str = "1.0";

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Show the agent's card: who it is, its skills and endpoints.
    Card,
    /// Ask the agent one question; the answer streams as it is written.
    Ask {
        /// The question.
        #[arg(required = true, num_args = 1.., value_name = "TEXT")]
        text: Vec<String>,
        /// Carry on a conversation (the context id an earlier answer gave).
        #[arg(long)]
        context: Option<String>,
        /// Wait for the whole answer instead of streaming it.
        #[arg(long)]
        no_stream: bool,
    },
    /// Talk with the agent until an empty line or Ctrl-D.
    Chat {
        /// Carry on a conversation (a context id).
        #[arg(long)]
        context: Option<String>,
    },
}

struct Agent {
    http: Http,
    base: url::Url,
    verbose: bool,
}

/// How a task ended, and the conversation it belongs to.
struct Reply {
    text: String,
    state: String,
    context: Option<String>,
}

pub fn run(ctx: &Context, command: AgentCommand) -> anyhow::Result<()> {
    let agent = Agent::new(&ctx.agent_url, ctx.global.verbose)?;
    match command {
        AgentCommand::Card => {
            let card = agent.card()?;
            if ctx.out.is_json() {
                ctx.out.document(&card);
                return Ok(());
            }
            ctx.out.item(
                &card,
                &[
                    col("Name", "name"),
                    col("Description", "description"),
                    col("Version", "version"),
                    col("Provider", "provider.organization"),
                    col("Streaming", "capabilities.streaming"),
                ],
            );
            ctx.out.list(
                &card["skills"],
                &[
                    col("Skill", "id"),
                    col("Name", "name"),
                    col("Description", "description"),
                ],
            );
        }
        AgentCommand::Ask {
            text,
            context,
            no_stream,
        } => {
            let text = text.join(" ");
            let reply = if no_stream || ctx.out.is_json() {
                let task = agent.send(&text, context.as_deref())?;
                if ctx.out.is_json() {
                    ctx.out.document(&task);
                    return Ok(());
                }
                let reply = reply_of(&task);
                println!("{}", reply.text);
                reply
            } else {
                let reply = agent.stream(&text, context.as_deref())?;
                println!();
                reply
            };
            finish(&reply)?;
            if let Some(context) = reply.context {
                notice(format!("(carry on with --context {context})"));
            }
        }
        AgentCommand::Chat { mut context } => {
            if std::io::stdin().is_terminal() {
                notice(format!(
                    "Talking with {} — an empty line ends it.",
                    agent.base
                ));
            }
            loop {
                let line = prompt("> ")?;
                if line.is_empty() {
                    break;
                }
                let reply = agent.stream(&line, context.as_deref())?;
                println!("\n");
                finish(&reply)?;
                context = reply.context.or(context);
            }
        }
    }
    Ok(())
}

/// A task that did not complete is an error, with the reason the agent gave.
fn finish(reply: &Reply) -> anyhow::Result<()> {
    match reply.state.as_str() {
        "TASK_STATE_COMPLETED" => Ok(()),
        "TASK_STATE_REJECTED" => bail!("the agent declined the message"),
        "TASK_STATE_FAILED" => bail!("the agent could not answer; try again later"),
        other => bail!("the agent's task ended as {other}"),
    }
}

impl Agent {
    fn new(base: &str, verbose: bool) -> anyhow::Result<Self> {
        let mut base = url::Url::parse(base).with_context(|| format!("not a URL: {base}"))?;
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        let http = Http::builder()
            .user_agent(format!("almena/{VERSION}"))
            .connect_timeout(Duration::from_secs(10))
            // A long answer streams for minutes.
            .timeout(None)
            .build()?;
        Ok(Self {
            http,
            base,
            verbose,
        })
    }

    fn url(&self, path: &str) -> anyhow::Result<url::Url> {
        let url = self.base.join(path)?;
        if self.verbose {
            eprintln!("> {url}");
        }
        Ok(url)
    }

    fn card(&self) -> anyhow::Result<Value> {
        let response = self
            .http
            .get(self.url(CARD)?)
            .send()
            .context("could not reach the agent")?;
        Ok(response.error_for_status()?.json()?)
    }

    fn call(
        &self,
        method: &str,
        text: &str,
        context: Option<&str>,
    ) -> anyhow::Result<reqwest::blocking::Response> {
        let mut message = json!({
            "messageId": new_id(),
            "role": "ROLE_USER",
            "parts": [{"text": text}],
        });
        if let Some(context) = context {
            message["contextId"] = json!(context);
        }
        let body =
            json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": {"message": message}});
        let response = self
            .http
            .post(self.url(JSONRPC)?)
            .header("A2A-Version", A2A_VERSION)
            .json(&body)
            .send()
            .context("could not reach the agent")?;
        Ok(response.error_for_status()?)
    }

    /// The whole task, once it is done.
    fn send(&self, text: &str, context: Option<&str>) -> anyhow::Result<Value> {
        let answer: Value = self.call("SendMessage", text, context)?.json()?;
        rpc_error(&answer)?;
        Ok(answer["result"]["task"].clone())
    }

    /// The reply printed as it streams in (Server-Sent Events).
    fn stream(&self, text: &str, context: Option<&str>) -> anyhow::Result<Reply> {
        let response = self.call("SendStreamingMessage", text, context)?;
        let mut reply = Reply {
            text: String::new(),
            state: String::new(),
            context: None,
        };
        let mut stdout = std::io::stdout();
        for line in BufReader::new(response).lines() {
            let line = line.context("reading the agent's answer")?;
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let event: Value = serde_json::from_str(data.trim())?;
            rpc_error(&event)?;
            let result = &event["result"];
            for part in result["artifactUpdate"]["artifact"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(chunk) = part["text"].as_str() {
                    write!(stdout, "{chunk}")?;
                    stdout.flush()?;
                    reply.text.push_str(chunk);
                }
            }
            for scope in ["task", "statusUpdate", "artifactUpdate"] {
                if let Some(id) = result[scope]["contextId"].as_str() {
                    reply.context = Some(id.to_owned());
                }
            }
            let state = result["statusUpdate"]["status"]["state"]
                .as_str()
                .or(result["task"]["status"]["state"].as_str());
            if let Some(state) = state {
                reply.state = state.to_owned();
            }
        }
        Ok(reply)
    }
}

fn reply_of(task: &Value) -> Reply {
    let text = task["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|artifact| artifact["parts"].as_array().into_iter().flatten())
        .filter_map(|part| part["text"].as_str())
        .collect();
    Reply {
        text,
        state: task["status"]["state"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        context: task["contextId"].as_str().map(str::to_owned),
    }
}

fn rpc_error(answer: &Value) -> anyhow::Result<()> {
    match answer.get("error") {
        Some(error) => bail!(
            "the agent refused the request: {}",
            error["message"].as_str().unwrap_or("unknown error")
        ),
        None => Ok(()),
    }
}

/// A random UUID v4, for message ids.
fn new_id() -> String {
    let mut bytes: [u8; 16] = std::array::from_fn(|_| 0);
    getrandom(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Randomness enough for an id: the standard library's per-process random
/// hasher keys, mixed with the time.
fn getrandom(bytes: &mut [u8; 16]) {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    for (half, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(now);
        hasher.write_usize(half);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_ids_are_uuids() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(new_id(), id);
    }

    #[test]
    fn a_task_reads_as_a_reply() {
        let task = json!({
            "contextId": "c1",
            "status": {"state": "TASK_STATE_COMPLETED"},
            "artifacts": [{"parts": [{"text": "Hello "}, {"text": "there"}]}],
        });
        let reply = reply_of(&task);
        assert_eq!(reply.text, "Hello there");
        assert_eq!(reply.context.as_deref(), Some("c1"));
        assert!(finish(&reply).is_ok());
    }
}
