use anyhow::Result;
use bollard::{
    image::{CreateImageOptions, ListImagesOptions},
    Docker,
};
use futures_util::StreamExt;
use log::info;
use std::{collections::HashMap, sync::Arc};
#[async_trait::async_trait]
pub trait Image: Sized {
    const LATEST_TAG: &'static str = "latest";
    fn image_name(&self) -> &str;
    fn image_tag(&self) -> &str {
        Self::LATEST_TAG
    }
    fn image_uri(&self) -> String {
        format!("{}:{}", self.image_name(), self.image_tag())
    }
    // Overrides the entrypoint of the image
    fn entrypoint(&self) -> Option<&str> {
        None
    }
    // If specified, used as a validation on container creation
    fn exposed_ports(&self) -> &[u16] {
        &[]
    }
    async fn pull_image(&self, client: Arc<Docker>) -> Result<()> {
        if Self::image_exists(self, client.clone()).await {
            info!("Image: {:?} already exists ", self.image_name());
            return Ok(());
        }
        self.create_image(client.clone()).await
    }
    async fn image_exists(&self, client: Arc<Docker>) -> bool {
        let filters = [("reference".to_string(), vec![self.image_uri()])]
            .into_iter()
            .collect::<HashMap<_, _>>();
        let images = &client
            .list_images(Some(ListImagesOptions::<String> {
                all: true,
                filters,
                ..Default::default()
            }))
            .await;
        match images {
            Ok(images) => !images.is_empty(),
            Err(_) => false,
        }
    }
    async fn create_image(&self, client: Arc<Docker>) -> anyhow::Result<()> {
        let options = Some(CreateImageOptions {
            from_image: self.image_name(),
            tag: self.image_tag(),
            ..Default::default()
        });
        let mut stream = client.create_image(options, None, None);
        while let Some(create_info) = stream.next().await {
            match create_info {
                Ok(info) => {
                    if let Some(status) = info.status {
                        println!("{:?}", status)
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

pub struct GenericImage {
    image_name: String,
    image_tag: Option<String>,
    entrypoint: Option<String>,
    exposed_ports: Option<Vec<u16>>,
}

impl Image for GenericImage {
    fn image_name(&self) -> &str {
        &self.image_name
    }

    fn image_tag(&self) -> &str {
        if let Some(tag) = &self.image_tag {
            return tag;
        }
        Self::LATEST_TAG
    }

    fn entrypoint(&self) -> Option<&str> {
        self.entrypoint.as_deref()
    }

    fn exposed_ports(&self) -> &[u16] {
        if let Some(ports) = &self.exposed_ports {
            return ports;
        }
        &[]
    }
}

impl GenericImage {
    pub fn new(image_name: &str) -> GenericImage {
        GenericImage {
            image_name: image_name.to_string(),
            image_tag: None,
            entrypoint: None,
            exposed_ports: None,
        }
    }

    pub fn with_tag(mut self, image_tag: &str) -> Self {
        self.image_tag = Some(image_tag.to_string());
        self
    }

    pub fn with_entrypoint(mut self, entrypoint: &str) -> Self {
        self.entrypoint = Some(entrypoint.to_string());
        self
    }

    pub fn with_exposed_ports(mut self, exposed_ports: &[u16]) -> Self {
        self.exposed_ports = Some(Vec::from(exposed_ports));
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn builds_docker_image() {
        let image = GenericImage::new("busybox")
            .with_entrypoint("sh")
            .with_tag("stable")
            .with_exposed_ports(&[8080]);
        assert_eq!(image.image_name(), "busybox");
        assert_eq!(image.image_tag(), "stable");
        assert_eq!(image.entrypoint(), Some("sh"));
        assert_eq!(image.exposed_ports(), &[8080]);
    }
}
