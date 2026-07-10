use serde::{Deserialize, Serialize};

use crate::depends::{DependencyExpr, PackageRef, parse_depends_field};

/// Fields extracted from a `.deb` control file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DebControl {
    pub package: String,
    pub version: String,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<DependencyExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_depends: Vec<DependencyExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<PackageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl DebControl {
    /// Parse a Debian control file body (the `control` member inside control.tar).
    pub fn parse(control_text: &str) -> Result<Self, String> {
        let mut package = None;
        let mut version = None;
        let mut architecture = None;
        let mut depends = Vec::new();
        let mut pre_depends = Vec::new();
        let mut provides = Vec::new();
        let mut description = None;

        let mut current_key: Option<String> = None;
        let mut current_value = String::new();

        let flush = |key: &str,
                     value: &str,
                     package: &mut Option<String>,
                     version: &mut Option<String>,
                     architecture: &mut Option<String>,
                     depends: &mut Vec<DependencyExpr>,
                     pre_depends: &mut Vec<DependencyExpr>,
                     provides: &mut Vec<PackageRef>,
                     description: &mut Option<String>| {
            let value = value.trim();
            match key {
                "Package" => *package = Some(value.to_owned()),
                "Version" => *version = Some(value.to_owned()),
                "Architecture" => *architecture = Some(value.to_owned()),
                "Depends" => *depends = parse_depends_field(value),
                "Pre-Depends" => *pre_depends = parse_depends_field(value),
                "Provides" => {
                    *provides = parse_depends_field(value)
                        .into_iter()
                        .flat_map(|expr| expr.alternatives)
                        .map(|clause| clause.package)
                        .collect();
                }
                "Description" => {
                    let first = value.lines().next().unwrap_or(value).trim();
                    if !first.is_empty() {
                        *description = Some(first.to_owned());
                    }
                }
                _ => {}
            }
        };

        for line in control_text.lines() {
            if let Some(rest) = line.strip_prefix(' ') {
                if !current_value.is_empty() {
                    current_value.push('\n');
                }
                current_value.push_str(rest);
                continue;
            }
            if let Some(key) = current_key.take() {
                flush(
                    &key,
                    &current_value,
                    &mut package,
                    &mut version,
                    &mut architecture,
                    &mut depends,
                    &mut pre_depends,
                    &mut provides,
                    &mut description,
                );
                current_value.clear();
            }
            if line.trim().is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            current_key = Some(key.trim().to_owned());
            current_value = value.trim_start().to_owned();
        }
        if let Some(key) = current_key.take() {
            flush(
                &key,
                &current_value,
                &mut package,
                &mut version,
                &mut architecture,
                &mut depends,
                &mut pre_depends,
                &mut provides,
                &mut description,
            );
        }

        Ok(Self {
            package: package.ok_or_else(|| "missing Package field".to_string())?,
            version: version.ok_or_else(|| "missing Version field".to_string())?,
            architecture: architecture.unwrap_or_else(|| "all".into()),
            depends,
            pre_depends,
            provides,
            description,
        })
    }

    pub fn all_depends(&self) -> impl Iterator<Item = &DependencyExpr> {
        self.pre_depends.iter().chain(self.depends.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_control_with_depends() {
        let text = "\
Package: sigma-updates-depdemo
Version: 0.1.0-1
Architecture: all
Depends: sigma-updates-sample (>= 0.1.0)
Description: Demo package
 more text
";
        let c = DebControl::parse(text).unwrap();
        assert_eq!(c.package, "sigma-updates-depdemo");
        assert_eq!(c.depends.len(), 1);
        assert_eq!(
            c.depends[0].alternatives[0].package.name,
            "sigma-updates-sample"
        );
    }
}
