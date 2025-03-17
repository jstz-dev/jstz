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
