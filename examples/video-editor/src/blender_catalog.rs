//! Socket and property metadata extracted from the official Blender 4.5 LTS runtime.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Socket {
    pub name: String,
    pub kind: String,
    pub index: usize,
    pub enabled: bool,
    pub default: Value,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Parameter {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub value: Value,
    pub options: Vec<Choice>,
    pub min: f64,
    pub max: f64,
}
impl Parameter {
    pub fn text(&self) -> String {
        self.value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| self.value.to_string())
    }
    pub fn parse(&self, text: &str) -> Result<Value, String> {
        let invalid = || {
            format!(
                "{}: enter a valid {} value.",
                self.label,
                self.kind.to_lowercase()
            )
        };
        match self.kind.as_str() {
            "STRING" | "FILE" => Ok(Value::String(text.into())),
            "ENUM" => self
                .options
                .iter()
                .any(|o| o.value == text)
                .then(|| Value::String(text.into()))
                .ok_or_else(invalid),
            "BOOLEAN" => match text {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                _ => Err(invalid()),
            },
            "FLOAT" | "INT" => {
                let value = text.trim().parse::<f64>().map_err(|_| invalid())?;
                if !value.is_finite()
                    || value < self.min
                    || value > self.max
                    || (self.kind == "INT" && value.fract() != 0.)
                {
                    return Err(format!(
                        "{} must be between {} and {}{}.",
                        self.label,
                        self.min,
                        self.max,
                        if self.kind == "INT" {
                            " (whole numbers)"
                        } else {
                            ""
                        }
                    ));
                }
                Ok(if self.kind == "INT" {
                    Value::from(value as i64)
                } else {
                    Value::from(value)
                })
            }
            "VECTOR" | "COLOR" => {
                let value: Vec<f64> = serde_json::from_str(text).map_err(|_| invalid())?;
                let size = self.value.as_array().map_or(0, Vec::len);
                if value.len() != size
                    || value
                        .iter()
                        .any(|v| !v.is_finite() || *v < self.min || *v > self.max)
                {
                    return Err(format!(
                        "{} needs {size} finite components in [x, y, …] form.",
                        self.label
                    ));
                }
                Ok(serde_json::json!(value))
            }
            "JSON" => {
                let value: Value = serde_json::from_str(text).map_err(|_| invalid())?;
                let rows = value.as_array().ok_or_else(invalid)?;
                if self.key == "color_ramp" {
                    if !(2..=32).contains(&rows.len())
                        || rows.iter().any(|s| {
                            !s["position"]
                                .as_f64()
                                .is_some_and(|v| v.is_finite() && (0. ..=1.).contains(&v))
                                || !s["color"].as_array().is_some_and(|c| {
                                    c.len() == 4
                                        && c.iter().all(|v| {
                                            v.as_f64().is_some_and(|v| v.is_finite() && v >= 0.)
                                        })
                                })
                        })
                    {
                        return Err("A ramp needs 2–32 stops, positions from 0 to 1 and four nonnegative color channels.".into());
                    }
                } else if self.key == "curves"
                    && (rows.len() != self.value.as_array().map_or(0, Vec::len)
                        || rows.iter().any(|curve| {
                            !curve.as_array().is_some_and(|points| {
                                (2..=32).contains(&points.len())
                                    && points.iter().all(|p| {
                                        p.as_array().is_some_and(|xy| {
                                            xy.len() == 2
                                                && xy.iter().all(|v| {
                                                    v.as_f64().is_some_and(|v| v.is_finite())
                                                })
                                        })
                                    })
                            })
                        }))
                {
                    return Err(
                        "Each curve needs 2–32 points with finite X and Y coordinates.".into(),
                    );
                }
                Ok(value)
            }
            _ => Err(invalid()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Definition {
    pub id: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub inputs: Vec<Socket>,
    pub outputs: Vec<Socket>,
    pub parameters: Vec<Parameter>,
}
#[derive(Deserialize)]
pub struct Catalog {
    pub version: String,
    pub nodes: Vec<Definition>,
}
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/blender-nodes.json"))
            .expect("checked-in Blender catalog")
    })
}
pub fn socket_color(kind: &str) -> u32 {
    match kind {
        "RGBA" => 0xd8bd76,
        "VECTOR" | "COLOR" => 0x8098df,
        "BOOLEAN" => 0xd48cbb,
        "AUDIO" => 0x64c5a0,
        _ => 0xa7abb1,
    }
}

/// Flatten vectors, ramp stops and curve points into ordinary numeric fields.
pub fn components(parameter: &Parameter, text: &str) -> Vec<(String, String)> {
    let value: Value = serde_json::from_str(text).unwrap_or_else(|_| parameter.value.clone());
    match parameter.kind.as_str() {
        "VECTOR" | "COLOR" => value
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, v)| {
                let names = if parameter.kind == "COLOR" {
                    ["R", "G", "B", "A"]
                } else {
                    ["X", "Y", "Z", "W"]
                };
                (names.get(i).unwrap_or(&"Value").to_string(), v.to_string())
            })
            .collect(),
        "JSON" if parameter.key == "color_ramp" => value
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .flat_map(|(i, stop)| {
                let mut fields = vec![(
                    format!("Stop {} · position", i + 1),
                    stop["position"].to_string(),
                )];
                fields.extend(
                    stop["color"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .map(|(channel, v)| {
                            (["R", "G", "B", "A"][channel].to_string(), v.to_string())
                        }),
                );
                fields
            })
            .collect(),
        "JSON" if parameter.key == "curves" => {
            value
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .flat_map(|(i, curve)| {
                    curve.as_array().into_iter().flatten().enumerate().flat_map(
                        move |(j, point)| {
                            point.as_array().into_iter().flatten().enumerate().map(
                                move |(axis, v)| {
                                    (
                                        format!(
                                            "Curve {} · point {} · {}",
                                            i + 1,
                                            j + 1,
                                            if axis == 0 { "X" } else { "Y" }
                                        ),
                                        v.to_string(),
                                    )
                                },
                            )
                        },
                    )
                })
                .collect()
        }
        _ => vec![],
    }
}
pub fn assemble(parameter: &Parameter, text: &str, parts: &[String]) -> Result<String, String> {
    if parts.is_empty() {
        parameter.parse(text)?;
        return Ok(text.into());
    }
    let values = parts
        .iter()
        .map(|text| {
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("{} needs finite numeric values.", parameter.label))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut result: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if matches!(parameter.kind.as_str(), "VECTOR" | "COLOR") {
        result = serde_json::json!(values);
    } else if parameter.key == "color_ramp" {
        for (stop, values) in result
            .as_array_mut()
            .ok_or("Invalid ramp")?
            .iter_mut()
            .zip(values.as_chunks::<5>().0.iter())
        {
            stop["position"] = Value::from(values[0]);
            stop["color"] = serde_json::json!(&values[1..]);
        }
    } else if parameter.key == "curves" {
        let mut numbers = values.into_iter();
        for curve in result.as_array_mut().ok_or("Invalid curves")? {
            for point in curve.as_array_mut().ok_or("Invalid curve")? {
                for coordinate in point.as_array_mut().ok_or("Invalid point")? {
                    *coordinate = Value::from(numbers.next().ok_or("Missing curve coordinate")?);
                }
            }
        }
    }
    let text = result.to_string();
    parameter.parse(&text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_catalog_defaults_and_structured_controls_round_trip() {
        let catalog = catalog();
        assert_eq!(catalog.nodes.len(), 131);
        let mut ids = std::collections::BTreeSet::new();
        for node in &catalog.nodes {
            assert!(ids.insert(&node.id));
            for p in &node.parameters {
                let text = p.text();
                p.parse(&text)
                    .unwrap_or_else(|error| panic!("{} / {error} = {text}", node.id));
                let fields = components(p, &text)
                    .into_iter()
                    .map(|(_, v)| v)
                    .collect::<Vec<_>>();
                let result = assemble(p, &text, &fields).unwrap();
                assert_eq!(p.parse(&text).unwrap(), p.parse(&result).unwrap());
            }
        }
        for id in [
            "CompositorNodeGlare",
            "CompositorNodeCryptomatteV2",
            "CompositorNodeKeying",
            "CompositorNodeDenoise",
            "CompositorNodeGroup",
            "NodeFrame",
            "NodeReroute",
            "ShaderNodeBlackbody",
            "ShaderNodeClamp",
            "ShaderNodeTexGabor",
            "ShaderNodeVectorMath",
        ] {
            assert!(ids.iter().any(|value| value.as_str() == id));
        }
    }
}
