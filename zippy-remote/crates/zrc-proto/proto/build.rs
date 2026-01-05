use std::io::Result;

fn main() -> Result<()> {
    // Re-run if proto changes
    println!("cargo:rerun-if-changed=zrc_v1.proto");

    let mut config = prost_build::Config::new();

    // Enable proto3 optional fields
    config.protoc_arg("--experimental_allow_proto3_optional");

    // Add serde derives when the serde feature is enabled
    #[cfg(feature = "serde")]
    {
        config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
        config.type_attribute(".", "#[serde(rename_all = \"camelCase\")]");
    }

    // Compile the proto file
    config.compile_protos(&["zrc_v1.proto"], &["."])?;

    Ok(())
}
