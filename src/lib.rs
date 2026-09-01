//! AviUtl2のトラックバーの情報を読み書きするためのライブラリ。

/// 時間制御の制御点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeControlPoint {
    pub coordinate: (f64, f64),
    pub handle_offset: (f64, f64),
}

bitflags::bitflags! {
    /// トラックバーのフラグ。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TrackFlag: u32 {
        /// 「加速」フラグ。
        const ACCELERATE = 0b00000001;
        /// 「減速」フラグ。
        const DECELERATE = 0b00000010;
        /// 「中間点無視」フラグ。
        const TWOPOINT = 0b00000100;
        /// 「中間点と制御点をリンク」フラグ。
        const LINK_MIDPOINTS_AND_CONTROL_POINTS = 0b00010000;
        /// 参照式が存在するかどうか。
        const REFERENCE = 0b00001000;
    }
}

/// トラックバーのスクリプトの情報。
#[derive(Debug, Clone, PartialEq)]
pub struct Movement {
    /// スクリプトの名前。
    pub name: String,
    /// スクリプトのパラメーター。
    pub parameters: Vec<f64>,
}

/// トラックバーの情報。
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// トラックの中間点ごとの設定値。
    pub values: Vec<f64>,
    /// 小数点下の精度。
    /// `0`の場合は整数値、`1`の場合は小数点以下1桁、`2`の場合は小数点以下2桁...という値になります。
    pub precision: usize,

    /// フラグ。
    ///
    /// # Note
    ///
    /// シリアライズ時は参照式のフラグはこの値ではなく`reference`の有無で決定されます。
    pub flag: TrackFlag,

    /// 使っている移動スクリプト。
    /// `None`の場合は「移動無し」です。
    pub movement: Option<Movement>,

    /// 時間制御の制御点。
    pub time_control_points: Option<Vec<TimeControlPoint>>,

    /// 参照式。
    pub reference: Option<String>,
}

/// トラックバーの情報をパースできなかった場合のエラー。
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TrackParseError {
    /// 不明なフォーマットが指定された場合。
    #[error("unknown format")]
    UnknownFormat,
    /// 数値が不正な場合。
    #[error("invalid number")]
    InvalidNumber(#[from] std::num::ParseFloatError),
}

fn get_precision(value: &str) -> usize {
    if let Some(pos) = value.find('.') {
        value.len() - pos - 1
    } else {
        0
    }
}

fn serialize_float(value: f64) -> String {
    let value = format!("{value:.6}");
    let value = value.trim_end_matches('0').trim_end_matches('.');

    if value == "-0" {
        "0".to_string()
    } else {
        value.to_string()
    }
}

impl Track {
    /// 既存のトラックバーの情報をパースします。
    pub fn parse(
        track: &str,
        info: &Option<aviutl2::generic::TrackInfo>,
    ) -> Result<Self, TrackParseError> {
        if track.is_empty() {
            return Err(TrackParseError::UnknownFormat);
        }
        let splat = track.split('|').collect::<Vec<_>>();
        if info.is_none() && splat.len() == 1 {
            return Ok(Self {
                values: vec![
                    splat[0]
                        .parse::<f64>()
                        .map_err(TrackParseError::InvalidNumber)?,
                ],
                precision: get_precision(splat[0]),
                flag: TrackFlag::empty(),
                movement: None,
                time_control_points: None,
                reference: None,
            });
        }
        let mut splat = splat.iter();
        let first_section = splat.next().unwrap().split(',').collect::<Vec<_>>();
        if first_section.len() < 3 {
            return Err(TrackParseError::UnknownFormat);
        }
        let flag = TrackFlag::from_bits_retain(
            first_section
                .last()
                .unwrap()
                .parse::<u32>()
                .map_err(|_| TrackParseError::UnknownFormat)?,
        );
        let movement_script_name = first_section[first_section.len() - 2];
        let precision = get_precision(first_section[0]);
        let values = first_section[0..first_section.len() - 2]
            .iter()
            .map(|s| s.parse::<f64>().map_err(TrackParseError::InvalidNumber))
            .collect::<Result<Vec<_>, _>>()?;
        let movement = if movement_script_name == "移動無し" {
            None
        } else {
            Some(Movement {
                name: movement_script_name.to_string(),
                parameters: info
                    .as_ref()
                    .ok_or(TrackParseError::UnknownFormat)?
                    .params
                    .clone(),
            })
        };
        let reference = flag
            .contains(TrackFlag::REFERENCE)
            .then(|| {
                splat
                    .next()
                    .ok_or(TrackParseError::UnknownFormat)
                    .map(|s| s.to_string())
            })
            .transpose()?;
        if movement.as_ref().is_some_and(|m| !m.parameters.is_empty()) {
            // パラメーターはTrackInfoから取得するため、ここでは無視する
            // ただしカーソルは進めておく
            if splat.next().is_none() {
                return Err(TrackParseError::UnknownFormat);
            }
        }
        let timecontrol = info
            .as_ref()
            .is_some_and(|i| i.timecontrol)
            .then(|| match splat.next() {
                Some(time_control_str) => {
                    let time_control_points = time_control_str.split(',').collect::<Vec<_>>();
                    if time_control_points.len() % 4 != 0 {
                        return Err(TrackParseError::UnknownFormat);
                    }
                    if time_control_points.len() == 4 {
                        Ok(vec![
                            TimeControlPoint {
                                coordinate: (0.0, 0.0),
                                handle_offset: (
                                    time_control_points[0]
                                        .parse::<f64>()
                                        .map_err(TrackParseError::InvalidNumber)?,
                                    time_control_points[1]
                                        .parse::<f64>()
                                        .map_err(TrackParseError::InvalidNumber)?,
                                ),
                            },
                            TimeControlPoint {
                                coordinate: (1.0, 1.0),
                                handle_offset: (
                                    time_control_points[2]
                                        .parse::<f64>()
                                        .map_err(TrackParseError::InvalidNumber)?,
                                    time_control_points[3]
                                        .parse::<f64>()
                                        .map_err(TrackParseError::InvalidNumber)?,
                                ),
                            },
                        ])
                    } else {
                        time_control_points
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|chunk| {
                                Ok(TimeControlPoint {
                                    coordinate: (
                                        chunk[0]
                                            .parse::<f64>()
                                            .map_err(TrackParseError::InvalidNumber)?,
                                        chunk[1]
                                            .parse::<f64>()
                                            .map_err(TrackParseError::InvalidNumber)?,
                                    ),
                                    handle_offset: (
                                        chunk[2]
                                            .parse::<f64>()
                                            .map_err(TrackParseError::InvalidNumber)?,
                                        chunk[3]
                                            .parse::<f64>()
                                            .map_err(TrackParseError::InvalidNumber)?,
                                    ),
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                    }
                }
                None => Ok(vec![
                    TimeControlPoint {
                        coordinate: (0.0, 0.0),
                        handle_offset: (0.25, 0.25),
                    },
                    TimeControlPoint {
                        coordinate: (1.0, 1.0),
                        handle_offset: (0.25, 0.25),
                    },
                ]),
            })
            .transpose()?;

        Ok(Self {
            values,
            precision,
            flag,
            movement,
            time_control_points: timecontrol,
            reference,
        })
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values = self
            .values
            .iter()
            .map(|value| format!("{value:.precision$}", precision = self.precision))
            .collect::<Vec<_>>()
            .join(",");

        if self.movement.is_none()
            && self.values.len() == 1
            && self.flag.is_empty()
            && self.time_control_points.is_none()
            && self.reference.is_none()
        {
            return f.write_str(&values);
        }

        let mut flag = self.flag;
        flag.set(TrackFlag::REFERENCE, self.reference.is_some());

        let movement_name = self
            .movement
            .as_ref()
            .map_or("移動無し", |movement| movement.name.as_str());
        write!(f, "{values},{movement_name},{}", flag.bits())?;

        if let Some(reference) = &self.reference {
            write!(f, "|{reference}")?;
        }

        if let Some(movement) = &self.movement
            && !movement.parameters.is_empty()
        {
            let parameters = movement
                .parameters
                .iter()
                .map(|value| serialize_float(*value))
                .collect::<Vec<_>>()
                .join(",");
            write!(f, "|{parameters}")?;
        }

        if let Some(time_control_points) = &self.time_control_points {
            const DEFAULT_TIME_CONTROL_POINTS: [TimeControlPoint; 2] = [
                TimeControlPoint {
                    coordinate: (0.0, 0.0),
                    handle_offset: (0.25, 0.25),
                },
                TimeControlPoint {
                    coordinate: (1.0, 1.0),
                    handle_offset: (0.25, 0.25),
                },
            ];

            if time_control_points.as_slice() == DEFAULT_TIME_CONTROL_POINTS {
                return Ok(());
            }

            let is_short_format = time_control_points.len() == 2
                && time_control_points[0].coordinate == (0.0, 0.0)
                && time_control_points[1].coordinate == (1.0, 1.0);
            let time_control = time_control_points
                .iter()
                .flat_map(|point| {
                    if is_short_format {
                        vec![point.handle_offset.0, point.handle_offset.1]
                    } else {
                        vec![
                            point.coordinate.0,
                            point.coordinate.1,
                            point.handle_offset.0,
                            point.handle_offset.1,
                        ]
                    }
                })
                .map(serialize_float)
                .collect::<Vec<_>>()
                .join(",");
            write!(f, "|{time_control}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_movement() {
        let base = "42.00";
        let parsed = Track::parse(base, &None).unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_movement() {
        let base = "50.000,100.000,直線移動,3";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "直線移動".to_string(),
                params: vec![],
                accelerate: true,
                decelerate: true,
                twopoint: false,
                timecontrol: false,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_default_timecontrol() {
        let base = "50.000,25.000,100.000,直線移動(時間制御),0";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "直線移動(時間制御)".to_string(),
                params: vec![],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_short_timecontrol() {
        let base = "50.000,100.000,直線移動(時間制御),0|0.25,1,0.191176,1";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "直線移動(時間制御)".to_string(),
                params: vec![],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_full_timecontrol() {
        let base = "50.000,100.000,直線移動(時間制御),0|0,0.538889,0.25,0.461111,1,1,0.191176,1";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "直線移動(時間制御)".to_string(),
                params: vec![],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_params_and_default_timecontrol() {
        let base = "50.000,100.000,コマ落ち時間制御@Basic_S,0|0,0.5,1,1,0,0,0,360,0";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "コマ落ち時間制御@Basic_S".to_string(),
                params: vec![0.0, 0.5, 1.0, 1.0, 0.0, 0.0, 0.0, 360.0, 0.0],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_params_and_short_timecontrol() {
        let base = "50.000,100.000,コマ落ち時間制御@Basic_S,0|0,0.5,1,1,0,0,0,360,0|0.25,0.461111,0.191176,1";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "コマ落ち時間制御@Basic_S".to_string(),
                params: vec![0.0, 0.5, 1.0, 1.0, 0.0, 0.0, 0.0, 360.0, 0.0],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_params_and_full_timecontrol() {
        let base = "50.000,100.000,コマ落ち時間制御@Basic_S,0|0,0.5,1,1,0,0,0,360,0|0,0.538889,0.25,0.461111,1,1,0.191176,1";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "コマ落ち時間制御@Basic_S".to_string(),
                params: vec![0.0, 0.5, 1.0, 1.0, 0.0, 0.0, 0.0, 360.0, 0.0],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_parse_with_params_expression_and_full_timecontrol() {
        let base = "50.000,100.000,コマ落ち時間制御@Basic_S,8|$|0,0.5,1,1,0,0,0,360,0|0,0,0.165441,0.411111,0.264706,0.366667,0.2,0,1,1,0.202206,0.516667";
        let parsed = Track::parse(
            base,
            &Some(aviutl2::generic::TrackInfo {
                mode: "コマ落ち時間制御@Basic_S".to_string(),
                params: vec![0.0, 0.5, 1.0, 1.0, 0.0, 0.0, 0.0, 360.0, 0.0],
                accelerate: false,
                decelerate: false,
                twopoint: false,
                timecontrol: true,
                group_num: 1,
                group_index: 0,
                group_name: None,
            }),
        )
        .unwrap();
        insta::assert_debug_snapshot!(parsed);
        assert_eq!(base, parsed.to_string());
    }

    #[test]
    fn test_to_string_float_precision() {
        let track = Track {
            values: vec![42.0],
            precision: 2,
            flag: TrackFlag::empty(),
            movement: Some(Movement {
                name: "test".to_string(),
                parameters: vec![1.234_567_89, 1.2, -0.000_000_1],
            }),
            time_control_points: None,
            reference: None,
        };

        assert_eq!("42.00,test,0|1.234568,1.2,0", track.to_string());
    }
}
