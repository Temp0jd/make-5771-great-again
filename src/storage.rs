use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{LogEntry, LogLevel, MacroProfile};

const PACKAGE_FORMAT: &str = "make5771.workflow-package";
const PACKAGE_VERSION: u32 = 1;
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
const MAX_ASSETS: usize = 500;

pub fn default_profile_path() -> PathBuf {
    PathBuf::from("profiles/default.m5771.json")
}

pub const PROFILE_SUFFIX: &str = ".m5771.json";

/// Lists all saved workflow profiles in `profiles/`, sorted by file name.
pub fn list_profiles() -> Vec<PathBuf> {
    list_profiles_in(Path::new("profiles"))
}

fn list_profiles_in(dir: &Path) -> Vec<PathBuf> {
    let mut profiles = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(PROFILE_SUFFIX))
            {
                profiles.push(path);
            }
        }
    }
    profiles.sort();
    profiles
}

/// Human-readable profile name: the file name without the `.m5771.json` suffix.
pub fn profile_display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| {
            name.to_string_lossy()
                .trim_end_matches(PROFILE_SUFFIX)
                .to_owned()
        })
        .unwrap_or_else(|| path.display().to_string())
}

/// Appends one log line to `logs/<date>.log`; failures are non-fatal.
pub fn append_log(entry: &LogEntry) -> io::Result<()> {
    use std::io::Write;
    fs::create_dir_all("logs")?;
    let file_name = format!("logs/{}.log", chrono::Local::now().format("%Y-%m-%d"));
    let level = match entry.level {
        LogLevel::Info => "信息",
        LogLevel::Success => "成功",
        LogLevel::Warning => "警告",
    };
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_name)?;
    writeln!(file, "{} [{}] {}", entry.time, level, entry.message)
}

pub fn load_profile(path: &Path) -> Result<MacroProfile, StorageError> {
    let contents = fs::read_to_string(path).map_err(StorageError::Read)?;
    let profile: MacroProfile = serde_json::from_str(&contents).map_err(StorageError::Decode)?;
    profile.validate().map_err(StorageError::Validation)?;
    Ok(profile)
}

pub fn save_profile(path: &Path, profile: &MacroProfile) -> Result<(), StorageError> {
    profile.validate().map_err(StorageError::Validation)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(StorageError::Write)?;
    }
    let contents = serde_json::to_string_pretty(profile).map_err(StorageError::Encode)?;
    fs::write(path, contents).map_err(StorageError::Write)
}

#[derive(Debug, Clone)]
pub struct PackageSummary {
    pub profile_name: String,
    pub template_count: usize,
    pub author: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkflowPackage {
    format: String,
    format_version: u32,
    app_version: String,
    created_at: String,
    profile: MacroProfile,
    assets: Vec<PackageAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageAsset {
    key: String,
    byte_length: usize,
    data_base64: String,
}

pub fn export_workflow_package(
    path: &Path,
    profile: &MacroProfile,
) -> Result<PackageSummary, PackageError> {
    profile
        .validate()
        .map_err(|issues| PackageError::Invalid(issues.join("；")))?;
    if profile.templates.len() > MAX_ASSETS {
        return Err(PackageError::Invalid(format!(
            "模板数量超过上限 {MAX_ASSETS}"
        )));
    }

    let mut packaged_profile = profile.clone();
    let mut remap = HashMap::new();
    let mut original_paths = HashSet::new();
    let mut assets = Vec::with_capacity(profile.templates.len());
    for template in &profile.templates {
        if !original_paths.insert(template.path.clone()) {
            return Err(PackageError::Invalid(format!(
                "多个模板引用了同一文件：{}",
                template.path
            )));
        }
        let image = image::open(&template.path).map_err(|error| {
            PackageError::Invalid(format!("无法读取模板“{}”：{error}", template.name))
        })?;
        if image.width() != template.width || image.height() != template.height {
            return Err(PackageError::Invalid(format!(
                "模板“{}”的记录尺寸与图片不一致",
                template.name
            )));
        }
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|error| PackageError::Invalid(format!("模板编码失败：{error}")))?;
        let bytes = encoded.into_inner();
        if bytes.len() > MAX_ASSET_BYTES {
            return Err(PackageError::Invalid(format!(
                "模板“{}”超过 {} MiB 上限",
                template.name,
                MAX_ASSET_BYTES / 1024 / 1024
            )));
        }
        let key = format!("assets/template-{}.png", template.id);
        remap.insert(template.path.clone(), key.clone());
        assets.push(PackageAsset {
            key,
            byte_length: bytes.len(),
            data_base64: encode_base64(&bytes),
        });
    }
    remap_profile_paths(&mut packaged_profile, &remap)?;

    let package = WorkflowPackage {
        format: PACKAGE_FORMAT.to_owned(),
        format_version: PACKAGE_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: chrono::Local::now().to_rfc3339(),
        profile: packaged_profile,
        assets,
    };
    let contents = serde_json::to_vec_pretty(&package)
        .map_err(|error| PackageError::Encode(error.to_string()))?;
    if contents.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PackageError::Invalid("分享包超过 128 MiB 上限".to_owned()));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(PackageError::Write)?;
    }
    fs::write(path, contents).map_err(PackageError::Write)?;

    Ok(PackageSummary {
        profile_name: profile.name.clone(),
        template_count: profile.templates.len(),
        author: profile.sharing.author.clone(),
    })
}

pub fn import_workflow_package(
    path: &Path,
) -> Result<(MacroProfile, PackageSummary), PackageError> {
    import_workflow_package_to(path, Path::new("imports"))
}

fn import_workflow_package_to(
    path: &Path,
    import_root: &Path,
) -> Result<(MacroProfile, PackageSummary), PackageError> {
    let size = fs::metadata(path).map_err(PackageError::Read)?.len();
    if size > MAX_PACKAGE_BYTES {
        return Err(PackageError::Invalid("分享包超过 128 MiB 上限".to_owned()));
    }
    let contents = fs::read(path).map_err(PackageError::Read)?;
    let package: WorkflowPackage = serde_json::from_slice(&contents)
        .map_err(|error| PackageError::Decode(error.to_string()))?;
    if package.format != PACKAGE_FORMAT {
        return Err(PackageError::Invalid("不是 Make 5771 流程包".to_owned()));
    }
    if package.format_version != PACKAGE_VERSION {
        return Err(PackageError::Invalid(format!(
            "不支持分享包版本 {}，当前只支持版本 {PACKAGE_VERSION}",
            package.format_version
        )));
    }
    if package.assets.len() > MAX_ASSETS {
        return Err(PackageError::Invalid(format!(
            "模板数量超过上限 {MAX_ASSETS}"
        )));
    }
    package
        .profile
        .validate()
        .map_err(|issues| PackageError::Invalid(issues.join("；")))?;

    let mut asset_map = HashMap::new();
    for asset in package.assets {
        if asset.byte_length > MAX_ASSET_BYTES {
            return Err(PackageError::Invalid(format!(
                "包内资源“{}”超过大小上限",
                asset.key
            )));
        }
        if asset_map.insert(asset.key.clone(), asset).is_some() {
            return Err(PackageError::Invalid("分享包包含重复资源标识".to_owned()));
        }
    }
    if asset_map.len() != package.profile.templates.len() {
        return Err(PackageError::Invalid(
            "分享包的模板清单与图片资源数量不一致".to_owned(),
        ));
    }

    let mut decoded_images = Vec::with_capacity(package.profile.templates.len());
    let mut total_decoded = 0_usize;
    for template in &package.profile.templates {
        let asset = asset_map
            .remove(&template.path)
            .ok_or_else(|| PackageError::Invalid(format!("模板“{}”缺少图片资源", template.name)))?;
        let bytes = decode_base64(&asset.data_base64)
            .map_err(|error| PackageError::Decode(format!("模板 Base64 无效：{error}")))?;
        if bytes.len() != asset.byte_length || bytes.len() > MAX_ASSET_BYTES {
            return Err(PackageError::Invalid(format!(
                "模板“{}”的资源长度不合法",
                template.name
            )));
        }
        total_decoded = total_decoded.saturating_add(bytes.len());
        if total_decoded as u64 > MAX_PACKAGE_BYTES {
            return Err(PackageError::Invalid(
                "解码后的模板总大小超过上限".to_owned(),
            ));
        }
        let mut reader =
            image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|error| {
            PackageError::Invalid(format!("模板“{}”不是有效 PNG：{error}", template.name))
        })?;
        if image.width() != template.width || image.height() != template.height {
            return Err(PackageError::Invalid(format!(
                "模板“{}”的申明尺寸与图片不一致",
                template.name
            )));
        }
        decoded_images.push((template.id, template.path.clone(), image));
    }
    if !asset_map.is_empty() {
        return Err(PackageError::Invalid(
            "分享包包含未声明的额外资源".to_owned(),
        ));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| PackageError::Invalid(error.to_string()))?
        .as_nanos();
    let destination = import_root.join(format!("package-{timestamp}"));
    let template_dir = destination.join("templates");
    fs::create_dir_all(&template_dir).map_err(PackageError::Write)?;

    let write_result = (|| -> Result<HashMap<String, String>, PackageError> {
        let mut remap = HashMap::new();
        for (id, key, image) in decoded_images {
            let image_path = template_dir.join(format!("template-{id}.png"));
            image
                .save_with_format(&image_path, image::ImageFormat::Png)
                .map_err(|error| PackageError::Write(io::Error::other(error)))?;
            remap.insert(key, image_path.to_string_lossy().into_owned());
        }
        Ok(remap)
    })();
    let remap = match write_result {
        Ok(remap) => remap,
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
    };

    let mut profile = package.profile;
    if let Err(error) = remap_profile_paths(&mut profile, &remap) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    let summary = PackageSummary {
        profile_name: profile.name.clone(),
        template_count: profile.templates.len(),
        author: profile.sharing.author.clone(),
    };
    Ok((profile, summary))
}

fn remap_profile_paths(
    profile: &mut MacroProfile,
    remap: &HashMap<String, String>,
) -> Result<(), PackageError> {
    for step in &mut profile.steps {
        remap_optional_path(&mut step.template, remap)?;
        for branch in &mut step.branches {
            remap_optional_path(&mut branch.trigger_template, remap)?;
            for action in &mut branch.actions {
                remap_optional_path(&mut action.template, remap)?;
            }
        }
    }
    for template in &mut profile.templates {
        template.path = remap.get(&template.path).cloned().ok_or_else(|| {
            PackageError::Invalid(format!("模板“{}”的路径无法映射", template.name))
        })?;
    }
    Ok(())
}

fn remap_optional_path(
    path: &mut Option<String>,
    remap: &HashMap<String, String>,
) -> Result<(), PackageError> {
    if let Some(current) = path {
        *current = remap
            .get(current)
            .cloned()
            .ok_or_else(|| PackageError::Invalid(format!("流程引用了未打包的模板：{current}")))?;
    }
    Ok(())
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let bytes = input.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err("长度不是 4 的倍数".to_owned());
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let last = index + 1 == bytes.len() / 4;
        let first = decode_base64_value(chunk[0])?;
        let second = decode_base64_value(chunk[1])?;
        let third_padding = chunk[2] == b'=';
        let fourth_padding = chunk[3] == b'=';
        if (third_padding || fourth_padding) && !last {
            return Err("填充字符只能出现在末尾".to_owned());
        }
        if third_padding && !fourth_padding {
            return Err("填充字符顺序无效".to_owned());
        }
        let third = if third_padding {
            0
        } else {
            decode_base64_value(chunk[2])?
        };
        let fourth = if fourth_padding {
            0
        } else {
            decode_base64_value(chunk[3])?
        };
        if third_padding && second & 0x0f != 0 {
            return Err("非零填充位".to_owned());
        }
        if fourth_padding && !third_padding && third & 0x03 != 0 {
            return Err("非零填充位".to_owned());
        }
        output.push((first << 2) | (second >> 4));
        if !third_padding {
            output.push((second << 4) | (third >> 2));
        }
        if !fourth_padding {
            output.push((third << 6) | fourth);
        }
    }
    Ok(output)
}

fn decode_base64_value(value: u8) -> Result<u8, String> {
    match value {
        b'A'..=b'Z' => Ok(value - b'A'),
        b'a'..=b'z' => Ok(value - b'a' + 26),
        b'0'..=b'9' => Ok(value - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(format!("包含非法字符 0x{value:02x}")),
    }
}

#[derive(Debug)]
pub enum PackageError {
    Read(io::Error),
    Write(io::Error),
    Decode(String),
    Encode(String),
    Invalid(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "读取分享包失败：{error}"),
            Self::Write(error) => write!(formatter, "写入分享包失败：{error}"),
            Self::Decode(error) => write!(formatter, "解析分享包失败：{error}"),
            Self::Encode(error) => write!(formatter, "生成分享包失败：{error}"),
            Self::Invalid(error) => write!(formatter, "分享包无效：{error}"),
        }
    }
}

impl std::error::Error for PackageError {}

#[derive(Debug)]
pub enum StorageError {
    Read(io::Error),
    Write(io::Error),
    Decode(serde_json::Error),
    Encode(serde_json::Error),
    Validation(Vec<String>),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "读取失败：{error}"),
            Self::Write(error) => write!(formatter, "写入失败：{error}"),
            Self::Decode(error) => write!(formatter, "文件格式错误：{error}"),
            Self::Encode(error) => write!(formatter, "序列化失败：{error}"),
            Self::Validation(issues) => write!(formatter, "{}", issues.join("；")),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TemplateAsset;

    #[test]
    fn profile_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "m5771-profile-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let expected = MacroProfile::default();

        save_profile(&path, &expected).expect("profile should save");
        let actual = load_profile(&path).expect("profile should load");
        let _ = fs::remove_file(path);

        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.steps.len(), expected.steps.len());
        assert_eq!(actual.loop_count, expected.loop_count);
    }

    #[test]
    fn list_profiles_only_includes_profile_files() {
        let root = std::env::temp_dir().join(format!("m5771-profiles-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("b.m5771.json"), "{}").unwrap();
        fs::write(root.join("a.m5771.json"), "{}").unwrap();
        fs::write(root.join("notes.txt"), "{}").unwrap();

        let profiles = list_profiles_in(&root);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profile_display_name(&profiles[0]), "a");
        assert_eq!(profile_display_name(&profiles[1]), "b");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn base64_codec_matches_standard_vectors() {
        for (plain, encoded) in [
            (b"".as_slice(), ""),
            (b"f".as_slice(), "Zg=="),
            (b"fo".as_slice(), "Zm8="),
            (b"foo".as_slice(), "Zm9v"),
            (b"foobar".as_slice(), "Zm9vYmFy"),
        ] {
            assert_eq!(encode_base64(plain), encoded);
            assert_eq!(decode_base64(encoded).unwrap(), plain);
        }
        assert!(decode_base64("abc").is_err());
        assert!(decode_base64("=m9v").is_err());
    }

    #[test]
    fn workflow_package_round_trip_embeds_template_images() {
        let root = std::env::temp_dir().join(format!(
            "m5771-package-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&root).unwrap();
        let image_path = root.join("source.png");
        image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255]))
            .save(&image_path)
            .unwrap();

        let mut profile = MacroProfile {
            name: "shareable".to_owned(),
            ..MacroProfile::default()
        };
        profile.templates.push(TemplateAsset {
            id: 7,
            name: "button".to_owned(),
            path: image_path.to_string_lossy().into_owned(),
            width: 8,
            height: 6,
            reference_width: 1280,
            reference_height: 720,
            search_region: None,
        });
        profile.steps[0].template = Some(profile.templates[0].path.clone());

        let package_path = root.join("flow.m5771pack");
        let exported = export_workflow_package(&package_path, &profile).unwrap();
        assert_eq!(exported.template_count, 1);

        let import_root = root.join("imports");
        let (imported, summary) = import_workflow_package_to(&package_path, &import_root).unwrap();
        assert_eq!(summary.profile_name, "shareable");
        assert_eq!(imported.templates.len(), 1);
        assert!(Path::new(&imported.templates[0].path).exists());
        assert_eq!(
            imported.steps[0].template,
            Some(imported.templates[0].path.clone())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_starter_package_matches_v1_schema() {
        let package: WorkflowPackage =
            serde_json::from_str(include_str!("../examples/auto-starter.m5771pack")).unwrap();
        assert_eq!(package.format, PACKAGE_FORMAT);
        assert_eq!(package.format_version, PACKAGE_VERSION);
        assert!(package.profile.validate().is_ok());
        assert!(package.assets.is_empty());
    }

    #[test]
    fn bundled_event_package_matches_v1_schema() {
        let package: WorkflowPackage = serde_json::from_str(include_str!(
            "../examples/morimens-event-tongdiao.m5771pack"
        ))
        .unwrap();
        assert_eq!(package.format, PACKAGE_FORMAT);
        assert_eq!(package.format_version, PACKAGE_VERSION);
        assert!(package.profile.validate().is_ok());
        assert_eq!(package.assets.len(), package.profile.templates.len());
        for template in &package.profile.templates {
            let asset = package
                .assets
                .iter()
                .find(|asset| asset.key == template.path)
                .expect("template asset missing");
            let bytes = decode_base64(&asset.data_base64).unwrap();
            assert_eq!(bytes.len(), asset.byte_length);
            let image = image::load_from_memory(&bytes).unwrap();
            assert_eq!(
                (image.width(), image.height()),
                (template.width, template.height)
            );
        }
    }
}
