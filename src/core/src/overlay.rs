//! Coordinates display windows without committing geometry after a partial failure.
/// Display frame in logical points: x, y, width, height.
pub type Screen = (i32, i32, u32, u32);

/// Converts a monitor's pixel frame with that monitor's own scale.
/// Window setters divide by the window's current scale, so pixels from a display
/// with a different scale would misplace the overlay on mixed-DPI setups.
pub fn screen(position: (i32, i32), size: (u32, u32), scale: f64) -> Screen {
    (
        (f64::from(position.0) / scale).round() as i32,
        (f64::from(position.1) / scale).round() as i32,
        (f64::from(size.0) / scale).round() as u32,
        (f64::from(size.1) / scale).round() as u32,
    )
}

pub trait Windows {
    fn reset(&mut self) -> Result<(), String>;
    fn exists(&self, index: usize) -> bool;
    fn create(&mut self, index: usize) -> Result<(), String>;
    fn configure(&mut self, index: usize, screen: Screen) -> Result<(), String>;
    fn show(&mut self, index: usize) -> Result<(), String>;
}

pub fn sync(
    cached: &mut Option<Vec<Screen>>,
    screens: Vec<Screen>,
    windows: &mut impl Windows,
) -> Result<(), String> {
    // Invalidate before setup so every failure triggers a complete retry.
    let changed = cached.take().as_ref() != Some(&screens);
    if changed {
        windows.reset()?;
    }
    for (index, screen) in screens.iter().copied().enumerate() {
        let created = !windows.exists(index);
        if created {
            windows.create(index)?;
        }
        if changed || created {
            windows.configure(index, screen)?;
        }
        windows.show(index)?;
    }
    *cached = Some(screens);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeWindows {
        windows: HashMap<usize, Option<Screen>>,
        fail_create: Option<usize>,
        fail_configure: Option<usize>,
        fail_show: Option<usize>,
    }
    impl Windows for FakeWindows {
        fn reset(&mut self) -> Result<(), String> {
            self.windows.clear();
            Ok(())
        }
        fn exists(&self, index: usize) -> bool {
            self.windows.contains_key(&index)
        }
        fn create(&mut self, index: usize) -> Result<(), String> {
            if self.fail_create == Some(index) {
                return Err("create failed".into());
            }
            self.windows.insert(index, None);
            Ok(())
        }
        fn configure(&mut self, index: usize, screen: Screen) -> Result<(), String> {
            if self.fail_configure == Some(index) {
                return Err("configure failed".into());
            }
            *self.windows.get_mut(&index).unwrap() = Some(screen);
            Ok(())
        }
        fn show(&mut self, index: usize) -> Result<(), String> {
            if self.fail_show == Some(index) {
                return Err("show failed".into());
            }
            Ok(())
        }
    }
    const SCREENS: [Screen; 2] = [(0, 0, 1920, 1080), (-1280, 0, 1280, 1024)];

    #[test]
    fn retries_partial_creation_configuration_and_show_failures() {
        for failure in ["create", "configure", "show"] {
            let mut windows = FakeWindows::default();
            match failure {
                "create" => windows.fail_create = Some(1),
                "configure" => windows.fail_configure = Some(1),
                _ => windows.fail_show = Some(1),
            }
            let mut cached = None;
            assert!(sync(&mut cached, SCREENS.to_vec(), &mut windows).is_err());
            assert_eq!(cached, None, "failed {failure} must not commit geometry");
            windows.fail_create = None;
            windows.fail_configure = None;
            windows.fail_show = None;
            sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
            assert_eq!(cached, Some(SCREENS.to_vec()));
            for (index, screen) in SCREENS.iter().enumerate() {
                assert_eq!(windows.windows[&index], Some(*screen));
            }
        }
    }

    #[test]
    fn configures_a_recreated_window_with_unchanged_displays() {
        let mut cached = None;
        let mut windows = FakeWindows::default();
        sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
        windows.windows.remove(&1);
        sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
        assert_eq!(windows.windows[&1], Some(SCREENS[1]));
    }

    #[test]
    fn retries_failed_configuration_when_displays_did_not_change() {
        let mut cached = None;
        let mut windows = FakeWindows::default();
        sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
        windows.windows.remove(&1);
        windows.fail_configure = Some(1);
        assert!(sync(&mut cached, SCREENS.to_vec(), &mut windows).is_err());
        assert_eq!(cached, None);
        windows.fail_configure = None;
        sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
        assert_eq!(windows.windows[&1], Some(SCREENS[1]));
    }

    #[test]
    fn mixed_scale_displays_convert_with_their_own_scale() {
        // Retina built-in at 2x beside a 1x external monitor on its left.
        let builtin = screen((0, 0), (3024, 1964), 2.0);
        let external = screen((-1920, 0), (1920, 1080), 1.0);
        assert_eq!(builtin, (0, 0, 1512, 982));
        assert_eq!(external, (-1920, 0, 1920, 1080));
        assert_eq!(screen((1, 3), (5, 7), 2.0), (1, 2, 3, 4));
    }

    #[test]
    fn display_removal_and_resize_replace_old_geometry() {
        let mut cached = None;
        let mut windows = FakeWindows::default();
        sync(&mut cached, SCREENS.to_vec(), &mut windows).unwrap();
        let resized = vec![(0, 0, 2560, 1440)];
        sync(&mut cached, resized.clone(), &mut windows).unwrap();
        assert_eq!(windows.windows.len(), 1);
        assert_eq!(windows.windows[&0], Some(resized[0]));
        assert_eq!(cached, Some(resized));
    }
}
