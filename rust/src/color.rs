const SCREEN_COLOR_DEFAULT: i32 = -2;
const ARGB_OPAQUE: u32 = 0xff00_0000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorLevel {
    Reset,
    Argb([f32; 3]),
    LegacyTemperature(i32),
    Invalid,
}

pub fn parse_color_level(value: i32) -> ColorLevel {
    if value == SCREEN_COLOR_DEFAULT {
        return ColorLevel::Reset;
    }
    if (1..=3).contains(&value) {
        return ColorLevel::LegacyTemperature(value);
    }

    let argb = value as u32;
    if argb & ARGB_OPAQUE != ARGB_OPAQUE {
        return ColorLevel::Invalid;
    }

    let red = ((argb >> 16) & 0xff) as f32;
    let green = ((argb >> 8) & 0xff) as f32;
    let blue = (argb & 0xff) as f32;
    let maximum = red.max(green).max(blue);
    if maximum == 0.0 {
        return ColorLevel::Argb([1.0, 1.0, 1.0]);
    }

    ColorLevel::Argb([red / maximum, green / maximum, blue / maximum])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub style: i32,
    pub temperature: i32,
    pub eye: i32,
    pub color_gains: Option<[f32; 3]>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            style: 0,
            temperature: 2,
            eye: 0,
            color_gains: None,
        }
    }
}

impl State {
    pub fn update(&mut self, mode: i32, value: i32) -> bool {
        match mode {
            0..=2 => {
                self.style = mode;
                self.update_color_level(value);
                true
            }
            3 => {
                self.eye = if value <= 0 { 0 } else { value.clamp(50, 255) };
                true
            }
            23 => self.update_color_level(value),
            _ => false,
        }
    }

    fn update_color_level(&mut self, value: i32) -> bool {
        match parse_color_level(value) {
            ColorLevel::Reset => {
                self.color_gains = None;
                true
            }
            ColorLevel::Argb(gains) => {
                self.color_gains = Some(gains);
                true
            }
            ColorLevel::LegacyTemperature(temperature) => {
                self.temperature = temperature;
                self.color_gains = None;
                true
            }
            ColorLevel::Invalid => false,
        }
    }

    pub fn render(self) -> ([f32; 16], f32) {
        let mut matrix = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        let mut green_eye = 1.0f32;
        let mut blue_eye = 1.0f32;
        if self.eye > 0 {
            let factor = ((self.eye.clamp(50, 255) - 50) as f32) / 205.0;
            green_eye = 1.0 - (5.0 + factor * 25.0) / 255.0;
            blue_eye = 1.0 - (20.0 + factor * 95.0) / 255.0;
        }
        let (red_temperature, green_temperature, blue_temperature) = match self.temperature {
            1 => (1.0, 0.925, 0.84),
            3 => (0.88, 0.95, 1.0),
            _ => (1.0, 1.0, 1.0),
        };
        let [red_color, green_color, blue_color] = self.color_gains.unwrap_or([1.0; 3]);
        matrix[0] = red_temperature * red_color;
        matrix[5] = green_temperature * green_eye * green_color;
        matrix[10] = blue_temperature * blue_eye * blue_color;
        let saturation = match self.style {
            1 => 1.2,
            2 => 1.3,
            _ => 1.0,
        };
        (matrix, saturation)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_color_level, ColorLevel, State};

    #[test]
    fn decodes_argb_to_normalized_channel_gains() {
        assert_eq!(
            parse_color_level(0xff80_4020u32 as i32),
            ColorLevel::Argb([1.0, 0.5, 0.25])
        );
        assert_eq!(
            parse_color_level(0xffff_ffffu32 as i32),
            ColorLevel::Argb([1.0, 1.0, 1.0])
        );
        assert_eq!(
            parse_color_level(0xff00_0000u32 as i32),
            ColorLevel::Argb([1.0, 1.0, 1.0])
        );
    }

    #[test]
    fn preserves_legacy_temperature_values_and_reset_sentinel() {
        assert_eq!(parse_color_level(1), ColorLevel::LegacyTemperature(1));
        assert_eq!(parse_color_level(2), ColorLevel::LegacyTemperature(2));
        assert_eq!(parse_color_level(3), ColorLevel::LegacyTemperature(3));
        assert_eq!(parse_color_level(-2), ColorLevel::Reset);
        assert_eq!(parse_color_level(0x7f80_4020), ColorLevel::Invalid);
    }

    #[test]
    fn color_modes_apply_argb_and_reset_override() {
        let mut state = State::default();
        assert!(state.update(2, 0xff80_4020u32 as i32));
        assert_eq!(state.style, 2);
        assert_eq!(state.color_gains, Some([1.0, 0.5, 0.25]));

        assert!(state.update(23, -2));
        assert_eq!(state.color_gains, None);
        assert!(state.update(23, -2));
    }

    #[test]
    fn legacy_temperature_and_eye_controls_remain_compatible() {
        let mut state = State::default();
        assert!(state.update(0, 3));
        assert_eq!(state.temperature, 3);
        assert_eq!(state.color_gains, None);
        assert!(state.update(3, 20));
        assert_eq!(state.eye, 50);
        assert!(state.update(3, 0));
        assert_eq!(state.eye, 0);
    }

    #[test]
    fn color_ball_override_is_replaced_by_temperature_presets() {
        let mut state = State::default();
        assert!(state.update(2, 0xff80_4020u32 as i32));
        assert_eq!(state.color_gains, Some([1.0, 0.5, 0.25]));

        assert!(state.update(2, 1));
        assert_eq!(state.temperature, 1);
        assert_eq!(state.color_gains, None);
        let (warm_matrix, _) = state.render();
        assert_eq!(warm_matrix[0], 1.0);
        assert_eq!(warm_matrix[5], 0.925);
        assert_eq!(warm_matrix[10], 0.84);

        assert!(state.update(2, 3));
        assert_eq!(state.temperature, 3);
        assert_eq!(state.color_gains, None);
        let (cool_matrix, _) = state.render();
        assert_eq!(cool_matrix[0], 0.88);
        assert_eq!(cool_matrix[5], 0.95);
        assert_eq!(cool_matrix[10], 1.0);

        assert!(state.update(2, 2));
        assert_eq!(state.temperature, 2);
        assert_eq!(state.color_gains, None);
        let (default_matrix, _) = state.render();
        assert_eq!(default_matrix[0], 1.0);
        assert_eq!(default_matrix[5], 1.0);
        assert_eq!(default_matrix[10], 1.0);
    }

    #[test]
    fn replayed_argb_keeps_color_ball_override() {
        let mut state = State::default();
        assert!(state.update(2, 0xff80_4020u32 as i32));
        assert_eq!(state.color_gains, Some([1.0, 0.5, 0.25]));
        let (matrix, _) = state.render();
        assert_eq!(matrix[5], 0.5);
        assert_eq!(matrix[10], 0.25);
    }

    #[test]
    fn color_gain_combines_with_eye_matrix() {
        let mut state = State::default();
        state.update(2, 0xff80_4020u32 as i32);
        state.update(3, 255);
        let (matrix, saturation) = state.render();
        assert_eq!(matrix[0], 1.0);
        assert!((matrix[5] - 0.5 * (1.0 - 30.0 / 255.0)).abs() < 0.00001);
        assert!((matrix[10] - 0.25 * (1.0 - 115.0 / 255.0)).abs() < 0.00001);
        assert_eq!(saturation, 1.3);
    }

    #[test]
    fn unknown_and_hdr_modes_do_not_update_color_state() {
        let mut state = State::default();
        let before = state;
        assert!(!state.update(7, 0xff00_0000u32 as i32));
        assert!(!state.update(100, 0xff00_0000u32 as i32));
        assert_eq!(state, before);
    }
}
