use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use camino::Utf8Path as Path;
use camino::Utf8PathBuf as PathBuf;
use std::env;
use std::fs;
use std::process::Command;
use std::thread;
use tracing::info;

#[cfg(target_os = "macos")]
const FFMPEG_DLL: &str = "libffmpeg.dylib";
#[cfg(target_os = "linux")]
const FFMPEG_DLL: &str = "libffmpeg.so";
#[cfg(target_os = "windows")]
const FFMPEG_DLL: &str = "libffmpeg.dll";

#[cfg(target_os = "macos")]
const LD_PATH: &str = "DYLD_LIBRARY_PATH";
#[cfg(target_os = "linux")]
const LD_PATH: &str = "LD_LIBRARY_PATH";
#[cfg(target_os = "windows")]
const PATH: &str = "PATH";

fn clone_ffmpeg(target_path: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("clone")
        .arg("--single-branch")
        .arg("--branch")
        .arg("release/5.0")
        .arg("--depth")
        .arg("1")
        .arg("https://github.com/ffmpeg/ffmpeg")
        .arg(target_path)
        .status()?;
    if !status.success() {
        bail!("Make install failed.");
    }

    Ok(())
}

fn build_ffmpeg(ffmpeg_path: &Path, ffmpeg_build_path: &Path) -> Result<()> {
    let status = Command::new("./configure")
        .current_dir(ffmpeg_path)
        .arg(format!("--prefix={}", ffmpeg_build_path))
        .arg("--disable-hwaccels")
        .arg("--disable-encoders")
        .arg("--enable-encoder=flac,aac,opus,pcm_f32le,alac,wmav2,libmp3lame")
        .arg("--disable-decoders")
        .arg("--enable-decoder=flac,aac,opus,pcm_f32le,alac,wmav2,mp3")
        .arg("--disable-parsers")
        .arg("--disable-protocols")
        .arg("--disable-bsfs")
        .arg("--disable-indevs")
        .arg("--disable-outdevs")
        .arg("--disable-filters")
        .arg("--disable-programs")
        .arg("--enable-protocol=file")
        .arg("--enable-libmp3lame")
        .status()
        .context("Configure failed")?;
    if !status.success() {
        bail!("Configure failed.");
    }

    let num_cpus = thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(4);

    let status = Command::new("make")
        .current_dir(ffmpeg_path)
        .arg(format!("-j{}", num_cpus))
        .status()
        .context("Make failed")?;
    if !status.success() {
        bail!("Make failed.");
    }

    let status = Command::new("make")
        .current_dir(ffmpeg_path)
        .arg("install")
        .status()
        .context("Make install failed")?;
    if !status.success() {
        bail!("Make install failed.");
    }

    let status = if cfg!(target_os = "macos") {
        Command::new("clang")
            .arg("-L/opt/homebrew/lib")
            .arg("-I/opt/homebrew/include")
            .current_dir(ffmpeg_build_path.join("lib"))
            .arg("-shared")
            .arg("-o")
            .arg(FFMPEG_DLL)
            .arg("-Wl,-all_load")
            .arg("libavcodec.a")
            .arg("libavdevice.a")
            .arg("libavfilter.a")
            .arg("libavformat.a")
            .arg("libavutil.a")
            .arg("libswresample.a")
            .arg("libswscale.a")
            .arg("-framework")
            .arg("VideoToolbox")
            .arg("-framework")
            .arg("AudioToolbox")
            .arg("-framework")
            .arg("CoreFoundation")
            .arg("-framework")
            .arg("CoreVideo")
            .arg("-framework")
            .arg("CoreMedia")
            .arg("-lmp3lame")
            .arg("-lz")
            .arg("-liconv")
            .arg("-lbz2")
            .status()
    } else if cfg!(target_os = "linux") {
        Command::new("gcc")
            .current_dir(ffmpeg_build_path.join("lib"))
            .arg("-shared")
            .arg("-o")
            .arg(FFMPEG_DLL)
            .arg("-Wl,--whole-archive")
            .arg("libavcodec.a")
            .arg("libavdevice.a")
            .arg("libavfilter.a")
            .arg("libavformat.a")
            .arg("libavutil.a")
            .arg("libswresample.a")
            .arg("libswscale.a")
            .arg("-Wl,--no-whole-archive")
            .arg("-Wl,-Bsymbolic")
            .arg("-lmp3lame")
            .status()
    } else {
        bail!(
            "Build FFmpeg dynamic lib on this platform is harder than you think.\
            Please use prebuilt dylib or following the instructions to build it on you own."
        );
    }
    .context("Build shared lib failed")?;

    if !status.success() {
        bail!("Build shared lib failed.");
    }

    Ok(())
}

fn main() -> Result<()> {
    let color = supports_color::on(supports_color::Stream::Stdout).is_some()
        && supports_color::on(supports_color::Stream::Stderr).is_some();
    tracing_subscriber::fmt()
        .with_ansi(color)
        .with_env_filter("info")
        .init();

    let cwd = Path::new(".");
    let ffmpeg_custom_path = Path::new("target/ffmpeg_build");

    let ffmpeg_path = Path::new("target/ffmpeg");
    let ffmpeg_prebuilt_path = cwd.join("prebuilt_ffmpeg");

    let ffmpeg_configure_path = ffmpeg_path.join("configure");

    let ffmpeg_custom_include_path = ffmpeg_custom_path.join("include");
    let ffmpeg_custom_lib_path = ffmpeg_custom_path.join("lib");
    let ffmpeg_custom_dll_path = ffmpeg_custom_lib_path.join(FFMPEG_DLL);

    let ffmpeg_prebuilt_include_path = ffmpeg_prebuilt_path.join("include");
    let ffmpeg_prebuilt_lib_path = ffmpeg_prebuilt_path.join("lib");
    let ffmpeg_prebuilt_dll_path = ffmpeg_prebuilt_lib_path.join(FFMPEG_DLL);

    let (ffmpeg_include_path, ffmpeg_lib_path, ffmpeg_dll_path) = if !ffmpeg_prebuilt_dll_path
        .exists()
    {
        fs::create_dir_all(&ffmpeg_path).context("Create ffmpeg source directory failed.")?;
        fs::create_dir_all(&ffmpeg_custom_path).context("Create ffmpeg build directory failed.")?;
        let ffmpeg_path = PathBuf::from_path_buf(ffmpeg_path.canonicalize().unwrap()).unwrap();
        let ffmpeg_custom_path =
            PathBuf::from_path_buf(ffmpeg_custom_path.canonicalize().unwrap()).unwrap();
        if !ffmpeg_configure_path.exists() {
            info!("Cloning ffmpeg to {}...", ffmpeg_path);
            clone_ffmpeg(&ffmpeg_path).context("Clone ffmpeg failed.")?;
        }
        info!("FFmpeg repo cloned.");

        if !ffmpeg_custom_dll_path.exists() {
            info!("Building ffmpeg to {}...", ffmpeg_custom_path);
            build_ffmpeg(&ffmpeg_path, &ffmpeg_custom_path).context("Build ffmpeg failed.")?;
        }
        info!("FFmpeg already built.");
        (
            &ffmpeg_custom_include_path,
            &ffmpeg_custom_lib_path,
            &ffmpeg_custom_dll_path,
        )
    } else {
        info!("Use prebuilt FFmpeg: {}", ffmpeg_prebuilt_dll_path);
        (
            &ffmpeg_prebuilt_include_path,
            &ffmpeg_prebuilt_lib_path,
            &ffmpeg_prebuilt_dll_path,
        )
    };

    let (ffmpeg_include_path, ffmpeg_lib_path, ffmpeg_dll_path) = (
        PathBuf::from_path_buf(ffmpeg_include_path.canonicalize().unwrap()).unwrap(),
        PathBuf::from_path_buf(ffmpeg_lib_path.canonicalize().unwrap()).unwrap(),
        PathBuf::from_path_buf(ffmpeg_dll_path.canonicalize().unwrap()).unwrap(),
    );

    let mut envs = vec![];
    envs.push(("FFMPEG_INCLUDE_DIR", ffmpeg_include_path.to_string()));
    envs.push(("FFMPEG_DLL_PATH", ffmpeg_dll_path.to_string()));
    #[cfg(not(windows))]
    {
        envs.push((LD_PATH, ffmpeg_lib_path.to_string()));
    }
    #[cfg(windows)]
    {
        envs.push((
            PATH,
            [ffmpeg_lib_path.into_string(), std::env::var(PATH).unwrap()].join(";"),
        ));
    }

    let args: Vec<_> = env::args_os().collect();
    Command::new("cargo").args(&args[1..]).envs(envs).status()?;

    Ok(())
}
