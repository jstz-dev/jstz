use rust_embed::Embed;
use serde_json::Value;
use std::fmt::Display;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub trait ReadWritable: Read + std::io::Write {}

impl ReadWritable for tempfile::NamedTempFile {}

#[derive(PartialEq, Eq, Debug)]
pub enum ProtocolConstants {
    Sandbox,
}

impl Default for ProtocolConstants {
    fn default() -> Self {
        Self::Sandbox
    }
}

impl Display for ProtocolConstants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Sandbox => "sandbox",
        })
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Protocol {
    Alpha,
    ParisC,
    Quebec,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Alpha
    }
}

impl Protocol {
    fn hash(&self) -> &'static str {
        match self {
            Protocol::Alpha => "ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK",
            Protocol::ParisC => "PsParisCZo7KAh1Z1smVd9ZMZ1HHn5gkzbM94V3PLCpknFWhUAi",
            Protocol::Quebec => "PsQubecQubecQubecQubecQubecQubecQubecQubecQubec",
        }
    }

    fn parameter_file(&self, constants: &ProtocolConstants) -> PathBuf {
        Path::new(&constants.to_string()).join(self.hash())
    }
}

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/resources/protocol_parameters/"]
pub struct ProtocolParameterFile;

#[derive(Default)]
pub struct ProtocolParameterBuilder {
    /// Target protocol version.
    protocol: Option<Protocol>,
    /// Protocol constants.
    constants: Option<ProtocolConstants>,
    /// Path to an existing parameter file whose content will be used as the base
    /// parameter set. Optional. If `source_path` is not given, a predefined parameter
    /// file will be used instead depending on `protocol` and `constants`.
    source_path: Option<PathBuf>,
}

impl ProtocolParameterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_protocol(&mut self, protocol: Protocol) -> &mut Self {
        self.protocol.replace(protocol);
        self
    }

    pub fn set_constants(&mut self, constants: ProtocolConstants) -> &mut Self {
        self.constants.replace(constants);
        self
    }

    pub fn set_source_path(&mut self, path: &str) -> &mut Self {
        self.source_path = Some(PathBuf::from(path));
        self
    }

    pub fn build(&mut self) -> anyhow::Result<impl ReadWritable> {
        let protocol = self.protocol.take();
        let constants = self.constants.take();
        let source_path = self.source_path.take();
        let mut raw_json = self.load_parameter_json(source_path, protocol, constants)?;
        let json = raw_json.as_object_mut().ok_or(anyhow::anyhow!(
            "Failed to convert loaded json file into a json object"
        ))?;

        let mut output_file = tempfile::NamedTempFile::new().unwrap();
        serde_json::to_writer(output_file.as_file(), &json)?;
        output_file.flush()?;
        output_file.rewind()?;
        Ok(output_file)
    }

    fn load_parameter_json(
        &self,
        source_path: Option<PathBuf>,
        protocol: Option<Protocol>,
        constants: Option<ProtocolConstants>,
    ) -> anyhow::Result<Value> {
        let raw_json: Value = match source_path {
            Some(path) => {
                let mut buffer = String::new();
                match std::fs::File::open(&path) {
                    Ok(mut f) => {
                        f.read_to_string(&mut buffer)?;
                        serde_json::from_slice(buffer.as_bytes())?
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Failed to open parameter file at {:?}: {}",
                            path,
                            e.to_string()
                        ))
                    }
                }
            }
            None => {
                let file_path = protocol
                    .unwrap_or_default()
                    .parameter_file(&constants.unwrap_or_default());
                let file_path_str = file_path.to_str().ok_or(anyhow::anyhow!(
                    "Failed to convert parameter file path to string"
                ))?;
                let f =
                    ProtocolParameterFile::get(file_path_str).ok_or(anyhow::anyhow!(
                        "Failed to load parameter file at '{}'",
                        file_path_str
                    ))?;
                serde_json::from_slice(&f.data)?
            }
        };
        Ok(raw_json)
    }
}

#[cfg(test)]
mod tests {
    use super::{Protocol, ProtocolConstants, ProtocolParameterBuilder};

    #[test]
    fn parameter_builder() {
        let mut builder = ProtocolParameterBuilder::new();
        builder
            .set_constants(ProtocolConstants::Sandbox)
            .set_protocol(Protocol::Alpha)
            .set_source_path("/test/path");
        assert_eq!(builder.constants.unwrap(), ProtocolConstants::Sandbox);
        assert_eq!(
            builder.source_path.unwrap().as_os_str().to_str().unwrap(),
            "/test/path"
        );
        assert_eq!(builder.protocol.unwrap().hash(), Protocol::Alpha.hash());
    }

    #[test]
    fn parameter_builder_default() {
        let mut builder = ProtocolParameterBuilder::new();
        // builder should be able to find the template file with default values
        // and write an output file, so we check if the result is ok here
        assert!(builder.build().is_ok());
    }

    #[test]
    fn build_parameters_from_given_file() {
        let mut builder = ProtocolParameterBuilder::new();
        let source_file = tempfile::NamedTempFile::new().unwrap();
        builder.set_source_path(source_file.path().to_str().unwrap());
        let json = serde_json::json!({"foo":"bar"});
        serde_json::to_writer(&source_file, &json).unwrap();

        let output_file = builder.build().unwrap();
        let json: serde_json::Value = serde_json::from_reader(output_file).unwrap();

        // this output file should have the values as the source file above
        assert_eq!(json.get("foo").unwrap().as_str().unwrap(), "bar");
    }
}
