use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataGroup {
    pub name: String,
    pub entries: Vec<MetadataEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsCoordinates {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataReport {
    pub file_name: String,
    pub file_size: u64,
    pub file_path: String,
    pub format: String,
    pub dimensions: Option<(u32, u32)>,
    pub gps: Option<GpsCoordinates>,
    pub groups: Vec<MetadataGroup>,
}

/// Read and parse structured metadata from an image or document.
pub fn read_metadata(path: &Path) -> Result<FileMetadataReport> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let meta = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {:?}", path))?;
    let file_size = meta.len();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut groups = Vec::new();
    let mut file_entries = vec![
        MetadataEntry {
            key: "File Name".into(),
            value: file_name.clone(),
        },
        MetadataEntry {
            key: "File Size".into(),
            value: format!("{:.2} KB ({} bytes)", file_size as f64 / 1024.0, file_size),
        },
        MetadataEntry {
            key: "Format".into(),
            value: ext.to_uppercase(),
        },
    ];

    if let Ok(modified) = meta.modified() {
        let dt: chrono::DateTime<chrono::Utc> = modified.into();
        file_entries.push(MetadataEntry {
            key: "Modified".into(),
            value: dt.to_rfc3339(),
        });
    }

    let mut dimensions = None;
    if let Ok(reader) = ImageReader::open(path) {
        if let Ok(dimensions_res) = reader.into_dimensions() {
            dimensions = Some(dimensions_res);
            file_entries.push(MetadataEntry {
                key: "Dimensions".into(),
                value: format!("{} × {} px", dimensions_res.0, dimensions_res.1),
            });
        }
    }

    groups.push(MetadataGroup {
        name: "File Information".into(),
        entries: file_entries,
    });

    // Parse EXIF tags using kamadak-exif if available
    let mut exif_entries = Vec::new();
    let mut camera_entries = Vec::new();
    let mut gps_entries = Vec::new();
    let mut parsed_gps = None;

    if let Ok(file) = std::fs::File::open(path) {
        let mut buf_reader = std::io::BufReader::new(file);
        let exif_reader = exif::Reader::new();
        if let Ok(exif) = exif_reader.read_from_container(&mut buf_reader) {
            let mut lat: Option<f64> = None;
            let mut lat_ref = "N";
            let mut lon: Option<f64> = None;
            let mut lon_ref = "E";
            let mut alt: Option<f64> = None;

            for f in exif.fields() {
                let tag_name = format!("{}", f.tag);
                let tag_val = f.display_value().with_unit(&exif).to_string();

                match f.tag {
                    exif::Tag::Make | exif::Tag::Model | exif::Tag::LensModel | exif::Tag::Software => {
                        camera_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    exif::Tag::GPSLatitude => {
                        if let exif::Value::Rational(ref r) = f.value {
                            if r.len() == 3 {
                                lat = Some(r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0);
                            }
                        }
                        gps_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    exif::Tag::GPSLatitudeRef => {
                        lat_ref = if tag_val.contains('S') { "S" } else { "N" };
                        gps_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    exif::Tag::GPSLongitude => {
                        if let exif::Value::Rational(ref r) = f.value {
                            if r.len() == 3 {
                                lon = Some(r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0);
                            }
                        }
                        gps_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    exif::Tag::GPSLongitudeRef => {
                        lon_ref = if tag_val.contains('W') { "W" } else { "E" };
                        gps_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    exif::Tag::GPSAltitude => {
                        if let exif::Value::Rational(ref r) = f.value {
                            if !r.is_empty() {
                                alt = Some(r[0].to_f64());
                            }
                        }
                        gps_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                    _ => {
                        exif_entries.push(MetadataEntry {
                            key: tag_name,
                            value: tag_val,
                        });
                    }
                }
            }

            if let (Some(mut la), Some(mut lo)) = (lat, lon) {
                if lat_ref == "S" {
                    la = -la;
                }
                if lon_ref == "W" {
                    lo = -lo;
                }
                parsed_gps = Some(GpsCoordinates {
                    latitude: la,
                    longitude: lo,
                    altitude: alt,
                });
            }
        }
    }

    if !camera_entries.is_empty() {
        groups.push(MetadataGroup {
            name: "Camera & Equipment".into(),
            entries: camera_entries,
        });
    }

    if !gps_entries.is_empty() {
        groups.push(MetadataGroup {
            name: "GPS Location".into(),
            entries: gps_entries,
        });
    }

    if !exif_entries.is_empty() {
        groups.push(MetadataGroup {
            name: "EXIF Attributes".into(),
            entries: exif_entries,
        });
    }

    Ok(FileMetadataReport {
        file_name,
        file_size,
        file_path: path.to_string_lossy().to_string(),
        format: ext,
        dimensions,
        gps: parsed_gps,
        groups,
    })
}

/// Strip metadata from the file, producing a cleaned copy.
/// If `strip_gps_only` is true, attempts to remove GPS tags only.
/// If false (or if format requires), re-encodes pure pixel data, wiping all EXIF/IPTC/GPS headers cleanly.
pub fn strip_metadata(input: &Path, output: &Path, strip_gps_only: bool) -> Result<PathBuf> {
    info!(
        "Stripping metadata from {:?} -> {:?} (strip_gps_only: {})",
        input, output, strip_gps_only
    );

    // Re-decoding and re-encoding raw image pixels cleanly strips all embedded metadata headers
    let img = image::open(input)
        .with_context(|| format!("Failed to open image for stripping metadata: {:?}", input))?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = output.with_extension("clean.tmp");
    let format = image::ImageFormat::from_path(output).unwrap_or(image::ImageFormat::Jpeg);

    img.save_with_format(&tmp, format)
        .with_context(|| format!("Failed to save clean image to {:?}", tmp))?;

    std::fs::rename(&tmp, output)
        .with_context(|| format!("Failed to rename {:?} -> {:?}", tmp, output))?;

    info!("Clean image written successfully to {:?}", output);
    Ok(output.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_read_and_strip_metadata() {
        let dir = std::env::temp_dir().join("wheel_meta_tests");
        let _ = std::fs::create_dir_all(&dir);

        let input_path = dir.join("test_meta.png");
        let output_path = dir.join("test_meta_clean.png");

        let mut img = RgbaImage::new(100, 100);
        for p in img.pixels_mut() {
            *p = Rgba([50, 100, 150, 255]);
        }
        img.save(&input_path).unwrap();

        let report = read_metadata(&input_path).unwrap();
        assert_eq!(report.format, "png");
        assert_eq!(report.dimensions, Some((100, 100)));
        assert!(!report.groups.is_empty());

        let stripped = strip_metadata(&input_path, &output_path, false).unwrap();
        assert_eq!(stripped, output_path);
        assert!(output_path.exists());

        let _ = std::fs::remove_file(input_path);
        let _ = std::fs::remove_file(output_path);
    }
}
