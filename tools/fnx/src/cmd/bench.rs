use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use clap::Args;

use crate::nixutil;

/// Wall-clock benchmarks for the flutter2nix pipeline. Each target runs a cold
/// pass (fresh Gradle user home) immediately followed by a warm pass (same home,
/// build outputs wiped) — the warm number is the CI-with-cache scenario.
///
/// All mutable state (Gradle homes, fixture copies, lock output) lives in
/// tempfile::TempDir, which is deleted when the benchmark finishes — including
/// on failure. Gradle daemons are disabled so no background process outlives
/// the run holding cache directories open.
#[derive(Args)]
pub struct BenchArgs {
    /// Benchmark target: lock | gradle-build | flutter-build | ios-lock | ios-build | all
    #[arg(long, default_value = "all")]
    pub target: String,
}

struct BenchResult {
    name: &'static str,
    cold: Duration,
    warm: Duration,
}

pub fn run(args: BenchArgs) -> anyhow::Result<()> {
    let repo_root = nixutil::find_repo_root()?;

    let mut results: Vec<BenchResult> = Vec::new();
    let want = |t: &str| args.target == "all" || args.target == t;

    if want("lock") {
        results.push(bench_lock(&repo_root)?);
    }
    if want("gradle-build") {
        results.push(bench_gradle_build(&repo_root)?);
    }
    if want("flutter-build") {
        results.push(bench_flutter_build(&repo_root)?);
    }
    if want("ios-lock") {
        results.push(bench_ios_lock(&repo_root)?);
    }
    if want("ios-build") {
        // Needs real signing material — gated like the signing e2e: skipped
        // under `all` when the local env file is absent, hard error when
        // requested explicitly.
        match bench_ios_build(&repo_root)? {
            Some(r) => results.push(r),
            None if args.target == "all" => eprintln!(
                "fnx bench: ios-build skipped ({} not present or not macOS)",
                super::signing_e2e::SIGNING_ENV_FILE
            ),
            None => bail!(
                "ios-build needs signing material: create {} (see docs/ios-testing.md) on macOS",
                super::signing_e2e::SIGNING_ENV_FILE
            ),
        }
    }
    if results.is_empty() && args.target != "all" {
        bail!(
            "unknown bench target '{}' (expected lock | gradle-build | flutter-build | ios-lock | ios-build | all)",
            args.target
        );
    }
    if results.is_empty() {
        bail!("no benchmark ran — every target was skipped");
    }

    println!();
    println!("{:<16} {:>10} {:>10}", "target", "cold", "warm");
    for r in &results {
        println!(
            "{:<16} {:>9.1}s {:>9.1}s",
            r.name,
            r.cold.as_secs_f64(),
            r.warm.as_secs_f64()
        );
    }
    println!();
    println!("warm = CI-with-cache scenario (caches retained — Gradle home / resolve cache / DerivedData — build outputs wiped)");

    let report = write_reports(&repo_root, &results)?;
    println!("recorded: {} (+ history.jsonl)", report.display());
    Ok(())
}

/// Persist results under benchmarks/ (committed): one row per target appended
/// to BENCHMARKS.md and one JSON line per run appended to history.jsonl, so
/// timings can be compared across commits. Returns the Markdown path.
fn write_reports(repo_root: &Path, results: &[BenchResult]) -> anyhow::Result<PathBuf> {
    let bench_dir = repo_root.join("benchmarks");
    std::fs::create_dir_all(&bench_dir).context("creating benchmarks/")?;

    let timestamp =
        command_line(repo_root, "date", &["+%Y-%m-%dT%H:%M:%S%z"]).context("reading timestamp")?;
    let commit = describe_commit(repo_root);
    let host = command_line(repo_root, "uname", &["-nm"]).unwrap_or_else(|_| "unknown".into());

    use std::io::Write;

    // Fixed shape, no strings needing escaping — hand-rolled JSON keeps fnx dependency-free.
    let json_results: Vec<String> = results
        .iter()
        .map(|r| {
            format!(
                r#"{{"target":"{}","cold_secs":{:.1},"warm_secs":{:.1}}}"#,
                r.name,
                r.cold.as_secs_f64(),
                r.warm.as_secs_f64()
            )
        })
        .collect();
    let line = format!(
        r#"{{"timestamp":"{timestamp}","commit":"{commit}","host":"{host}","results":[{}]}}"#,
        json_results.join(",")
    );
    let history = bench_dir.join("history.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history)
        .with_context(|| format!("opening {}", history.display()))?;
    writeln!(file, "{line}").context("appending to history.jsonl")?;

    let md_path = bench_dir.join("BENCHMARKS.md");
    if !md_path.exists() {
        std::fs::write(
            &md_path,
            "# fnx bench history\n\n\
             Appended by `fnx bench`. cold = fresh Gradle user home; warm = same home with\n\
             build outputs wiped (the CI-with-cache scenario). Timings are machine-local —\n\
             the host is recorded per run in history.jsonl.\n\n\
             | date | commit | target | cold | warm |\n\
             |------|--------|--------|-----:|-----:|\n",
        )
        .with_context(|| format!("writing {}", md_path.display()))?;
    }
    let mut rows = String::new();
    for r in results {
        rows.push_str(&format!(
            "| {timestamp} | {commit} | {} | {:.1}s | {:.1}s |\n",
            r.name,
            r.cold.as_secs_f64(),
            r.warm.as_secs_f64()
        ));
    }
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&md_path)
        .with_context(|| format!("opening {}", md_path.display()))?;
    write!(file, "{rows}").context("appending to BENCHMARKS.md")?;

    Ok(md_path)
}

/// Current commit id for cross-run comparison: jj first (this repo's VCS),
/// git as a fallback for plain checkouts.
fn describe_commit(repo_root: &Path) -> String {
    command_line(
        repo_root,
        "jj",
        &["log", "--no-graph", "-r", "@", "-T", "commit_id.short()"],
    )
    .or_else(|_| command_line(repo_root, "git", &["rev-parse", "--short", "HEAD"]))
    .unwrap_or_else(|_| "unknown".into())
}

/// Run a command and return its first line of stdout, trimmed.
fn command_line(repo_root: &Path, program: &str, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("running {program}"))?;
    if !out.status.success() {
        bail!("{program} failed");
    }
    let line = String::from_utf8(out.stdout)?
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        bail!("{program} produced no output");
    }
    Ok(line)
}

/// `gradle2nix lock` against the Flutter fixture — the dependency-resolution
/// benchmark. Cold downloads the Gradle distribution and resolves every artifact;
/// warm reuses the temp Gradle home.
fn bench_lock(repo_root: &Path) -> anyhow::Result<BenchResult> {
    let bin = build_release_bin(repo_root, "gradle2nix")?;

    let gradle_home = tempfile::tempdir().context("creating temp gradle home")?;
    let out_dir = tempfile::tempdir().context("creating temp output dir")?;
    let project = repo_root.join("tests/fixtures/flutter/minimal-app/android");

    let run_lock = |label: &str| {
        let mut cmd = Command::new(&bin);
        cmd.arg("lock")
            .arg("--project-dir")
            .arg(&project)
            .arg("--output")
            .arg(out_dir.path().join("flutter2nix.lock"))
            .arg("--gradle-user-home")
            .arg(gradle_home.path())
            .args(["--timeout-secs", "600"]);
        run_timed(cmd, label)
    };

    let cold = run_lock("lock (cold)")?;
    let warm = run_lock("lock (warm)")?;
    Ok(BenchResult {
        name: "lock",
        cold,
        warm,
    })
}

/// Offline `gradle assembleRelease` of the pure-Gradle fixture against the
/// nix-built Maven repo — the plain Android CI build benchmark.
fn bench_gradle_build(repo_root: &Path) -> anyhow::Result<BenchResult> {
    require_on_path("gradle")?;
    let sdk = android_sdk_root()?;
    let init_script = nix_build_path(repo_root, ".#bench-init-script")?;

    let (_tmp, project) =
        copy_fixture(&repo_root.join("tests/fixtures/gradle/android-minimal-app"))?;
    let gradle_home = tempfile::tempdir().context("creating temp gradle home")?;
    write_gradle_home_properties(gradle_home.path(), &sdk)?;

    let build = |label: &str| {
        let mut cmd = Command::new("gradle");
        cmd.arg("-p")
            .arg(&project)
            .args([
                "assembleRelease",
                "--offline",
                "--no-daemon",
                "--no-configuration-cache",
            ])
            .arg("--init-script")
            .arg(&init_script)
            .env("GRADLE_USER_HOME", gradle_home.path());
        run_timed(cmd, label)
    };

    let cold = build("gradle-build (cold)")?;
    wipe_build_outputs(&project)?;
    let warm = build("gradle-build (warm)")?;
    Ok(BenchResult {
        name: "gradle-build",
        cold,
        warm,
    })
}

/// Full `flutter build appbundle --no-pub` of the Flutter fixture, offline —
/// the Flutter CI build + release benchmark (goal: warm < 60s).
fn bench_flutter_build(repo_root: &Path) -> anyhow::Result<BenchResult> {
    require_on_path("flutter")?;
    require_on_path("gradle")?;
    let sdk = android_sdk_root()?;
    let init_script = nix_build_path(repo_root, ".#bench-init-script")?;

    let fixture = repo_root.join("tests/fixtures/flutter/minimal-app");
    if !fixture.join("android/local.properties").exists() {
        bail!(
            "missing {}/android/local.properties — enter `nix develop` (the shellHook writes it)",
            fixture.display()
        );
    }

    let (_tmp, project) = copy_fixture(&fixture)?;

    // Dart package config: reuse the fixture's .dart_tool if it was copied;
    // otherwise run pub get once, untimed (the benchmark measures the build).
    if !project.join(".dart_tool/package_config.json").exists() {
        let status = Command::new("flutter")
            .args(["pub", "get"])
            .current_dir(&project)
            .status()
            .context("running flutter pub get")?;
        if !status.success() {
            bail!("flutter pub get failed in {}", project.display());
        }
    }

    // Same gradlew replacement buildFlutterAndroidApp performs in the sandbox.
    let gradlew = project.join("android/gradlew");
    std::fs::write(&gradlew, "#!/bin/sh\nexec gradle --offline \"$@\"\n")
        .context("writing gradlew")?;
    set_executable(&gradlew)?;

    let gradle_home = tempfile::tempdir().context("creating temp gradle home")?;
    let init_d = gradle_home.path().join("init.d");
    std::fs::create_dir_all(&init_d).context("creating init.d")?;
    std::fs::copy(&init_script, init_d.join("gradle2nix-bench.gradle"))
        .context("installing init script")?;
    write_gradle_home_properties(gradle_home.path(), &sdk)?;

    let build = |label: &str| {
        let mut cmd = Command::new("flutter");
        cmd.args(["build", "appbundle", "--no-pub"])
            .current_dir(&project)
            .env("GRADLE_USER_HOME", gradle_home.path());
        run_timed(cmd, label)
    };

    let cold = build("flutter-build (cold)")?;
    wipe_build_outputs(&project)?;
    let warm = build("flutter-build (warm)")?;
    Ok(BenchResult {
        name: "flutter-build",
        cold,
        warm,
    })
}

/// Path of the real-world-tier iOS fixture: a minimal native app wrapping the
/// same Firebase/Messaging pod tree as a production white-label PWA wrapper.
const PWA_WRAPPER_FIXTURE: &str = "crates/ios2nix/tests/fixtures/xcode-projects/pwa-wrapper-app";

/// `ios2nix lock` against the pwa-wrapper-app Podfile.lock (19 pods incl.
/// subspecs) — CocoaPods resolution + source hashing. Cold fetches every
/// podspec from the CDN and hashes every pod source (nix-prefetch-git); warm
/// reuses the resolve cache.
fn bench_ios_lock(repo_root: &Path) -> anyhow::Result<BenchResult> {
    require_on_path("nix-prefetch-git")?;
    let bin = build_release_bin(repo_root, "ios2nix")?;

    let ios_dir = tempfile::tempdir().context("creating temp ios dir")?;
    std::fs::copy(
        repo_root.join(PWA_WRAPPER_FIXTURE).join("Podfile.lock"),
        ios_dir.path().join("Podfile.lock"),
    )
    .context("copying bench Podfile.lock fixture")?;
    let cache_dir = tempfile::tempdir().context("creating temp cache dir")?;

    let run_lock = |label: &str| {
        let mut cmd = Command::new(&bin);
        cmd.arg("lock")
            .arg("--ios-dir")
            .arg(ios_dir.path())
            .arg("--output")
            .arg(ios_dir.path().join("ios2nix.lock"))
            .arg("--cache-dir")
            .arg(cache_dir.path())
            .args(["--timeout-secs", "600"]);
        run_timed(cmd, label)
    };

    let cold = run_lock("ios-lock (cold)")?;
    let warm = run_lock("ios-lock (warm)")?;
    Ok(BenchResult {
        name: "ios-lock",
        cold,
        warm,
    })
}

/// `pod install` + signed `ios2nix archive` + `ios2nix export` of the
/// pwa-wrapper-app fixture (real Firebase/Messaging pod tree) — the
/// time-to-signed-.ipa benchmark. Cold runs pod install and a fresh
/// DerivedData build; warm retains Pods/ and DerivedData with the
/// archive/export outputs wiped. Gated on the local signing env file (see
/// signing_e2e); returns Ok(None) when the gate is closed so `--target all`
/// can skip instead of fail.
fn bench_ios_build(repo_root: &Path) -> anyhow::Result<Option<BenchResult>> {
    let Some(vars) = super::signing_e2e::load_signing_vars(repo_root)? else {
        return Ok(None);
    };
    require_on_path("xcodebuild")?;
    require_on_path("pod")?;
    let bin = build_release_bin(repo_root, "ios2nix")?;

    // Temp keychain + identity import + profile install; stdout is the
    // keychain path. Untimed — the benchmark measures the build, not the
    // one-off signing setup.
    let setup_out = {
        let mut cmd = Command::new(&bin);
        cmd.arg("sign-setup").current_dir(repo_root);
        for (key, value) in &vars {
            cmd.env(key, value);
        }
        cmd.output().context("running ios2nix sign-setup")?
    };
    if !setup_out.status.success() {
        bail!(
            "ios2nix sign-setup failed:\n{}",
            String::from_utf8_lossy(&setup_out.stderr)
        );
    }
    let keychain = PathBuf::from(String::from_utf8(setup_out.stdout)?.trim());
    let _keychain_guard = KeychainGuard(keychain.clone());

    let team_id = vars["IOS2NIX_TEAM_ID"].clone();
    let identity = vars["IOS2NIX_SIGNING_IDENTITY"].clone();
    let method = vars
        .get("IOS2NIX_EXPORT_METHOD")
        .cloned()
        .unwrap_or_else(|| "ad-hoc".to_string());
    let profile = profile_metadata(Path::new(&vars["IOS2NIX_PROFILE_PATH"]))?;

    let (_tmp, project) = copy_fixture(&repo_root.join(PWA_WRAPPER_FIXTURE))?;
    // Pods/ is gitignored but may exist in a local checkout that ran pod
    // install — wipe it from the copy so the cold pass is genuinely cold.
    let pods_dir = project.join("Pods");
    if pods_dir.exists() {
        std::fs::remove_dir_all(&pods_dir).context("removing copied Pods dir")?;
    }

    // Stamp the profile's App ID into the app target's pbxproj (themer-style).
    // A command-line PRODUCT_BUNDLE_IDENTIFIER override would hit every target
    // including the pod frameworks, whose IDs then collide with the
    // provisioningProfiles map and fail the export with "<framework> does not
    // support provisioning profiles".
    let pbxproj = project.join("PwaWrapperApp.xcodeproj/project.pbxproj");
    let stamped = std::fs::read_to_string(&pbxproj)
        .context("reading fixture pbxproj")?
        .replace("com.example.pwawrapperapp", &profile.bundle_id);
    std::fs::write(&pbxproj, stamped).context("stamping bundle id into fixture pbxproj")?;

    let derived_data = tempfile::tempdir().context("creating temp DerivedData dir")?;
    let export_opts = project.join("BenchExportOptions.plist");
    write_export_options_plist(&export_opts, &method, &team_id, &identity, &profile)?;

    let archive_path = project.join("out.xcarchive");
    let ipa_dir = project.join("ipa");

    let pass = |phase: &str| -> anyhow::Result<Duration> {
        let mut archive = Command::new(&bin);
        archive
            .arg("archive")
            .arg("--workspace")
            .arg(project.join("PwaWrapperApp.xcworkspace"))
            .args(["--scheme", "PwaWrapperApp"])
            .arg("--archive-path")
            .arg(&archive_path)
            .args(["--team-id", &team_id])
            .args(["--signing-identity", &identity])
            .args(["--profile-specifier", &profile.name])
            .arg("--keychain")
            .arg(&keychain)
            .arg("--derived-data")
            .arg(derived_data.path());
        let archive_time = run_timed(archive, &format!("ios-build archive ({phase})"))?;

        let mut export = Command::new(&bin);
        export
            .arg("export")
            .arg("--archive-path")
            .arg(&archive_path)
            .arg("--export-opts-plist")
            .arg(&export_opts)
            .arg("--output-path")
            .arg(&ipa_dir);
        let export_time = run_timed(export, &format!("ios-build export ({phase})"))?;

        Ok(archive_time + export_time)
    };

    // pod install belongs to the cold pass: a cache-less CI runner pays it.
    let mut pod_install = Command::new("pod");
    pod_install.arg("install").current_dir(&project);
    let pods_time = run_timed(pod_install, "ios-build pod install (cold)")?;

    let cold = pods_time + pass("cold")?;
    // CI-warm semantics: retain Pods/ and DerivedData, wipe what the pass produced.
    for output in [&archive_path, &ipa_dir] {
        if output.exists() {
            std::fs::remove_dir_all(output)
                .with_context(|| format!("removing {}", output.display()))?;
        }
    }
    let warm = pass("warm")?;

    Ok(Some(BenchResult {
        name: "ios-build",
        cold,
        warm,
    }))
}

/// Deletes the sign-setup temp keychain (which also drops its search-list
/// entry) when the benchmark finishes, including on failure.
struct KeychainGuard(PathBuf);

impl Drop for KeychainGuard {
    fn drop(&mut self) {
        let _ = Command::new("security")
            .arg("delete-keychain")
            .arg(&self.0)
            .output();
    }
}

struct ProfileMetadata {
    uuid: String,
    name: String,
    bundle_id: String,
}

/// Decode the CMS-signed profile and read the fields the signed build needs.
/// Shell-based (security + PlistBuddy) — fnx stays dependency-free.
fn profile_metadata(profile: &Path) -> anyhow::Result<ProfileMetadata> {
    let decoded = tempfile::NamedTempFile::new().context("creating temp plist")?;
    let status = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile)
        .arg("-o")
        .arg(decoded.path())
        .status()
        .context("running security cms -D")?;
    if !status.success() {
        bail!("security cms -D failed on {}", profile.display());
    }

    let read = |key: &str| -> anyhow::Result<String> {
        let out = Command::new("/usr/libexec/PlistBuddy")
            .arg("-c")
            .arg(format!("Print :{key}"))
            .arg(decoded.path())
            .output()
            .context("running PlistBuddy")?;
        if !out.status.success() {
            bail!("PlistBuddy failed reading :{key} from decoded profile");
        }
        Ok(String::from_utf8(out.stdout)?.trim().to_string())
    };

    let uuid = read("UUID")?;
    let name = read("Name")?;
    let app_id = read("Entitlements:application-identifier")?;
    let bundle_id = app_id
        .split_once('.')
        .map(|(_, bundle)| bundle.to_string())
        .context("application-identifier missing TEAMID. prefix")?;

    Ok(ProfileMetadata {
        uuid,
        name,
        bundle_id,
    })
}

/// Manual-signing ExportOptions.plist. Fixed shape, hand-rolled like the
/// JSON in write_reports — keeps fnx dependency-free.
fn write_export_options_plist(
    path: &Path,
    method: &str,
    team_id: &str,
    identity: &str,
    profile: &ProfileMetadata,
) -> anyhow::Result<()> {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>{}</string>
  <key>teamID</key><string>{}</string>
  <key>signingStyle</key><string>manual</string>
  <key>signingCertificate</key><string>{}</string>
  <key>provisioningProfiles</key><dict>
    <key>{}</key><string>{}</string>
  </dict>
  <key>destination</key><string>export</string>
  <key>stripSwiftSymbols</key><true/>
  <key>compileBitcode</key><false/>
</dict></plist>
"#,
        esc(method),
        esc(team_id),
        esc(identity),
        esc(&profile.bundle_id),
        esc(&profile.uuid),
    );
    std::fs::write(path, plist).with_context(|| format!("writing {}", path.display()))
}

/// Build a workspace binary in release mode, untimed (compilation must never
/// pollute a measurement), and return its path.
fn build_release_bin(repo_root: &Path, package: &str) -> anyhow::Result<PathBuf> {
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", package])
        .current_dir(repo_root)
        .status()
        .context("running cargo build")?;
    if !status.success() {
        bail!("cargo build --release -p {package} failed");
    }
    Ok(repo_root.join("target/release").join(package))
}

fn run_timed(mut cmd: Command, label: &str) -> anyhow::Result<Duration> {
    eprintln!("fnx bench: {label}...");
    let start = Instant::now();
    let status = cmd.status().with_context(|| format!("spawning {label}"))?;
    let elapsed = start.elapsed();
    if !status.success() {
        bail!("{label} failed with {status}");
    }
    eprintln!("fnx bench: {label} took {:.1}s", elapsed.as_secs_f64());
    Ok(elapsed)
}

/// Copy a fixture into a TempDir. Returns the dir (keep it alive — dropping
/// deletes the copy) and the path of the copied project inside it.
fn copy_fixture(src: &Path) -> anyhow::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir().context("creating temp dir")?;
    let status = Command::new("cp")
        .arg("-a")
        .arg(src)
        .arg(dir.path())
        .status()
        .context("copying fixture")?;
    if !status.success() {
        bail!("cp -a {} failed", src.display());
    }
    let name = src.file_name().context("fixture path has no file name")?;
    let project = dir.path().join(name);
    Ok((dir, project))
}

/// CI-warm semantics: keep the Gradle user home, wipe everything the build wrote
/// into the project copy.
fn wipe_build_outputs(project: &Path) -> anyhow::Result<()> {
    for rel in [
        "build",
        "app/build",
        ".gradle",
        "android/.gradle",
        "android/app/build",
    ] {
        let p = project.join(rel);
        if p.exists() {
            std::fs::remove_dir_all(&p).with_context(|| format!("removing {}", p.display()))?;
        }
    }
    Ok(())
}

/// aapt2 override (Maven binary cannot exec on NixOS-style hosts), in-process
/// Kotlin (no compile daemon), and no Gradle daemon (nothing outlives the bench).
fn write_gradle_home_properties(gradle_home: &Path, sdk: &Path) -> anyhow::Result<()> {
    let aapt2 = find_aapt2(sdk)?;
    std::fs::write(
        gradle_home.join("gradle.properties"),
        format!(
            "android.aapt2FromMavenOverride={}\n\
             kotlin.compiler.execution.strategy=in-process\n\
             org.gradle.daemon=false\n",
            aapt2.display()
        ),
    )
    .context("writing gradle.properties")
}

fn find_aapt2(sdk: &Path) -> anyhow::Result<PathBuf> {
    let build_tools = sdk.join("build-tools");
    for entry in std::fs::read_dir(&build_tools)
        .with_context(|| format!("reading {}", build_tools.display()))?
    {
        let candidate = entry?.path().join("aapt2");
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("no aapt2 found under {}", build_tools.display())
}

fn android_sdk_root() -> anyhow::Result<PathBuf> {
    for var in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Ok(v) = std::env::var(var) {
            return Ok(PathBuf::from(v));
        }
    }
    bail!("ANDROID_SDK_ROOT/ANDROID_HOME not set — run inside `nix develop`")
}

fn require_on_path(tool: &str) -> anyhow::Result<()> {
    let found = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(tool).is_file()))
        .unwrap_or(false);
    if !found {
        bail!("'{tool}' not found on PATH — run inside `nix develop`");
    }
    Ok(())
}

fn nix_build_path(repo_root: &Path, attr: &str) -> anyhow::Result<PathBuf> {
    let out = Command::new("nix")
        .args(["build", "--no-link", "--print-out-paths", attr])
        .current_dir(repo_root)
        .output()
        .context("running nix build")?;
    if !out.status.success() {
        bail!(
            "nix build {attr} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(PathBuf::from(String::from_utf8(out.stdout)?.trim()))
}

fn set_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("chmod +x {}", path.display()))
}
