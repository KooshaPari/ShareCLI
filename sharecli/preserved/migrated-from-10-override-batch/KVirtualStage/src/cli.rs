// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

use crate::core::KVirtualStageCore;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "kvirtualstage")]
#[command(about = "A Playwright-equivalent desktop automation platform for AI agents")]
pub struct KVirtualStageCommand {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the KVirtualStage service
    Start {
        /// Enable web UI
        #[arg(long)]
        ui: bool,

        /// Port for web UI
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "localhost")]
        host: String,
    },

    /// Show system status
    Status,

    /// Session management
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Execute automation script
    Run {
        /// Script file to execute
        script: String,

        /// Session name to use
        #[arg(long)]
        session: Option<String>,
    },

    /// Record desktop interactions
    Record {
        /// Output file path
        #[arg(long, default_value = "recording.mp4")]
        output: String,

        /// Recording format (mp4, gif, webm)
        #[arg(long, default_value = "mp4")]
        format: String,

        /// Session name to record
        #[arg(long)]
        session: Option<String>,
    },

    /// Screenshot operations
    Screenshot {
        /// Output file path
        #[arg(long, default_value = "screenshot.png")]
        output: String,

        /// Session name to screenshot
        #[arg(long)]
        session: Option<String>,
    },

    /// MCP server operations
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
pub enum SessionCommands {
    /// Create a new session
    Create {
        /// Session name
        #[arg(long)]
        name: String,

        /// Desktop environment (kubuntu, ubuntu, debian)
        #[arg(long, default_value = "kubuntu")]
        desktop: String,

        /// Container image
        #[arg(long)]
        image: Option<String>,

        /// Resource limits
        #[arg(long, default_value = "2048")]
        memory: u64,

        #[arg(long, default_value = "2")]
        cpu: u32,
    },

    /// List all sessions
    List,

    /// Connect to a session
    Connect {
        /// Session name
        name: String,
    },

    /// Stop a session
    Stop {
        /// Session name
        name: String,
    },

    /// Remove a session
    Remove {
        /// Session name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum McpCommands {
    /// Start MCP server
    Start {
        /// Port for MCP server
        #[arg(long, default_value = "3001")]
        port: u16,
    },

    /// Stop MCP server
    Stop,

    /// List MCP tools
    Tools,

    /// Test MCP connection
    Test {
        /// MCP server URL
        url: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Initialize configuration
    Init,
}

impl KVirtualStageCommand {
    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            Commands::Start { ui, port, host } => {
                self.start_service(*ui, *port, host.clone()).await
            }
            Commands::Status => self.show_status().await,
            Commands::Session { command } => self.execute_session_command(command).await,
            Commands::Run { script, session } => self.run_script(script, session.clone()).await,
            Commands::Record {
                output,
                format,
                session,
            } => self.record_session(output, format, session.clone()).await,
            Commands::Screenshot { output, session } => {
                self.screenshot_session(output, session.clone()).await
            }
            Commands::Mcp { command } => self.execute_mcp_command(command).await,
            Commands::Config { command } => self.execute_config_command(command).await,
        }
    }

    async fn start_service(&self, ui: bool, port: u16, host: String) -> Result<()> {
        info!("Starting KVirtualStage service on {}:{}", host, port);

        let core = KVirtualStageCore::new().await?;

        if ui {
            info!("Web UI enabled");
            core.start_with_ui(host, port).await?;
        } else {
            core.start_headless().await?;
        }

        Ok(())
    }

    async fn show_status(&self) -> Result<()> {
        let core = KVirtualStageCore::new().await?;
        let status = core.get_status().await?;

        println!("KVirtualStage Status:");
        println!("  Version: {}", env!("CARGO_PKG_VERSION"));
        println!("  Sessions: {}", status.active_sessions);
        println!("  Container Runtime: {}", status.container_runtime);
        println!(
            "  Web UI: {}",
            if status.web_ui_active {
                "Active"
            } else {
                "Inactive"
            }
        );
        println!(
            "  MCP Server: {}",
            if status.mcp_server_active {
                "Active"
            } else {
                "Inactive"
            }
        );

        Ok(())
    }

    async fn execute_session_command(&self, command: &SessionCommands) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        match command {
            SessionCommands::Create {
                name,
                desktop,
                image,
                memory,
                cpu,
            } => {
                core.create_session(name.clone(), desktop.clone(), image.clone(), *memory, *cpu)
                    .await?;
                println!("Session '{}' created successfully", name);
            }
            SessionCommands::List => {
                let sessions = core.list_sessions().await?;
                println!("Active Sessions:");
                for session in sessions {
                    println!(
                        "  {} - {} ({}) - {}",
                        session.name, session.desktop, session.status, session.created_at
                    );
                }
            }
            SessionCommands::Connect { name } => {
                core.connect_session(name.clone()).await?;
                println!("Connected to session '{}'", name);
            }
            SessionCommands::Stop { name } => {
                core.stop_session(name.clone()).await?;
                println!("Session '{}' stopped", name);
            }
            SessionCommands::Remove { name } => {
                core.remove_session(name.clone()).await?;
                println!("Session '{}' removed", name);
            }
        }

        Ok(())
    }

    async fn run_script(&self, script: &str, session: Option<String>) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        info!("Running script: {}", script);
        if let Some(session_name) = session {
            core.run_script_in_session(script, session_name).await?;
        } else {
            core.run_script(script).await?;
        }

        Ok(())
    }

    async fn record_session(
        &self,
        output: &str,
        format: &str,
        session: Option<String>,
    ) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        info!("Recording session to: {} (format: {})", output, format);
        core.start_recording(output, format, session).await?;

        Ok(())
    }

    async fn screenshot_session(&self, output: &str, session: Option<String>) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        info!("Taking screenshot: {}", output);
        core.take_screenshot(output, session).await?;

        Ok(())
    }

    async fn execute_mcp_command(&self, command: &McpCommands) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        match command {
            McpCommands::Start { port } => {
                core.start_mcp_server(*port).await?;
                println!("MCP server started on port {}", port);
            }
            McpCommands::Stop => {
                core.stop_mcp_server().await?;
                println!("MCP server stopped");
            }
            McpCommands::Tools => {
                let tools = core.list_mcp_tools().await?;
                println!("Available MCP Tools:");
                for tool in tools {
                    println!("  {} - {}", tool.name, tool.description);
                }
            }
            McpCommands::Test { url } => {
                core.test_mcp_connection(url.clone()).await?;
                println!("MCP connection test successful");
            }
        }

        Ok(())
    }

    async fn execute_config_command(&self, command: &ConfigCommands) -> Result<()> {
        let core = KVirtualStageCore::new().await?;

        match command {
            ConfigCommands::Show => {
                let config = core.get_config().await?;
                println!("Current Configuration:");
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
            ConfigCommands::Set { key, value } => {
                core.set_config(key.clone(), value.clone()).await?;
                println!("Configuration updated: {} = {}", key, value);
            }
            ConfigCommands::Init => {
                core.init_config().await?;
                println!("Configuration initialized");
            }
        }

        Ok(())
    }
}
