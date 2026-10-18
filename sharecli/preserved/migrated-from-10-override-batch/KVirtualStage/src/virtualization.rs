// SPDX-License-Identifier: Apache-2.0 OR MIT
use anyhow::{anyhow, Result};
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{ContainerCreateResponse, HostConfig, PortBinding};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<u16>,
    pub vnc_port: Option<u16>,
}

pub struct VirtualizationManager {
    docker: Docker,
    containers: HashMap<String, ContainerInfo>,
}

impl VirtualizationManager {
    pub async fn new() -> Result<Self> {
        info!("Initializing VirtualizationManager");

        let docker = Docker::connect_with_local_defaults()?;

        // Test connection
        match docker.ping().await {
            Ok(_) => info!("Docker connection successful"),
            Err(e) => {
                error!("Docker connection failed: {}", e);
                return Err(anyhow!("Docker connection failed: {}", e));
            }
        }

        Ok(Self {
            docker,
            containers: HashMap::new(),
        })
    }

    pub async fn create_container(
        &mut self,
        session_id: String,
        desktop: String,
        image: Option<String>,
        memory_mb: u64,
        cpu_cores: u32,
    ) -> Result<String> {
        info!("Creating container for session: {}", session_id);

        let image_name = image.unwrap_or_else(|| self.get_default_image(&desktop));

        // Ensure image exists
        self.ensure_image(&image_name).await?;

        let container_name = format!("kvirtualstage-{session_id}");

        // Find available VNC port
        let vnc_port = self.find_available_port(5900).await?;

        // Configure container
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "5900/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some(vnc_port.to_string()),
            }]),
        );

        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            memory: Some((memory_mb * 1024 * 1024) as i64), // Convert to bytes
            nano_cpus: Some(cpu_cores as i64 * 1_000_000_000), // Convert to nanocpus
            shm_size: Some(268435456),                      // 256MB shared memory
            ..Default::default()
        };

        let mut env = vec![
            "DISPLAY=:0".to_string(),
            "VNC_PASSWORD=kvirtualstage".to_string(),
            "RESOLUTION=1920x1080".to_string(),
        ];

        // Desktop-specific environment variables
        match desktop.as_str() {
            "kubuntu" => {
                env.push("DESKTOP_SESSION=plasma".to_string());
                env.push("XDG_SESSION_DESKTOP=KDE".to_string());
            }
            "ubuntu" => {
                env.push("DESKTOP_SESSION=ubuntu".to_string());
                env.push("XDG_SESSION_DESKTOP=ubuntu:GNOME".to_string());
            }
            _ => {
                warn!("Unknown desktop environment: {}, using default", desktop);
            }
        }

        let config = Config {
            image: Some(image_name.clone()),
            env: Some(env),
            host_config: Some(host_config),
            exposed_ports: Some({
                let mut ports = HashMap::new();
                ports.insert("5900/tcp".to_string(), HashMap::new());
                ports
            }),
            ..Default::default()
        };

        // Create container
        let response: ContainerCreateResponse = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.clone(),
                    platform: None,
                }),
                config,
            )
            .await?;

        let container_id = response.id;

        // Start container
        self.docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await?;

        // Store container info
        let container_info = ContainerInfo {
            id: container_id.clone(),
            name: container_name,
            image: image_name,
            status: "running".to_string(),
            ports: vec![vnc_port],
            vnc_port: Some(vnc_port),
        };

        self.containers.insert(session_id, container_info);

        info!("Container created successfully: {}", container_id);
        Ok(container_id)
    }

    pub async fn stop_container(&self, container_id: String) -> Result<()> {
        info!("Stopping container: {}", container_id);

        self.docker
            .stop_container(&container_id, None::<StopContainerOptions>)
            .await?;

        Ok(())
    }

    pub async fn remove_container(&self, container_id: String) -> Result<()> {
        info!("Removing container: {}", container_id);

        // Stop container first
        let _ = self.stop_container(container_id.clone()).await;

        // Remove container
        self.docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        Ok(())
    }

    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        Ok(self.containers.values().cloned().collect())
    }

    pub async fn get_container_info(&self, session_id: &str) -> Option<&ContainerInfo> {
        self.containers.get(session_id)
    }

    async fn ensure_image(&self, image_name: &str) -> Result<()> {
        info!("Ensuring image exists: {}", image_name);

        // Check if image exists locally
        let images = self.docker.list_images::<String>(None).await?;

        for image in images {
            if !image.repo_tags.is_empty() && image.repo_tags.contains(&image_name.to_string()) {
                info!("Image {} already exists locally", image_name);
                return Ok(());
            }
        }

        // Pull image
        info!("Pulling image: {}", image_name);
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image_name,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        info!("Image pull: {}", status);
                    }
                }
                Err(e) => {
                    error!("Image pull error: {}", e);
                    return Err(anyhow!("Failed to pull image: {}", e));
                }
            }
        }

        Ok(())
    }

    fn get_default_image(&self, desktop: &str) -> String {
        match desktop {
            "kubuntu" => "ghcr.io/kvirtualstage/kubuntu-desktop:latest".to_string(),
            "ubuntu" => "ghcr.io/kvirtualstage/ubuntu-desktop:latest".to_string(),
            "debian" => "ghcr.io/kvirtualstage/debian-desktop:latest".to_string(),
            _ => "ghcr.io/kvirtualstage/kubuntu-desktop:latest".to_string(),
        }
    }

    async fn find_available_port(&self, start_port: u16) -> Result<u16> {
        use std::net::{TcpListener, ToSocketAddrs};

        for port in start_port..start_port + 1000 {
            let addr = format!("127.0.0.1:{port}");
            if let Ok(mut addrs) = addr.to_socket_addrs() {
                if let Some(addr) = addrs.next() {
                    if TcpListener::bind(addr).is_ok() {
                        return Ok(port);
                    }
                }
            }
        }

        Err(anyhow!("No available port found"))
    }
}

// Add missing import
use futures::StreamExt;
