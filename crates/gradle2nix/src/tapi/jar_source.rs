use std::io::Write;
use std::path::PathBuf;

pub enum TapiShimSource {
    Embedded(&'static [u8]),
    EnvPath(PathBuf),
}

pub fn select_tapi_shim_source() -> anyhow::Result<TapiShimSource> {
    if let Ok(path_str) = std::env::var("GRADLE2NIX_TAPI_SHIM_PATH") {
        let path = PathBuf::from(&path_str);
        anyhow::ensure!(
            path.exists(),
            "GRADLE2NIX_TAPI_SHIM_PATH={} not found",
            path_str
        );
        return Ok(TapiShimSource::EnvPath(path));
    }
    Ok(TapiShimSource::Embedded(include_bytes!(
        "../../../../tapi-shim/build/libs/tapi-shim.jar"
    )))
}

pub fn extract_jar_to_temp(source: TapiShimSource) -> anyhow::Result<PathBuf> {
    match source {
        TapiShimSource::Embedded(bytes) => {
            let mut temp = tempfile::Builder::new()
                .prefix("gradle2nix-tapi-shim-")
                .suffix(".jar")
                .tempfile()?;
            temp.write_all(bytes)?;
            let (_, path) = temp.keep()?;
            Ok(path)
        }
        TapiShimSource::EnvPath(path) => Ok(path),
    }
}
