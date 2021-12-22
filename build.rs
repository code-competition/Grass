fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("./Protobufs/sandbox.proto")?;

    Ok(())
}
